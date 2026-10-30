//! The dongle's screen: the 1.69" 240x280 ST7789V LCD of the ZMK `dongle_screen` shield
//! (janpfischer/zmk-dongle-screen 0.1.0), used in landscape as there.
//!
//! Wiring on the XIAO BLE, from the shield's `seeeduino_xiao_ble.overlay`: SPI3 SCK P1.13 (D8),
//! MOSI P1.15 (D10), CS P1.14 (D9), DC P1.12 (D7), RST P0.29 (D3), backlight PWM P1.11 (D6).
//!
//! [`ScreenRenderer`] draws the output, layer, modifiers and both halves' batteries.
//! [`ScreenController`] runs the backlight (80%, off after 5 minutes without a key press, as in
//! ZMK) and tracks which transport is active, which `RenderContext` doesn't carry.

use core::fmt::Write as _;
use core::sync::atomic::{AtomicU8, Ordering};

use embassy_nrf::pwm::{DutyCycle, SimplePwm};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use heapless::String;
use rmk::core_traits::Runnable;
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::embassy_futures::select::{Either3, select3};
use rmk::embassy_time::{Duration, Instant, Timer};
use rmk::event::{ConnectionStatusChangeEvent, ConnectionType, EventSubscriber, KeyboardEvent, SubscribableEvent};
use rmk::types::battery::{BatteryStatus, ChargeState};
use rmk::types::ble::BleState;
use rmk::types::modifier::ModifierCombination;

/// Landscape size of the panel
pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 240;

/// ZMK: CONFIG_DONGLE_SCREEN_MAX_BRIGHTNESS
const BRIGHTNESS_PERCENT: u32 = 80;
/// ZMK: CONFIG_DONGLE_SCREEN_IDLE_TIMEOUT_S
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Indexed by layer, see `keymap.rs`
const LAYER_NAMES: [&str; 7] = ["WIN", "SYMBOL", "NAV", "MAC", "SYM_MAC", "NAV_MAC", "ADJUST"];

const BACKGROUND: Rgb565 = Rgb565::BLACK;
const FOREGROUND: Rgb565 = Rgb565::WHITE;
const DIM: Rgb565 = Rgb565::new(10, 20, 10);
const ACCENT: Rgb565 = Rgb565::new(4, 24, 28);

const OUTPUT_AREA: Rectangle = Rectangle::new(Point::new(0, 0), Size::new(WIDTH as u32, 44));
const LAYER_AREA: Rectangle = Rectangle::new(Point::new(0, 52), Size::new(WIDTH as u32, 56));
const MODIFIERS_TOP: i32 = 120;
const BATTERY_TOP: i32 = 172;

/// Active transport, set by [`ScreenController`]
static ACTIVE_OUTPUT: AtomicU8 = AtomicU8::new(NO_OUTPUT);
const NO_OUTPUT: u8 = 0;
const USB_OUTPUT: u8 = 1;
const BLE_OUTPUT: u8 = 2;

/// Drives the backlight and tracks the active transport for the renderer.
pub struct ScreenController {
    backlight: SimplePwm<'static>,
}

impl ScreenController {
    /// `backlight` must run at a 1ms period, the default `SimpleConfig`.
    pub fn new(backlight: SimplePwm<'static>) -> Self {
        let mut controller = Self { backlight };
        controller.set_backlight(true);
        controller
    }

    fn set_backlight(&mut self, on: bool) {
        let max = u32::from(self.backlight.max_duty());
        let duty = if on { max * BRIGHTNESS_PERCENT / 100 } else { 0 };
        // The backlight is active high, and `inverted` keeps the pin high for the first `duty`
        // counts of each period (ZMK: PWM_POLARITY_NORMAL)
        self.backlight.set_duty(0, DutyCycle::inverted(duty as u16));
    }
}

impl Runnable for ScreenController {
    async fn run(&mut self) -> ! {
        let mut keys = KeyboardEvent::subscriber();
        let mut connection = ConnectionStatusChangeEvent::subscriber();
        let mut lit = true;
        let mut off_at = Instant::now() + IDLE_TIMEOUT;
        loop {
            let deadline = lit.then_some(off_at);
            let idle = async move {
                match deadline {
                    Some(at) => Timer::at(at).await,
                    None => core::future::pending().await,
                }
            };
            let event = select3(keys.next_event(), connection.next_event(), idle).await;
            match event {
                Either3::First(_) => {
                    off_at = Instant::now() + IDLE_TIMEOUT;
                    if !lit {
                        self.set_backlight(true);
                        lit = true;
                    }
                }
                Either3::Second(ConnectionStatusChangeEvent(status)) => {
                    let output = match status.decide_active() {
                        Some(ConnectionType::Usb) => USB_OUTPUT,
                        Some(ConnectionType::Ble) => BLE_OUTPUT,
                        None => NO_OUTPUT,
                    };
                    ACTIVE_OUTPUT.store(output, Ordering::Relaxed);
                }
                Either3::Third(()) => {
                    self.set_backlight(false);
                    lit = false;
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Usb,
    Ble(u8),
    Advertising(u8),
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Half {
    connected: bool,
    battery: BatteryStatus,
}

/// What is on the screen, so that only the parts that changed are redrawn: the LCD driver
/// flushes only the area drawn since the last flush.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Shown {
    output: Output,
    layer: u8,
    modifiers: ModifierCombination,
    halves: [Half; 2],
}

impl Shown {
    fn new(ctx: &RenderContext) -> Self {
        let ble = ctx.ble_status;
        let output = match ACTIVE_OUTPUT.load(Ordering::Relaxed) {
            USB_OUTPUT => Output::Usb,
            BLE_OUTPUT => Output::Ble(ble.profile),
            _ if ble.state == BleState::Advertising => Output::Advertising(ble.profile),
            _ => Output::None,
        };
        Self {
            output,
            layer: ctx.layer,
            modifiers: ctx.modifiers,
            halves: core::array::from_fn(|id| Half {
                connected: ctx.peripherals_connected[id],
                battery: ctx.peripheral_batteries[id].0,
            }),
        }
    }
}

#[derive(Default)]
pub struct ScreenRenderer {
    shown: Option<Shown>,
}

impl DisplayRenderer<Rgb565> for ScreenRenderer {
    fn render<D: DrawTarget<Color = Rgb565>>(&mut self, ctx: &RenderContext, display: &mut D) {
        let new = Shown::new(ctx);
        let old = self.shown.replace(new);
        if old.is_none() {
            display.clear(BACKGROUND).ok();
        }
        if old.map(|o| o.output) != Some(new.output) {
            draw_output(display, new.output);
        }
        if old.map(|o| o.layer) != Some(new.layer) {
            draw_layer(display, new.layer);
        }
        if old.map(|o| o.modifiers) != Some(new.modifiers) {
            draw_modifiers(display, new.modifiers);
        }
        for (id, (half, name)) in new.halves.iter().zip(["L", "R"]).enumerate() {
            if old.map(|o| o.halves[id]) != Some(*half) {
                draw_battery(display, id, name, half);
            }
        }
    }
}

fn centered_text<D: DrawTarget<Color = Rgb565>>(display: &mut D, text: &str, center: Point, color: Rgb565) {
    let character_style = MonoTextStyle::new(&FONT_10X20, color);
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    Text::with_text_style(text, center, character_style, text_style)
        .draw(display)
        .ok();
}

fn draw_box<D: DrawTarget<Color = Rgb565>>(display: &mut D, area: Rectangle, fill: Rgb565, stroke: Rgb565) {
    let style = PrimitiveStyleBuilder::new()
        .fill_color(fill)
        .stroke_color(stroke)
        .stroke_width(2)
        .build();
    area.into_styled(style).draw(display).ok();
}

fn draw_output<D: DrawTarget<Color = Rgb565>>(display: &mut D, output: Output) {
    display.fill_solid(&OUTPUT_AREA, BACKGROUND).ok();
    let mut text: String<16> = String::new();
    let color = match output {
        Output::Usb => {
            text.push_str("USB").ok();
            Rgb565::GREEN
        }
        Output::Ble(profile) => {
            write!(text, "BT {}", profile + 1).ok();
            Rgb565::CYAN
        }
        Output::Advertising(profile) => {
            write!(text, "BT {} ...", profile + 1).ok();
            Rgb565::YELLOW
        }
        Output::None => {
            text.push_str("NO HOST").ok();
            Rgb565::RED
        }
    };
    centered_text(display, &text, OUTPUT_AREA.center(), color);
}

fn draw_layer<D: DrawTarget<Color = Rgb565>>(display: &mut D, layer: u8) {
    display.fill_solid(&LAYER_AREA, BACKGROUND).ok();
    let mut text: String<16> = String::new();
    match LAYER_NAMES.get(usize::from(layer)) {
        Some(name) => text.push_str(name).ok(),
        None => write!(text, "LAYER {layer}").ok(),
    };
    // Base layers (WIN, MAC) plain, the rest highlighted
    let fill = if matches!(layer, 0 | 3) { BACKGROUND } else { ACCENT };
    let pill = Rectangle::new(Point::new(40, LAYER_AREA.top_left.y), Size::new(200, LAYER_AREA.size.height));
    draw_box(display, pill, fill, FOREGROUND);
    centered_text(display, &text, pill.center(), FOREGROUND);
}

fn draw_modifiers<D: DrawTarget<Color = Rgb565>>(display: &mut D, modifiers: ModifierCombination) {
    let m = modifiers;
    let held = [
        ("CTRL", m.left_ctrl() || m.right_ctrl()),
        ("SHFT", m.left_shift() || m.right_shift()),
        ("ALT", m.left_alt() || m.right_alt()),
        ("GUI", m.left_gui() || m.right_gui()),
    ];
    for (i, (label, on)) in held.into_iter().enumerate() {
        let cell = Rectangle::new(Point::new(i as i32 * 70 + 4, MODIFIERS_TOP), Size::new(62, 36));
        let (fill, color) = if on { (ACCENT, FOREGROUND) } else { (BACKGROUND, DIM) };
        draw_box(display, cell, fill, color);
        centered_text(display, label, cell.center(), color);
    }
}

fn draw_battery<D: DrawTarget<Color = Rgb565>>(display: &mut D, id: usize, name: &str, half: &Half) {
    let left = id as i32 * 140;
    let area = Rectangle::new(Point::new(left, BATTERY_TOP), Size::new(140, HEIGHT as u32 - BATTERY_TOP as u32));
    display.fill_solid(&area, BACKGROUND).ok();

    let mut text: String<16> = String::new();
    let level = match half.battery {
        BatteryStatus::Available { level, charge_state } if half.connected => {
            match level {
                Some(level) => write!(text, "{name} {level}%").ok(),
                None => write!(text, "{name} ?").ok(),
            };
            if charge_state == ChargeState::Charging {
                text.push_str(" +").ok();
            }
            level
        }
        _ => {
            write!(text, "{name} --").ok();
            None
        }
    };
    let color = if half.connected { FOREGROUND } else { DIM };
    centered_text(display, &text, Point::new(left + 70, BATTERY_TOP + 16), color);

    let bar = Rectangle::new(Point::new(left + 20, BATTERY_TOP + 34), Size::new(100, 20));
    draw_box(display, bar, BACKGROUND, color);
    if let Some(level) = level {
        let level = u32::from(level.min(100));
        let fill_color = match level {
            51.. => Rgb565::GREEN,
            21..=50 => Rgb565::YELLOW,
            _ => Rgb565::RED,
        };
        let fill = Rectangle::new(bar.top_left + Point::new(3, 3), Size::new(94 * level / 100, 14));
        display.fill_solid(&fill, fill_color).ok();
    }
}
