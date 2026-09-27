//! The shared night-sky palette and color helpers, so every module draws
//! from the same handful of tones.

use ratatui::style::Color;

pub const TEXT: Color = Color::Rgb(226, 232, 240);
pub const BACKGROUND: Color = Color::Rgb(17, 18, 29);
pub const MUTED: Color = Color::Rgb(160, 167, 186);
pub const BORDER: Color = Color::Rgb(52, 54, 77);
pub const SELECTION: Color = Color::Rgb(32, 32, 49);
pub const SLATE_400: Color = Color::Rgb(148, 163, 184);
pub const SLATE_500: Color = Color::Rgb(100, 116, 139);
pub const SLATE_600: Color = Color::Rgb(71, 85, 105);
pub const SLATE_700: Color = Color::Rgb(51, 65, 85);
pub const VIOLET_LIGHT: Color = Color::Rgb(185, 166, 245);
pub const GOLD: Color = Color::Rgb(251, 191, 36);
pub const EMERALD: Color = Color::Rgb(145, 203, 181);
pub const AMBER: Color = Color::Rgb(241, 196, 119);
pub const RED: Color = Color::Rgb(243, 155, 155);

/// Dot colors for the three row-health states, shared by every card.
pub const HEALTH_GOOD: (u8, u8, u8) = (52, 211, 153);
pub const HEALTH_WARN: (u8, u8, u8) = (251, 191, 36);
pub const HEALTH_BAD: (u8, u8, u8) = (248, 113, 113);

// Pink on the left melting into violet on the right, like the poster.
pub const GRADIENT: [(u8, u8, u8); 3] = [(244, 114, 182), (216, 96, 245), (147, 100, 250)];

pub fn gradient_rgb(t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0) * 2.0;
    let (a, b, frac) = if t < 1.0 {
        (GRADIENT[0], GRADIENT[1], t)
    } else {
        (GRADIENT[1], GRADIENT[2], t - 1.0)
    };
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * frac) as u8;
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}

pub fn gradient(t: f32) -> Color {
    let (r, g, b) = gradient_rgb(t);
    Color::Rgb(r, g, b)
}

pub fn scale(rgb: (u8, u8, u8), f: f32) -> Color {
    Color::Rgb(
        (rgb.0 as f32 * f) as u8,
        (rgb.1 as f32 * f) as u8,
        (rgb.2 as f32 * f) as u8,
    )
}

pub fn pulse(rgb: (u8, u8, u8), tick: u64) -> Color {
    let phase = (tick % 24) as f32 / 24.0 * std::f32::consts::TAU;
    scale(rgb, 0.65 + 0.35 * phase.sin())
}

/// Ping-pong a value into [0, 1] so animated gradients drift without a seam.
pub fn tri(x: f32) -> f32 {
    let m = x.rem_euclid(2.0);
    if m <= 1.0 {
        m
    } else {
        2.0 - m
    }
}
