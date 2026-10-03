use crate::Color;

const BYTES_PER_PIXEL: usize = 4;

/// Byte order of an eight-bit-per-channel framebuffer pixel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    /// Red, green, blue, unused.
    Rgb,
    /// Blue, green, red, unused.
    Bgr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceError {
    Empty,
    StrideTooSmall,
    SizeOverflow,
    BufferTooSmall,
}

/// A checked view over caller-owned pixel memory.
///
/// `stride` is measured in pixels and may be larger than `width`.
pub struct Surface<'pixels> {
    pixels: &'pixels mut [u8],
    width: u32,
    height: u32,
    stride: u32,
    format: PixelFormat,
}

impl<'pixels> Surface<'pixels> {
    /// Creates a checked view over `pixels`.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError`] when the dimensions are empty, the stride is
    /// narrower than the visible width, the byte length overflows, or the
    /// supplied storage is too small.
    pub fn new(
        pixels: &'pixels mut [u8],
        width: u32,
        height: u32,
        stride: u32,
        format: PixelFormat,
    ) -> Result<Self, SurfaceError> {
        if width == 0 || height == 0 {
            return Err(SurfaceError::Empty);
        }
        if stride < width {
            return Err(SurfaceError::StrideTooSmall);
        }

        let required = usize::try_from(stride)
            .ok()
            .and_then(|stride| stride.checked_mul(height as usize))
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL))
            .ok_or(SurfaceError::SizeOverflow)?;
        if pixels.len() < required {
            return Err(SurfaceError::BufferTooSmall);
        }

        Ok(Self {
            pixels: &mut pixels[..required],
            width,
            height,
            stride,
            format,
        })
    }

    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        self.format
    }

    pub fn clear(&mut self, color: Color) {
        for y in 0..self.height {
            for x in 0..self.width {
                self.write_opaque(x, y, color);
            }
        }
    }

    /// Blends a pixel using an additional 0..=255 coverage value.
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color, coverage: u8) {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
            return;
        };
        if x >= self.width || y >= self.height || color.alpha == 0 || coverage == 0 {
            return;
        }

        let alpha = multiply_u8(color.alpha, coverage);
        if alpha == 255 {
            self.write_opaque(x, y, color);
            return;
        }

        let offset = self.offset(x, y);
        let (red, green, blue) = match self.format {
            PixelFormat::Rgb => (
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
            ),
            PixelFormat::Bgr => (
                self.pixels[offset + 2],
                self.pixels[offset + 1],
                self.pixels[offset],
            ),
        };
        let output = Color::rgb(
            blend_channel(color.red, red, alpha),
            blend_channel(color.green, green, alpha),
            blend_channel(color.blue, blue, alpha),
        );
        self.write_opaque(x, y, output);
    }

    #[must_use]
    pub fn color_at(&self, x: u32, y: u32) -> Option<Color> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = self.offset(x, y);
        let (red, green, blue) = match self.format {
            PixelFormat::Rgb => (
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
            ),
            PixelFormat::Bgr => (
                self.pixels[offset + 2],
                self.pixels[offset + 1],
                self.pixels[offset],
            ),
        };
        Some(Color::rgb(red, green, blue))
    }

    fn write_opaque(&mut self, x: u32, y: u32, color: Color) {
        let offset = self.offset(x, y);
        let (first, third) = match self.format {
            PixelFormat::Rgb => (color.red, color.blue),
            PixelFormat::Bgr => (color.blue, color.red),
        };
        self.pixels[offset] = first;
        self.pixels[offset + 1] = color.green;
        self.pixels[offset + 2] = third;
        self.pixels[offset + 3] = 0;
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        (y as usize * self.stride as usize + x as usize) * BYTES_PER_PIXEL
    }
}

fn multiply_u8(left: u8, right: u8) -> u8 {
    let value = (u16::from(left) * u16::from(right) + 127) / 255;
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn blend_channel(source: u8, destination: u8, alpha: u8) -> u8 {
    let alpha = u16::from(alpha);
    let inverse = 255 - alpha;
    let value = (u16::from(source) * alpha + u16::from(destination) * inverse + 127) / 255;
    u8::try_from(value).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_layouts() {
        let mut pixels = [0; 16];
        assert!(matches!(
            Surface::new(&mut pixels, 2, 2, 1, PixelFormat::Rgb),
            Err(SurfaceError::StrideTooSmall)
        ));
        assert!(matches!(
            Surface::new(&mut pixels[..15], 2, 2, 2, PixelFormat::Rgb),
            Err(SurfaceError::BufferTooSmall)
        ));
    }

    #[test]
    fn respects_pixel_format_and_stride() {
        let mut pixels = [0x55; 24];
        {
            let mut surface = Surface::new(&mut pixels, 2, 2, 3, PixelFormat::Bgr).unwrap();
            surface.clear(Color::rgb(0x11, 0x22, 0x33));
            assert_eq!(surface.color_at(1, 1), Some(Color::rgb(0x11, 0x22, 0x33)));
        }
        assert_eq!(&pixels[20..24], &[0x55; 4]);
    }

    #[test]
    fn blends_alpha_and_coverage() {
        let mut pixels = [0; 4];
        let mut surface = Surface::new(&mut pixels, 1, 1, 1, PixelFormat::Rgb).unwrap();
        surface.clear(Color::BLACK);
        surface.blend_pixel(0, 0, Color::rgba(255, 255, 255, 128), 128);
        assert_eq!(surface.color_at(0, 0), Some(Color::rgb(64, 64, 64)));
    }

    #[test]
    fn clips_pixels_outside_surface() {
        let mut pixels = [0; 4];
        let mut surface = Surface::new(&mut pixels, 1, 1, 1, PixelFormat::Rgb).unwrap();
        surface.blend_pixel(-1, 0, Color::WHITE, 255);
        surface.blend_pixel(1, 0, Color::WHITE, 255);
        assert_eq!(surface.color_at(0, 0), Some(Color::BLACK));
    }
}
