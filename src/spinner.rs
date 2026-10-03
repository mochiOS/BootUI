use crate::{Color, Point, Surface};

const DOT_COUNT: usize = 12;
const UNIT: i32 = 1_024;
const POSITIONS: [(i32, i32); DOT_COUNT] = [
    (0, -1_024),
    (512, -887),
    (887, -512),
    (1_024, 0),
    (887, 512),
    (512, 887),
    (0, 1_024),
    (-512, 887),
    (-887, 512),
    (-1_024, 0),
    (-887, -512),
    (-512, -887),
];

const ARC_POINT_COUNT: usize = 32;
const ARC_SWEEP_POINTS: usize = 22;
const ARC_POSITIONS: [(i32, i32); ARC_POINT_COUNT] = [
    (0, -1_024),
    (200, -1_004),
    (392, -946),
    (569, -852),
    (724, -724),
    (852, -569),
    (946, -392),
    (1_004, -200),
    (1_024, 0),
    (1_004, 200),
    (946, 392),
    (852, 569),
    (724, 724),
    (569, 852),
    (392, 946),
    (200, 1_004),
    (0, 1_024),
    (-200, 1_004),
    (-392, 946),
    (-569, 852),
    (-724, 724),
    (-852, 569),
    (-946, 392),
    (-1_004, 200),
    (-1_024, 0),
    (-1_004, -200),
    (-946, -392),
    (-852, -569),
    (-724, -724),
    (-569, -852),
    (-392, -946),
    (-200, -1_004),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpinnerStyle {
    pub radius: u32,
    pub dot_radius: u32,
    pub color: Color,
}

impl SpinnerStyle {
    #[must_use]
    pub const fn new(radius: u32, dot_radius: u32, color: Color) -> Self {
        Self {
            radius,
            dot_radius,
            color,
        }
    }
}

/// Appearance of a single rotating circular stroke.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcSpinnerStyle {
    pub radius: u32,
    pub stroke_width: u32,
    pub color: Color,
}

impl ArcSpinnerStyle {
    #[must_use]
    pub const fn new(radius: u32, stroke_width: u32, color: Color) -> Self {
        Self {
            radius,
            stroke_width,
            color,
        }
    }
}

impl Surface<'_> {
    /// Draws a rotating antialiased arc with rounded ends.
    pub fn draw_arc_spinner(&mut self, center: Point, phase: u8, style: ArcSpinnerStyle) {
        if style.radius == 0 || style.stroke_width == 0 || style.color.alpha == 0 {
            return;
        }
        let start = usize::from(phase) % ARC_POINT_COUNT;
        let brush_radius = style.stroke_width.div_ceil(2);
        for offset in 0..ARC_SWEEP_POINTS {
            let current = arc_point(center, style.radius, (start + offset) % ARC_POINT_COUNT);
            self.fill_circle(current, brush_radius, style.color);
            if offset + 1 < ARC_SWEEP_POINTS {
                let next = arc_point(
                    center,
                    style.radius,
                    (start + offset + 1) % ARC_POINT_COUNT,
                );
                self.fill_circle(
                    Point::new((current.x + next.x) / 2, (current.y + next.y) / 2),
                    brush_radius,
                    style.color,
                );
            }
        }
    }

    /// Draws one of twelve deterministic spinner phases.
    ///
    /// The platform derives `phase` from its monotonic clock; `BootUI` does not
    /// own a timer or block the boot process.
    pub fn draw_spinner(&mut self, center: Point, phase: u8, style: SpinnerStyle) {
        if style.radius == 0 || style.dot_radius == 0 || style.color.alpha == 0 {
            return;
        }
        let phase = usize::from(phase) % DOT_COUNT;
        for (index, (unit_x, unit_y)) in POSITIONS.into_iter().enumerate() {
            let age = (index + DOT_COUNT - phase) % DOT_COUNT;
            let fade = u8::try_from((DOT_COUNT - 1 - age) * 207 / (DOT_COUNT - 1)).unwrap_or(207);
            let intensity = 48_u8.saturating_add(fade);
            let alpha = multiply_u8(style.color.alpha, intensity);
            let x =
                i64::from(center.x) + i64::from(unit_x) * i64::from(style.radius) / i64::from(UNIT);
            let y =
                i64::from(center.y) + i64::from(unit_y) * i64::from(style.radius) / i64::from(UNIT);
            self.fill_circle(
                Point::new(saturating_i64_to_i32(x), saturating_i64_to_i32(y)),
                style.dot_radius,
                style.color.with_alpha(alpha),
            );
        }
    }
}

fn arc_point(center: Point, radius: u32, index: usize) -> Point {
    let (unit_x, unit_y) = ARC_POSITIONS[index];
    let x = i64::from(center.x) + i64::from(unit_x) * i64::from(radius) / i64::from(UNIT);
    let y = i64::from(center.y) + i64::from(unit_y) * i64::from(radius) / i64::from(UNIT);
    Point::new(saturating_i64_to_i32(x), saturating_i64_to_i32(y))
}

fn multiply_u8(left: u8, right: u8) -> u8 {
    let value = (u16::from(left) * u16::from(right) + 127) / 255;
    u8::try_from(value).unwrap_or(u8::MAX)
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
    fn spinner_has_a_bright_head_and_faded_tail() {
        let mut pixels = [0; 32 * 32 * 4];
        let mut surface = Surface::new(&mut pixels, 32, 32, 32, PixelFormat::Rgb).unwrap();
        surface.draw_spinner(Point::new(16, 16), 0, SpinnerStyle::new(8, 2, Color::WHITE));

        let head = surface.color_at(16, 8).unwrap().red;
        let tail = surface.color_at(12, 9).unwrap().red;
        assert!(head > tail);
        assert!(tail > 0);
    }

    #[test]
    fn arc_spinner_draws_one_stroke_and_leaves_a_gap() {
        let mut pixels = [0; 40 * 40 * 4];
        let mut surface = Surface::new(&mut pixels, 40, 40, 40, PixelFormat::Rgb).unwrap();
        surface.draw_arc_spinner(
            Point::new(20, 20),
            0,
            ArcSpinnerStyle::new(10, 2, Color::WHITE),
        );

        assert!(surface.color_at(20, 10).unwrap().red > 250);
        assert_eq!(surface.color_at(10, 20).unwrap(), Color::BLACK);
    }
}
