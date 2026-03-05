use core::convert::Infallible;
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::pixelcolor::raw::RawU16;
use embedded_graphics_core::pixelcolor::{Rgb565, RgbColor};
use embedded_hal::digital::OutputPin;
use mipidsi::interface::Interface as DisplayInterface;
use mipidsi::models::ILI9341Rgb565;
use slint::platform::software_renderer::{LineBufferProvider, Rgb565Pixel};

pub const DISPLAY_WIDTH: u16 = 320;
pub const DISPLAY_HEIGHT: u16 = 240;

pub struct DrawBuffer<'a, DI, RST>
where
    DI: DisplayInterface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
{
    pub display: mipidsi::Display<DI, ILI9341Rgb565, RST>,
    pub buffer: &'a mut [Rgb565Pixel],
}

impl<DI, RST> LineBufferProvider for &mut DrawBuffer<'_, DI, RST>
where
    DI: DisplayInterface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
{
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb565Pixel]),
    ) {
        let line_pixels = &mut self.buffer[range.clone()];
        render_fn(line_pixels);

        if range.is_empty() {
            return;
        }

        let _ = self.display.set_pixels(
            range.start as u16,
            line as u16,
            (range.end - 1) as u16,
            line as u16,
            line_pixels.iter().map(|pixel| RawU16::new(pixel.0).into()),
        );
    }
}

pub fn clear_display<DI, RST>(
    display: &mut mipidsi::Display<DI, ILI9341Rgb565, RST>,
) -> Result<(), DI::Error>
where
    DI: DisplayInterface<Word = u8>,
    RST: OutputPin<Error = Infallible>,
{
    display.clear(Rgb565::BLACK)
}
