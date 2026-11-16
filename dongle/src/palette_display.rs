//! ST7789 driver with a 4-bit palette framebuffer, for RMK's `DisplayProcessor`.
//!
//! RMK's `LcdAsyncDisplay` keeps a full RGB565 framebuffer: 131K for the 280x240 panel, half the
//! nRF52840's RAM, which left too little for the stack (it overflowed into the statics). The screen
//! only uses a few colors, so this one stores a palette index per pixel (33K).
//!
//! It drives the panel directly instead of through lcd-async: lcd-async can only send pixels as
//! separate windows (`show_raw_data`), and the panel stayed black when the screen was sent as
//! several windows, while one window per flush (what `LcdAsyncDisplay` does) works. So each flush
//! opens one window and streams it in chunks with chip select held low, like one long transfer.
//! The init sequence is lcd-async's for the ST7789.

use core::convert::Infallible;

use embassy_nrf::gpio::Output;
use embassy_nrf::spim::Spim;
use embassy_time::Timer;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::{RawData, RawU16};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use rmk::display::DisplayDriver;

/// Rows converted and sent per SPI write when the dirty area is full width
pub const STAGING_ROWS: usize = 16;

/// MADCTL for landscape (lcd-async `Rotation::Deg90`): columns reversed, rows and columns swapped,
/// RGB order
const ADDRESS_MODE: u8 = 0x60;
/// The 280-pixel side of the panel sits 20 pixels into the controller's 320 (lcd-async
/// `display_offset(0, 20)` in portrait, which becomes a column offset in landscape)
const COLUMN_OFFSET: u16 = 20;

pub struct PaletteDisplay<const W: usize, const H: usize> {
    spi: Spim<'static>,
    cs: Output<'static>,
    dc: Output<'static>,
    reset: Output<'static>,
    /// At most 16 colors. Index 0 is what the zeroed framebuffer starts as.
    palette: &'static [Rgb565],
    /// Palette indices, two pixels per byte: the even pixel in the high nibble
    pixels: &'static mut [u8],
    /// RGB565 (big endian, as the panel takes it) of the rows being sent
    staging: &'static mut [u8],
    /// Union of the areas drawn since the last flush
    dirty: Option<Rectangle>,
}

impl<const W: usize, const H: usize> PaletteDisplay<W, H> {
    /// `cs` and `reset` must start high. The panel is set up in `init`, which `DisplayProcessor`
    /// calls before the first render. `pixels` is `W * H / 2` bytes, `staging` at least one row
    /// (`W * 2` bytes). Colors missing from `palette` are drawn as the closest one.
    pub fn new(
        spi: Spim<'static>,
        cs: Output<'static>,
        dc: Output<'static>,
        reset: Output<'static>,
        palette: &'static [Rgb565],
        pixels: &'static mut [u8],
        staging: &'static mut [u8],
    ) -> Self {
        assert!(!palette.is_empty() && palette.len() <= 16);
        assert!(pixels.len() == W * H / 2 && (W * H) % 2 == 0);
        assert!(staging.len() >= W * 2);
        Self {
            spi,
            cs,
            dc,
            reset,
            palette,
            pixels,
            staging,
            // The panel's RAM holds garbage until the first full flush
            dirty: Some(Self::screen()),
        }
    }

    fn screen() -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(W as u32, H as u32))
    }

    /// One command and its parameters in its own chip select cycle
    async fn command(&mut self, command: u8, params: &[u8]) {
        self.cs.set_low();
        self.dc.set_low();
        self.spi.write(&[command]).await.ok();
        self.dc.set_high();
        if !params.is_empty() {
            self.spi.write(params).await.ok();
        }
        self.cs.set_high();
    }

    fn index(&self, color: Rgb565) -> u8 {
        if let Some(i) = self.palette.iter().position(|&c| c == color) {
            return i as u8;
        }
        let distance = |c: Rgb565| {
            let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
            d(c.r(), color.r()) + d(c.g(), color.g()) + d(c.b(), color.b())
        };
        let closest = (0..self.palette.len()).min_by_key(|&i| distance(self.palette[i]));
        closest.unwrap_or(0) as u8
    }

    fn set(&mut self, x: usize, y: usize, index: u8) {
        let i = y * W + x;
        let byte = &mut self.pixels[i / 2];
        *byte = if i % 2 == 0 {
            (*byte & 0x0F) | (index << 4)
        } else {
            (*byte & 0xF0) | index
        };
    }

    fn get(&self, x: usize, y: usize) -> u8 {
        let i = y * W + x;
        let byte = self.pixels[i / 2];
        if i % 2 == 0 { byte >> 4 } else { byte & 0x0F }
    }

    /// Merge `area` (already clipped to the screen) into the dirty region
    fn mark_dirty(&mut self, area: Rectangle) {
        let Some(area_br) = area.bottom_right() else {
            return;
        };
        self.dirty = Some(match self.dirty {
            None => area,
            Some(d) => Rectangle::with_corners(
                d.top_left.component_min(area.top_left),
                d.bottom_right().unwrap_or(d.top_left).component_max(area_br),
            ),
        });
    }
}

impl<const W: usize, const H: usize> OriginDimensions for PaletteDisplay<W, H> {
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

impl<const W: usize, const H: usize> DrawTarget for PaletteDisplay<W, H> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        let mut min = Point::new(i32::MAX, i32::MAX);
        let mut max = Point::new(i32::MIN, i32::MIN);
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 && (point.x as usize) < W && (point.y as usize) < H {
                let index = self.index(color);
                self.set(point.x as usize, point.y as usize, index);
                min = min.component_min(point);
                max = max.component_max(point);
            }
        }
        if min.x <= max.x {
            self.mark_dirty(Rectangle::with_corners(min, max));
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        let area = area.intersection(&Self::screen());
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        let index = self.index(color);
        for y in area.top_left.y..=bottom_right.y {
            for x in area.top_left.x..=bottom_right.x {
                self.set(x as usize, y as usize, index);
            }
        }
        self.mark_dirty(area);
        Ok(())
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Self::Error> {
        let index = self.index(color);
        self.pixels.fill((index << 4) | index);
        self.mark_dirty(Self::screen());
        Ok(())
    }
}

impl<const W: usize, const H: usize> DisplayDriver for PaletteDisplay<W, H> {
    /// Reset and set up the panel, as lcd-async's ST7789 model does
    async fn init(&mut self) {
        self.reset.set_low();
        Timer::after_millis(10).await;
        self.reset.set_high();
        Timer::after_millis(150).await;
        // Sleep out
        self.command(0x11, &[]).await;
        Timer::after_millis(10).await;
        self.command(0x36, &[ADDRESS_MODE]).await;
        // Inversion on: Zephyr's st7789v driver always turns it on for this panel
        self.command(0x21, &[]).await;
        // 16 bits per pixel
        self.command(0x3A, &[0x55]).await;
        Timer::after_millis(10).await;
        // Normal display mode
        self.command(0x13, &[]).await;
        Timer::after_millis(10).await;
        // Display on
        self.command(0x29, &[]).await;
        Timer::after_millis(120).await;
    }

    async fn flush(&mut self) {
        let Some(dirty) = self.dirty.take() else {
            return;
        };
        let x0 = dirty.top_left.x as usize;
        let y0 = dirty.top_left.y as usize;
        let width = dirty.size.width as usize;
        let height = dirty.size.height as usize;

        // One window for the whole dirty area
        let [xs_hi, xs_lo] = (x0 as u16 + COLUMN_OFFSET).to_be_bytes();
        let [xe_hi, xe_lo] = ((x0 + width - 1) as u16 + COLUMN_OFFSET).to_be_bytes();
        let [ys_hi, ys_lo] = (y0 as u16).to_be_bytes();
        let [ye_hi, ye_lo] = ((y0 + height - 1) as u16).to_be_bytes();
        self.command(0x2A, &[xs_hi, xs_lo, xe_hi, xe_lo]).await;
        self.command(0x2B, &[ys_hi, ys_lo, ye_hi, ye_lo]).await;

        // Memory write, then the pixels in chunks without releasing chip select
        self.cs.set_low();
        self.dc.set_low();
        self.spi.write(&[0x2C]).await.ok();
        self.dc.set_high();
        let rows_per_pass = self.staging.len() / (width * 2);
        let mut y = y0;
        while y < y0 + height {
            let rows = rows_per_pass.min(y0 + height - y);
            for row in 0..rows {
                for col in 0..width {
                    let color = self.palette[usize::from(self.get(x0 + col, y + row))];
                    let at = (row * width + col) * 2;
                    self.staging[at..at + 2].copy_from_slice(&RawU16::from(color).into_inner().to_be_bytes());
                }
            }
            self.spi.write(&self.staging[..rows * width * 2]).await.ok();
            y += rows;
        }
        self.cs.set_high();
    }
}
