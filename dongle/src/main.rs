//! USB dongle on a Seeed XIAO BLE (nRF52840): split central. Runs the keymap
//! and Vial, receives both halves over BLE, and sends reports to the host over
//! USB (or a BLE host). It has no matrix of its own, but drives a screen.

#![no_std]
#![no_main]

mod keymap;
mod nb;
mod screen;
mod vial;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, SPI3, USBD};
use embassy_nrf::pwm::{SimpleConfig, SimplePwm};
use embassy_nrf::spim::{self, Spim};
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{bind_interrupts, rng, usb};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration};
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use rmk::ble::BleTransport;
use rmk::config::{
    BehaviorConfig, DeviceConfig, MouseKeyConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig,
};
use rmk::display::DisplayProcessor;
use rmk::display::drivers::lcd_async::LcdAsyncDisplay;
use rmk::display::lcd_async::Builder;
use rmk::display::lcd_async::interface::SpiInterface;
use rmk::display::lcd_async::models::ST7789;
use rmk::display::lcd_async::options::{ColorInversion, Orientation, Rotation};
use rmk::host::HostService;
use rmk::keyboard::Keyboard;
use rmk::split::PeripheralMatrixConfig;
use rmk::usb::UsbTransport;
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};
use screen::{ScreenController, ScreenRenderer};
use static_cell::{ConstStaticCell, StaticCell};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<USBD>;
    SPIM3 => spim::InterruptHandler<SPI3>;
    RNG => rng::InterruptHandler<RNG>;
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 3;
const L2CAP_RXQ: u8 = 3;
const L2CAP_MTU: usize = 251;

/// Hold Esc + the top-right key to unlock Vial
const VIAL_UNLOCK_KEYS: &[(u8, u8)] = &[(0, 0), (0, 13)];

/// Central toward both halves. The peripheral role is for a BLE host: RMK's
/// BLE transport always advertises, even when USB is in use.
fn build_sdc<'d, const N: usize>(
    p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    sdc::Builder::new()?
        .support_scan()
        .support_central()
        .support_adv()
        .support_peripheral()
        .support_dle_peripheral()
        .support_dle_central()
        .support_phy_update_central()
        .support_phy_update_peripheral()
        .support_le_2m_phy()
        // Left and right half
        .central_count(2)?
        .peripheral_count(1)?
        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
        .build(p, rng, mpsl, mem)
}

fn ble_addr() -> [u8; 6] {
    let ficr = embassy_nrf::pac::FICR;
    let high = u64::from(ficr.deviceid(1).read());
    let addr = high << 32 | u64::from(ficr.deviceid(0).read());
    let addr = addr | 0x0000_c000_0000_0000;
    unwrap!(addr.to_le_bytes()[..6].try_into())
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Eyelash Sofle dongle (central, XIAO BLE)");
    let p = embassy_nrf::init(embassy_nrf::config::Config::default());
    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: 500,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };
    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    static SESSION_MEM: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(unwrap!(mpsl::MultiprotocolServiceLayer::with_timeslots(
        mpsl_p,
        Irqs,
        lfclk_cfg,
        SESSION_MEM.init(mpsl::SessionMem::new())
    )));
    spawner.spawn(mpsl_task(&*mpsl).unwrap());
    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    // Oversized on purpose: too small fails at boot, too big only logs a warning
    let mut sdc_mem = sdc::Mem::<16384>::new();
    let sdc = unwrap!(build_sdc(sdc_p, &mut rng, mpsl, &mut sdc_mem));

    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));
    let flash = Flash::take(mpsl, p.NVMC);

    let device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "sofle",
        product_name: "Eyelash Sofle",
        ..DeviceConfig::default()
    };
    // 64K for the 7-layer keymap, below the bootloader at 0xF4000
    let storage_config = StorageConfig {
        start_addr: 0xA0000,
        num_sectors: 16,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        device_config,
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, VIAL_UNLOCK_KEYS),
        storage_config,
        ..Default::default()
    };

    let mut keymap_data = KeymapData::new_with_encoder(keymap::get_default_keymap(), keymap::get_default_encoder_map());
    let mut behavior_config = BehaviorConfig::default();
    behavior_config.tri_layer = Some(keymap::TRI_LAYER);
    unwrap!(behavior_config.morse.profiles.push(keymap::HOLD_TAP_PROFILE));
    // ZMK ran mouse keys at 1200 px/s scaled 2x, reaching full speed in 500ms
    behavior_config.mouse_key = MouseKeyConfig {
        move_delta: 8,
        max_speed: 6,
        ticks_to_max: 25,
        move_max: 48,
        ..Default::default()
    };
    let per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut keymap_data,
        flash,
        &storage_config,
        &mut behavior_config,
        &per_key_config,
    )
    .await;

    let mut keyboard = Keyboard::new(&keymap);
    let host_service = HostService::new(&keymap, &rmk_config);

    let mut usb_transport = UsbTransport::new(driver, rmk_config.device_config).with_host_service(&host_service);
    // Peripheral ids are set in each half's firmware
    let mut ble_transport = BleTransport::new(
        sdc,
        ble_addr(),
        rmk_config,
        [
            // 0: left half, 5x6 at cols 0..6
            PeripheralMatrixConfig {
                rows: 5,
                cols: 6,
                row_offset: 0,
                col_offset: 0,
            },
            // 1: right half, 5x7 at cols 7..14
            PeripheralMatrixConfig {
                rows: 5,
                cols: 7,
                row_offset: 0,
                col_offset: 7,
            },
        ],
    )
    .with_host_service(&host_service);

    // Screen: wiring and layout are in screen.rs. SPIM3 is the only SPIM that runs at 32MHz.
    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M32;
    // Below the priorities the BLE stack reserves
    interrupt::SPIM3.set_priority(interrupt::Priority::P3);
    static SPI_BUS: StaticCell<Mutex<NoopRawMutex, Spim<'static>>> = StaticCell::new();
    let spi_bus = SPI_BUS.init(Mutex::new(Spim::new_txonly(p.SPI3, Irqs, p.P1_13, p.P1_15, spi_config)));
    let spi = SpiDevice::new(spi_bus, Output::new(p.P1_14, Level::High, OutputDrive::Standard));
    let dc = Output::new(p.P1_12, Level::Low, OutputDrive::Standard);
    let lcd = unwrap!(
        Builder::new(ST7789, SpiInterface::new(spi, dc))
            .reset_pin(Output::new(p.P0_29, Level::High, OutputDrive::Standard))
            // In the panel's native portrait orientation, as in the ZMK shield
            .display_size(240, 280)
            .display_offset(0, 20)
            // Landscape (ZMK: CONFIG_DONGLE_SCREEN_HORIZONTAL); Deg270 if upside down
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            // Zephyr's st7789v driver always turns inversion on
            .invert_colors(ColorInversion::Inverted)
            .init(&mut Delay)
            .await
            .ok()
    );
    // 131K. Const-initialized so that it never passes through the stack.
    static FRAMEBUFFER: ConstStaticCell<[u8; screen::WIDTH * screen::HEIGHT * 2]> =
        ConstStaticCell::new([0; screen::WIDTH * screen::HEIGHT * 2]);
    let mut display = DisplayProcessor::with_renderer(
        LcdAsyncDisplay::<_, _, _, _, { screen::WIDTH }, { screen::HEIGHT }>::new(lcd, FRAMEBUFFER.take()),
        ScreenRenderer::default(),
    )
    // The USB state doesn't trigger a redraw (see screen.rs), so poll for it
    .with_render_interval(Duration::from_secs(1));
    // The default config runs at a 1ms period, as in the ZMK shield
    let mut screen_controller = ScreenController::new(SimplePwm::new_1ch(p.PWM1, p.P1_11, &SimpleConfig::default()));

    run_all!(storage, usb_transport, ble_transport, keyboard, display, screen_controller).await;
}
