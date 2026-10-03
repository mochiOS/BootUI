#![no_std]

//! Allocation-free UI rendering for firmware and early-boot environments.
//!
//! `BootUI` renders into memory supplied by its caller. It does not own the
//! framebuffer, allocate memory, access firmware services, or manage input.

mod color;
mod draw;
mod geometry;
mod surface;

pub use color::Color;
pub use geometry::{Point, Rect, Size};
pub use surface::{PixelFormat, Surface, SurfaceError};
