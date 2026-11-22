//! Neon colour palette shared by game logic (particle colours) and rendering.

use macroquad::color::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    /// Colour with the given intensity. Everything is drawn additively, so
    /// "alpha" is really brightness.
    pub fn a(self, alpha: f32) -> Color {
        Color::new(self.0, self.1, self.2, alpha.clamp(0.0, 1.0))
    }

    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
            self.2 + (other.2 - self.2) * t,
        )
    }
}

pub const WHITE: Rgb = Rgb(1.0, 1.0, 1.0);
pub const CYAN: Rgb = Rgb(0.0, 0.9, 1.0);
pub const AMBER: Rgb = Rgb(1.0, 0.7, 0.14);
pub const RED: Rgb = Rgb(1.0, 0.16, 0.28);
pub const GREEN: Rgb = Rgb(0.2, 1.0, 0.42);
pub const YELLOW: Rgb = Rgb(1.0, 0.9, 0.18);
pub const MAGENTA: Rgb = Rgb(1.0, 0.22, 0.88);
pub const BLUE: Rgb = Rgb(0.3, 0.52, 1.0);
pub const ORANGE: Rgb = Rgb(1.0, 0.42, 0.08);
pub const GRID: Rgb = Rgb(0.12, 0.3, 0.8);
