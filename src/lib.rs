#![no_std]

//! Allocation-free UI rendering for firmware and early-boot environments.
//!
//! `BootUI` renders into memory supplied by its caller. It does not own the
//! framebuffer, allocate memory, access firmware services, or manage input.

mod color;
mod draw;
mod geometry;
mod image;
mod spinner;
mod surface;

pub use color::Color;
pub use geometry::{Point, Rect, Size};
pub use image::{Image, ImageError};
pub use spinner::{ArcSpinnerStyle, SpinnerStyle};
pub use surface::{PixelFormat, Surface, SurfaceError};
