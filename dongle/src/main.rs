//! USB dongle on a Seeed XIAO BLE (nRF52840): split central. Runs the keymap
//! and Vial, receives both halves over BLE, and sends reports to the host over
//! USB (or a BLE host). It has no matrix of its own, but drives a screen.

#![no_std]
#![no_main]

mod keymap;
mod nb;
mod palette_display;
mod screen;
mod vial;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, SPI2, USBD};
use embassy_nrf::pwm::{SimpleConfig, SimplePwm};
use embassy_nrf::spim::{self, Spim};
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{bind_interrupts, rng, usb};
use embassy_time::Duration;
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use palette_display::PaletteDisplay;
use rmk::ble::BleTransport;
use rmk::config::{
    BehaviorConfig, DeviceConfig, MouseKeyConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig,
};
use rmk::display::DisplayProcessor;
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
    SPI2 => spim::InterruptHandler<SPI2>;
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
        // +8dBm, the nRF52840's maximum, as the halves (ZMK: CONFIG_BT_CTLR_TX_PWR_PLUS_8). At the
        // default 0dBm the link to the halves failed to establish (sync timeout).
        .default_tx_power(8)?
        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
        .build(p, rng, mpsl, mem)
}

/// Fit both split links into one 7.5ms connection interval.
///
/// The SoftDevice Controller defaults to a 7.5ms event length and 7.5ms central ACL event spacing.
/// With both halves at a 7.5ms interval, their events then land on top of each other every time,
/// and the scheduler keeps skipping one link: new connections failed with sync timeout (`0x3E`)
/// and established ones with supervision timeout (`0x08`). ZMK (Zephyr's own controller) doesn't
/// reserve time like this. 2.5ms per event spaced 3.75ms apart interleaves the two links and
/// leaves time for scanning and the BLE host advertiser.
///
/// Set once after enabling the controller, as nRF Connect SDK does: the values survive the HCI
/// Reset that the host sends at startup.
fn set_sdc_event_timing() {
    let event_length = sdc::raw::sdc_hci_cmd_vs_event_length_set_t { event_length_us: 2500 };
    let spacing = sdc::raw::sdc_hci_cmd_vs_central_acl_event_spacing_set_t { central_acl_event_spacing_us: 3750 };
    // SAFETY: plain HCI command calls with valid parameter structs, after `sdc_enable`
    unsafe {
        assert_eq!(sdc::raw::sdc_hci_cmd_vs_event_length_set(&event_length), 0);
        assert_eq!(sdc::raw::sdc_hci_cmd_vs_central_acl_event_spacing_set(&spacing), 0);
    }
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
    set_sdc_event_timing();

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
    behavior_config.tri_layer_2 = Some(keymap::TRI_LAYER_MAC);
    // Not `unwrap!`: the error holds the profile, which only implements `defmt::Format` with rmk's
    // `defmt` feature (off while `usb_log` is on)
    assert!(behavior_config.morse.profiles.push(keymap::HOLD_TAP_PROFILE).is_ok());
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

    // Screen: wiring and layout are in screen.rs. On SPIM2, not SPIM3 (which ZMK uses at 16MHz):
    // nRF52840 errata 198, SPIM3 can send corrupted bytes when the CPU or another EasyDMA user (the
    // radio) touches the same RAM block as its TX buffer. A corrupted window command drew the
    // modifiers row over the output row. SPIM0-2 top out at 8MHz. The panel stayed black at 32MHz.
    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M8;
    // Below the priorities the BLE stack reserves
    interrupt::SPI2.set_priority(interrupt::Priority::P3);
    // 4-bit palette framebuffer (33K): a full RGB565 one (131K) left too little RAM for the stack.
    // Const-initialized so that they never pass through the stack.
    static PIXELS: ConstStaticCell<[u8; screen::WIDTH * screen::HEIGHT / 2]> =
        ConstStaticCell::new([0; screen::WIDTH * screen::HEIGHT / 2]);
    static STAGING: ConstStaticCell<[u8; screen::WIDTH * 2 * palette_display::STAGING_ROWS]> =
        ConstStaticCell::new([0; screen::WIDTH * 2 * palette_display::STAGING_ROWS]);
    // The panel is reset and set up by `DisplayProcessor`, before it first draws
    let lcd_display = PaletteDisplay::<{ screen::WIDTH }, { screen::HEIGHT }>::new(
        Spim::new_txonly(p.SPI2, Irqs, p.P1_13, p.P1_15, spi_config),
        Output::new(p.P1_14, Level::High, OutputDrive::Standard),
        Output::new(p.P1_12, Level::Low, OutputDrive::Standard),
        Output::new(p.P0_29, Level::High, OutputDrive::Standard),
        &screen::PALETTE,
        PIXELS.take(),
        STAGING.take(),
    );
    let mut display = DisplayProcessor::with_renderer(lcd_display, ScreenRenderer::default())
        // Neither the USB state nor Caps Word (on/off, its timeout) triggers a redraw (see screen.rs),
        // so poll for them. A render that changes nothing sends nothing to the panel.
        .with_render_interval(Duration::from_millis(250));
    // The default config runs at a 1ms period, as in the ZMK shield
    let mut screen_controller = ScreenController::new(SimplePwm::new_1ch(p.PWM1, p.P1_11, &SimpleConfig::default()));

    run_all!(storage, usb_transport, ble_transport, keyboard, display, screen_controller).await;
}
