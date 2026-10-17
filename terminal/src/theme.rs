//! The shared night-sky palette and color helpers, so every module draws
//! from the same handful of tones.

use ratatui::style::Color;

/// A 24-bit color as raw channels, for math (scaling, fades) before it
/// becomes a terminal [`Color`].
pub type Rgb = (u8, u8, u8);

pub const fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

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
pub const GOLD: Color = rgb(GOLD_RGB);
pub const EMERALD: Color = rgb(EMERALD_RGB);
pub const AMBER: Color = rgb(AMBER_RGB);
pub const RED: Color = Color::Rgb(243, 155, 155);

pub const GOLD_RGB: Rgb = (251, 191, 36);
pub const EMERALD_RGB: Rgb = (145, 203, 181);
pub const AMBER_RGB: Rgb = (241, 196, 119);

/// Named accents for cards and for `hdr` / `bar` rows. A config or row that
/// names anything else is rejected (config) or falls back to text (rows).
pub const ACCENTS: &[(&str, Rgb)] = &[
    ("violet", (167, 139, 250)),
    ("pink", (244, 114, 182)),
    ("sky", (125, 211, 252)),
    ("blue", (59, 130, 246)),
    ("emerald", (52, 211, 153)),
    ("gold", GOLD_RGB),
    ("red", (248, 113, 113)),
    ("slate", (148, 163, 184)),
    ("coral", (217, 119, 87)),
    ("teal", (16, 163, 127)),
];

/// A card's default accent, and the docker card's.
pub const VIOLET_RGB: Rgb = ACCENTS[0].1;
pub const BLUE_RGB: Rgb = ACCENTS[3].1;

pub fn accent_rgb(name: &str) -> Option<Rgb> {
    ACCENTS.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

pub fn accent_names() -> String {
    ACCENTS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn scale(c: Rgb, f: f32) -> Color {
    Color::Rgb(
        (c.0 as f32 * f) as u8,
        (c.1 as f32 * f) as u8,
        (c.2 as f32 * f) as u8,
    )
}

/// The 24-bit ANSI foreground escape for plain stdout (help, quiet mode).
pub fn ansi(c: Rgb) -> String {
    format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2)
}

pub const ANSI_RESET: &str = "\x1b[0m";
