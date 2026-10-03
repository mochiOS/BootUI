use crate::{Color, Point, Rect, Surface};

const CHANNELS: usize = 4;
const FRACTION_BITS: u32 = 16;
const FRACTION_ONE: u64 = 1 << FRACTION_BITS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageError {
    Empty,
    StrideTooSmall,
    SizeOverflow,
    BufferTooSmall,
}

/// A checked view over unpremultiplied RGBA8 pixels.
#[derive(Clone, Copy)]
pub struct Image<'pixels> {
    pixels: &'pixels [u8],
    width: u32,
    height: u32,
    stride: usize,
}

impl<'pixels> Image<'pixels> {
    /// Creates an image whose `stride` is measured in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError`] when the dimensions are empty, the stride is too
    /// narrow, the byte length overflows, or `pixels` is too small.
    pub fn new(
        pixels: &'pixels [u8],
        width: u32,
        height: u32,
        stride: usize,
    ) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::Empty);
        }
        let row_bytes = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(CHANNELS))
            .ok_or(ImageError::SizeOverflow)?;
        if stride < row_bytes {
            return Err(ImageError::StrideTooSmall);
        }
        let required = stride
            .checked_mul(height as usize)
            .ok_or(ImageError::SizeOverflow)?;
        if pixels.len() < required {
            return Err(ImageError::BufferTooSmall);
        }
        Ok(Self {
            pixels: &pixels[..required],
            width,
            height,
            stride,
        })
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    fn color_at(self, x: u32, y: u32) -> Color {
        let offset = y as usize * self.stride + x as usize * CHANNELS;
        Color::rgba(
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        )
    }

    fn bilinear_color(self, x: u64, y: u64) -> Color {
        let x0 = u32::try_from(x >> FRACTION_BITS)
            .unwrap_or(u32::MAX)
            .min(self.width - 1);
        let y0 = u32::try_from(y >> FRACTION_BITS)
            .unwrap_or(u32::MAX)
            .min(self.height - 1);
        let x1 = x0.saturating_add(1).min(self.width - 1);
        let y1 = y0.saturating_add(1).min(self.height - 1);
        let fraction_x = x & (FRACTION_ONE - 1);
        let fraction_y = y & (FRACTION_ONE - 1);
        let top_left = self.color_at(x0, y0);
        let top_right = self.color_at(x1, y0);
        let bottom_left = self.color_at(x0, y1);
        let bottom_right = self.color_at(x1, y1);

        Color::rgba(
            bilinear_channel(
                top_left.red,
                top_right.red,
                bottom_left.red,
                bottom_right.red,
                fraction_x,
                fraction_y,
            ),
            bilinear_channel(
                top_left.green,
                top_right.green,
                bottom_left.green,
                bottom_right.green,
                fraction_x,
                fraction_y,
            ),
            bilinear_channel(
                top_left.blue,
                top_right.blue,
                bottom_left.blue,
                bottom_right.blue,
                fraction_x,
                fraction_y,
            ),
            bilinear_channel(
                top_left.alpha,
                top_right.alpha,
                bottom_left.alpha,
                bottom_right.alpha,
                fraction_x,
                fraction_y,
            ),
        )
    }

    fn filtered_color(
        self,
        destination_x: u64,
        destination_y: u64,
        destination_width: u32,
        destination_height: u32,
        samples_x: u32,
        samples_y: u32,
    ) -> Color {
        let mut alpha_sum = 0_u64;
        let mut red_sum = 0_u64;
        let mut green_sum = 0_u64;
        let mut blue_sum = 0_u64;
        for sample_y in 0..samples_y {
            let virtual_y = destination_y
                .saturating_mul(u64::from(samples_y))
                .saturating_add(u64::from(sample_y));
            let source_y = source_coordinate(
                virtual_y,
                destination_height.saturating_mul(samples_y),
                self.height(),
            );
            for sample_x in 0..samples_x {
                let virtual_x = destination_x
                    .saturating_mul(u64::from(samples_x))
                    .saturating_add(u64::from(sample_x));
                let source_x = source_coordinate(
                    virtual_x,
                    destination_width.saturating_mul(samples_x),
                    self.width(),
                );
                let color = self.bilinear_color(source_x, source_y);
                let alpha = u64::from(color.alpha);
                alpha_sum += alpha;
                red_sum += u64::from(color.red) * alpha;
                green_sum += u64::from(color.green) * alpha;
                blue_sum += u64::from(color.blue) * alpha;
            }
        }
        if alpha_sum == 0 {
            return Color::TRANSPARENT;
        }
        let sample_count = u64::from(samples_x) * u64::from(samples_y);
        Color::rgba(
            rounded_u8(red_sum, alpha_sum),
            rounded_u8(green_sum, alpha_sum),
            rounded_u8(blue_sum, alpha_sum),
            rounded_u8(alpha_sum, sample_count),
        )
    }
}

impl Surface<'_> {
    /// Draws an RGBA image without scaling.
    pub fn draw_image(&mut self, image: Image<'_>, origin: Point) {
        for source_y in 0..image.height() {
            let destination_y = i64::from(origin.y) + i64::from(source_y);
            if destination_y < 0 || destination_y >= i64::from(self.height()) {
                continue;
            }
            for source_x in 0..image.width() {
                let destination_x = i64::from(origin.x) + i64::from(source_x);
                if destination_x < 0 || destination_x >= i64::from(self.width()) {
                    continue;
                }
                self.blend_pixel(
                    saturating_i64_to_i32(destination_x),
                    saturating_i64_to_i32(destination_y),
                    image.color_at(source_x, source_y),
                    u8::MAX,
                );
            }
        }
    }

    /// Draws an RGBA image into `destination` with fixed-point bilinear
    /// filtering. This keeps logos and icons smooth when display scale changes.
    pub fn draw_image_scaled(&mut self, image: Image<'_>, destination: Rect) {
        if destination.is_empty() {
            return;
        }
        let left = i64::from(destination.origin.x).max(0);
        let top = i64::from(destination.origin.y).max(0);
        let right = (i64::from(destination.origin.x) + i64::from(destination.size.width))
            .min(i64::from(self.width()));
        let bottom = (i64::from(destination.origin.y) + i64::from(destination.size.height))
            .min(i64::from(self.height()));
        if left >= right || top >= bottom {
            return;
        }
        let samples_x = image.width().div_ceil(destination.size.width).clamp(1, 8);
        let samples_y = image.height().div_ceil(destination.size.height).clamp(1, 8);

        for destination_y in top..bottom {
            let local_y =
                u64::try_from(destination_y - i64::from(destination.origin.y)).unwrap_or_default();
            for destination_x in left..right {
                let local_x = u64::try_from(destination_x - i64::from(destination.origin.x))
                    .unwrap_or_default();
                self.blend_pixel(
                    saturating_i64_to_i32(destination_x),
                    saturating_i64_to_i32(destination_y),
                    image.filtered_color(
                        local_x,
                        local_y,
                        destination.size.width,
                        destination.size.height,
                        samples_x,
                        samples_y,
                    ),
                    u8::MAX,
                );
            }
        }
    }
}

fn source_coordinate(destination: u64, destination_size: u32, source_size: u32) -> u64 {
    let numerator = (destination.saturating_mul(2).saturating_add(1))
        .saturating_mul(u64::from(source_size))
        .saturating_mul(FRACTION_ONE);
    let denominator = u64::from(destination_size).saturating_mul(2);
    let centered = numerator / denominator;
    let coordinate = centered.saturating_sub(FRACTION_ONE / 2);
    coordinate.min(u64::from(source_size - 1) << FRACTION_BITS)
}

fn bilinear_channel(
    top_left: u8,
    top_right: u8,
    bottom_left: u8,
    bottom_right: u8,
    fraction_x: u64,
    fraction_y: u64,
) -> u8 {
    let top = interpolate(top_left, top_right, fraction_x);
    let bottom = interpolate(bottom_left, bottom_right, fraction_x);
    interpolate(top, bottom, fraction_y)
}

fn interpolate(left: u8, right: u8, fraction: u64) -> u8 {
    let inverse = FRACTION_ONE - fraction;
    let value = (u64::from(left) * inverse + u64::from(right) * fraction + FRACTION_ONE / 2)
        >> FRACTION_BITS;
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn rounded_u8(numerator: u64, denominator: u64) -> u8 {
    u8::try_from((numerator + denominator / 2) / denominator).unwrap_or(u8::MAX)
}

fn saturating_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    #[test]
    fn validates_image_storage() {
        let pixels = [0; 16];
        assert!(matches!(
            Image::new(&pixels, 2, 2, 7),
            Err(ImageError::StrideTooSmall)
        ));
        assert!(matches!(
            Image::new(&pixels[..15], 2, 2, 8),
            Err(ImageError::BufferTooSmall)
        ));
    }

    #[test]
    fn scales_with_bilinear_filtering() {
        let source = [
            0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
        ];
        let image = Image::new(&source, 2, 2, 8).unwrap();
        let mut destination = [0; 3 * 3 * 4];
        let mut surface = Surface::new(&mut destination, 3, 3, 3, PixelFormat::Rgb).unwrap();
        surface.draw_image_scaled(image, Rect::new(0, 0, 3, 3));

        let center = surface.color_at(1, 1).unwrap().red;
        assert!((120..=136).contains(&center));
    }

    #[test]
    fn image_alpha_is_composited() {
        let source = [255, 255, 255, 128];
        let image = Image::new(&source, 1, 1, 4).unwrap();
        let mut destination = [0; 4];
        let mut surface = Surface::new(&mut destination, 1, 1, 1, PixelFormat::Rgb).unwrap();
        surface.draw_image(image, Point::new(0, 0));
        assert_eq!(surface.color_at(0, 0), Some(Color::rgb(128, 128, 128)));
    }

    #[test]
    fn downscaling_filters_transparency_without_dark_fringes() {
        let source = [
            255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let image = Image::new(&source, 4, 1, 16).unwrap();
        let mut destination = [0; 4];
        let mut surface = Surface::new(&mut destination, 1, 1, 1, PixelFormat::Rgb).unwrap();
        surface.draw_image_scaled(image, Rect::new(0, 0, 1, 1));

        let color = surface.color_at(0, 0).unwrap();
        assert!((120..=136).contains(&color.red));
        assert_eq!(color.red, color.green);
        assert_eq!(color.green, color.blue);
    }
}
