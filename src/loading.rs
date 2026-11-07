use crate::{ArcSpinnerStyle, Color, Image, Point, Rect, Surface};

/// A logo and an animated arc, painted into caller-owned boot surfaces.
pub struct LoadingScreen<'pixels> {
    logo: Image<'pixels>,
}

impl<'pixels> LoadingScreen<'pixels> {
    #[must_use]
    pub const fn new(logo: Image<'pixels>) -> Self {
        Self { logo }
    }

    /// The caller supplies elapsed monotonic milliseconds, independently of I/O.
    #[must_use]
    pub const fn phase_at(elapsed_millis: u64) -> u8 {
        ((elapsed_millis % 1_000) * crate::spinner::ARC_POINT_COUNT as u64 / 1_000) as u8
    }

    pub fn paint(&self, surface: &mut Surface<'_>, elapsed_millis: u64) {
        surface.clear(Color::BLACK);
        let longest = (surface.width() / 8).min(surface.height() / 8).max(1);
        let source_longest = self.logo.width().max(self.logo.height());
        let width =
            (u64::from(self.logo.width()) * u64::from(longest) / u64::from(source_longest)) as u32;
        let height =
            (u64::from(self.logo.height()) * u64::from(longest) / u64::from(source_longest)) as u32;
        let x = i64::from(surface.width()) / 2 - i64::from(width) / 2;
        let y = i64::from(surface.height()) * 38 / 100 - i64::from(height) / 2;
        surface.draw_image_scaled(
            self.logo,
            Rect::new(x as i32, y as i32, width.max(1), height.max(1)),
        );
        Self::paint_spinner(surface, elapsed_millis);
    }

    #[must_use]
    pub fn spinner_bounds(surface: &Surface<'_>) -> Rect {
        let center = Self::spinner_center(surface);
        Rect::new(
            center.x.saturating_sub(12),
            center.y.saturating_sub(12),
            24,
            24,
        )
    }

    pub fn clear_spinner(surface: &mut Surface<'_>) {
        surface.fill_rect(Self::spinner_bounds(surface), Color::BLACK);
    }

    pub fn paint_spinner(surface: &mut Surface<'_>, elapsed_millis: u64) {
        Self::clear_spinner(surface);
        surface.draw_arc_spinner(
            Self::spinner_center(surface),
            Self::phase_at(elapsed_millis),
            ArcSpinnerStyle::new(8, 2, Color::rgb(244, 246, 250)),
        );
    }

    fn spinner_center(surface: &Surface<'_>) -> Point {
        Point::new(
            (surface.width() / 2).min(i32::MAX as u32) as i32,
            (u64::from(surface.height()) * 3 / 5).min(i32::MAX as u64) as i32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;
    use std::vec;

    #[test]
    fn elapsed_time_advances_and_wraps_the_same_arc() {
        let logo_pixels = [255; 4 * 4 * 4];
        let logo = Image::new(&logo_pixels, 4, 4, 16).unwrap();
        let screen = LoadingScreen::new(logo);
        let mut pixels = vec![0; 128 * 96 * 4];
        screen.paint(
            &mut Surface::new(&mut pixels, 128, 96, 128, PixelFormat::Bgr).unwrap(),
            0,
        );
        let first = pixels.clone();
        LoadingScreen::paint_spinner(
            &mut Surface::new(&mut pixels, 128, 96, 128, PixelFormat::Bgr).unwrap(),
            250,
        );
        assert_ne!(pixels, first);
        LoadingScreen::paint_spinner(
            &mut Surface::new(&mut pixels, 128, 96, 128, PixelFormat::Bgr).unwrap(),
            1_000,
        );
        assert_eq!(
            pixels, first,
            "rotation must leave no pixels from earlier phases"
        );
        assert_eq!(
            LoadingScreen::phase_at(u64::MAX),
            LoadingScreen::phase_at(u64::MAX % 1_000)
        );
    }
}
