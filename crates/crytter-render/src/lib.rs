//! Canvas2D renderer for crytter.
//!
//! Draws the terminal grid to an HTML canvas using Canvas2D API.

pub mod palette;
mod renderer;

pub use palette::Theme;
pub use renderer::Renderer;
