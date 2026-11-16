//! Display driver with a 4-bit palette framebuffer, for RMK's `DisplayProcessor`.
//!
//! RMK's `LcdAsyncDisplay` keeps a full RGB565 framebuffer: 131K for the 280x240 panel, half the
//! nRF52840's RAM, which left too little for the stack (it overflowed into the statics). The screen
//! only uses a few colors, so this one stores a palette index per pixel (33K) and converts the
//! dirty area to RGB565 a few rows at a time when flushing.

use core::convert::Infallible;

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::{RawData, RawU16};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_hal::digital::OutputPin;
use rmk::display::DisplayDriver;
use rmk::display::lcd_async::Display;
use rmk::display::lcd_async::interface::Interface;
use rmk::display::lcd_async::models::Model;

/// Rows converted and sent per transfer when the dirty area is full width
pub const STAGING_ROWS: usize = 16;

pub struct PaletteDisplay<DI, MOD, RST, const W: usize, const H: usize>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    display: Display<DI, MOD, RST>,
    /// At most 16 colors. Index 0 is what the zeroed framebuffer starts as.
    palette: &'static [Rgb565],
    /// Palette indices, two pixels per byte: the even pixel in the high nibble
    pixels: &'static mut [u8],
    /// RGB565 (big endian, as the panel takes it) of the rows being sent
    staging: &'static mut [u8],
    /// Union of the areas drawn since the last flush
    dirty: Option<Rectangle>,
}

impl<DI, MOD, RST, const W: usize, const H: usize> PaletteDisplay<DI, MOD, RST, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    /// `display` must be initialized. `pixels` is `W * H / 2` bytes, `staging` at least one row
    /// (`W * 2` bytes). Colors missing from `palette` are drawn as the closest one.
    pub fn new(
        display: Display<DI, MOD, RST>,
        palette: &'static [Rgb565],
        pixels: &'static mut [u8],
        staging: &'static mut [u8],
    ) -> Self {
        assert!(!palette.is_empty() && palette.len() <= 16);
        assert!(pixels.len() == W * H / 2 && (W * H) % 2 == 0);
        assert!(staging.len() >= W * 2);
        Self {
            display,
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

impl<DI, MOD, RST, const W: usize, const H: usize> OriginDimensions for PaletteDisplay<DI, MOD, RST, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

impl<DI, MOD, RST, const W: usize, const H: usize> DrawTarget for PaletteDisplay<DI, MOD, RST, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
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

impl<DI, MOD, RST, const W: usize, const H: usize> DisplayDriver for PaletteDisplay<DI, MOD, RST, W, H>
where
    DI: Interface<Word = u8>,
    MOD: Model<ColorFormat = Rgb565>,
    RST: OutputPin,
{
    async fn init(&mut self) {}

    async fn flush(&mut self) {
        let Some(dirty) = self.dirty else {
            return;
        };
        let x0 = dirty.top_left.x as usize;
        let y0 = dirty.top_left.y as usize;
        let width = dirty.size.width as usize;
        let height = dirty.size.height as usize;
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
            let data = &self.staging[..rows * width * 2];
            if self
                .display
                .show_raw_data(x0 as u16, y as u16, width as u16, rows as u16, data)
                .await
                .is_err()
            {
                // Stays dirty, so the next flush retries
                return;
            }
            y += rows;
        }
        self.dirty = None;
    }
}
