use crate::{Color, Point, Rect, Surface};

const SUBPIXEL_SCALE: i64 = 8;
const SAMPLE_OFFSETS: [i64; 4] = [1, 3, 5, 7];
const SAMPLE_COUNT: u16 = 16;

impl Surface<'_> {
    /// Fills a pixel-aligned rectangle.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rounded_rect(rect, 0, color);
    }

    /// Fills a rounded rectangle using 4x4 subpixel coverage at its boundary.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        if rect.is_empty() || color.alpha == 0 {
            return;
        }
        let radius = radius.min(rect.size.width / 2).min(rect.size.height / 2);
        let Some(bounds) = clipped_bounds(rect, self.width(), self.height()) else {
            return;
        };

        for y in bounds.top..bounds.bottom {
            for x in bounds.left..bounds.right {
                let coverage = rounded_rect_coverage(rect, radius, x, y);
                self.blend_pixel(x, y, color, coverage);
            }
        }
    }

    /// Fills an antialiased circle. `radius` is measured from the center to
    /// the outer edge.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        if radius == 0 || color.alpha == 0 {
            return;
        }
        let radius_i64 = i64::from(radius);
        let rect = Rect::new(
            saturating_i64_to_i32(i64::from(center.x) - radius_i64),
            saturating_i64_to_i32(i64::from(center.y) - radius_i64),
            radius.saturating_mul(2),
            radius.saturating_mul(2),
        );
        let Some(bounds) = clipped_bounds(rect, self.width(), self.height()) else {
            return;
        };

        for y in bounds.top..bounds.bottom {
            for x in bounds.left..bounds.right {
                let coverage = circle_coverage(center, radius, x, y);
                self.blend_pixel(x, y, color, coverage);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn clipped_bounds(rect: Rect, width: u32, height: u32) -> Option<Bounds> {
    let left = i64::from(rect.origin.x).max(0);
    let top = i64::from(rect.origin.y).max(0);
    let right = (i64::from(rect.origin.x) + i64::from(rect.size.width))
        .min(i64::from(width))
        .max(0);
    let bottom = (i64::from(rect.origin.y) + i64::from(rect.size.height))
        .min(i64::from(height))
        .max(0);
    if left >= right || top >= bottom {
        return None;
    }
    Some(Bounds {
        left: saturating_i64_to_i32(left),
        top: saturating_i64_to_i32(top),
        right: saturating_i64_to_i32(right),
        bottom: saturating_i64_to_i32(bottom),
    })
}

fn rounded_rect_coverage(rect: Rect, radius: u32, pixel_x: i32, pixel_y: i32) -> u8 {
    if radius == 0 {
        return u8::MAX;
    }

    let left = i64::from(rect.origin.x) * SUBPIXEL_SCALE;
    let top = i64::from(rect.origin.y) * SUBPIXEL_SCALE;
    let right = (i64::from(rect.origin.x) + i64::from(rect.size.width)) * SUBPIXEL_SCALE;
    let bottom = (i64::from(rect.origin.y) + i64::from(rect.size.height)) * SUBPIXEL_SCALE;
    let radius = i64::from(radius) * SUBPIXEL_SCALE;
    let radius_squared = i128::from(radius) * i128::from(radius);
    let mut inside = 0;

    for offset_y in SAMPLE_OFFSETS {
        let sample_y = i64::from(pixel_y) * SUBPIXEL_SCALE + offset_y;
        for offset_x in SAMPLE_OFFSETS {
            let sample_x = i64::from(pixel_x) * SUBPIXEL_SCALE + offset_x;
            let corner_x = if sample_x < left + radius {
                left + radius
            } else if sample_x > right - radius {
                right - radius
            } else {
                sample_x
            };
            let corner_y = if sample_y < top + radius {
                top + radius
            } else if sample_y > bottom - radius {
                bottom - radius
            } else {
                sample_y
            };
            let delta_x = i128::from(sample_x - corner_x);
            let delta_y = i128::from(sample_y - corner_y);
            if delta_x * delta_x + delta_y * delta_y <= radius_squared {
                inside += 1;
            }
        }
    }

    coverage_from_samples(inside)
}

fn circle_coverage(center: Point, radius: u32, pixel_x: i32, pixel_y: i32) -> u8 {
    let center_x = i64::from(center.x) * SUBPIXEL_SCALE;
    let center_y = i64::from(center.y) * SUBPIXEL_SCALE;
    let radius = i64::from(radius) * SUBPIXEL_SCALE;
    let radius_squared = i128::from(radius) * i128::from(radius);
    let mut inside = 0;

    for offset_y in SAMPLE_OFFSETS {
        let sample_y = i64::from(pixel_y) * SUBPIXEL_SCALE + offset_y;
        for offset_x in SAMPLE_OFFSETS {
            let sample_x = i64::from(pixel_x) * SUBPIXEL_SCALE + offset_x;
            let delta_x = i128::from(sample_x - center_x);
            let delta_y = i128::from(sample_y - center_y);
            if delta_x * delta_x + delta_y * delta_y <= radius_squared {
                inside += 1;
            }
        }
    }

    coverage_from_samples(inside)
}

fn coverage_from_samples(inside: u16) -> u8 {
    let coverage = (inside * u16::from(u8::MAX) + SAMPLE_COUNT / 2) / SAMPLE_COUNT;
    u8::try_from(coverage).unwrap_or(u8::MAX)
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
    fn rounded_corner_has_partial_coverage() {
        let mut pixels = [0; 8 * 8 * 4];
        let mut surface = Surface::new(&mut pixels, 8, 8, 8, PixelFormat::Rgb).unwrap();
        surface.fill_rounded_rect(Rect::new(0, 0, 8, 8), 4, Color::WHITE);

        assert_eq!(surface.color_at(0, 0), Some(Color::BLACK));
        let edge = surface.color_at(1, 0).unwrap().red;
        assert!(edge > 0 && edge < 255);
        assert_eq!(surface.color_at(3, 3), Some(Color::WHITE));
    }

    #[test]
    fn circle_boundary_is_antialiased() {
        let mut pixels = [0; 10 * 10 * 4];
        let mut surface = Surface::new(&mut pixels, 10, 10, 10, PixelFormat::Rgb).unwrap();
        surface.fill_circle(Point::new(5, 5), 4, Color::WHITE);

        let boundary = surface.color_at(2, 2).unwrap().red;
        assert!(boundary > 0 && boundary < 255);
        assert_eq!(surface.color_at(5, 5), Some(Color::WHITE));
        assert_eq!(surface.color_at(0, 0), Some(Color::BLACK));
    }

    #[test]
    fn drawing_is_clipped_to_the_surface() {
        let mut pixels = [0; 4 * 4 * 4];
        let mut surface = Surface::new(&mut pixels, 4, 4, 4, PixelFormat::Rgb).unwrap();
        surface.fill_rounded_rect(Rect::new(-8, -8, 10, 10), 3, Color::WHITE);
        surface.fill_circle(Point::new(4, 4), 4, Color::WHITE);
    }
}
