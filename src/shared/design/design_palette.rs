//! Concrete palette values.
//!
//! Monochrome system expressed as `iced::Color`s — pure grayscale with
//! no chromatic accents. Light and dark modes share the same structure;
//! only luminance inverts.

use iced::Color;

// ---------------------------------------------------------------------------
// Light theme
// ---------------------------------------------------------------------------

pub const LIGHT_SURFACE_BASE: Color = Color::from_rgb(0.98, 0.98, 0.98);
pub const LIGHT_SURFACE_PAPER: Color = Color::from_rgb(0.96, 0.96, 0.96);
pub const LIGHT_SURFACE_RAISED: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const LIGHT_SURFACE_SUBTLE: Color = Color::from_rgb(0.94, 0.94, 0.94);
pub const LIGHT_SURFACE_GALAXY_TINT: Color = Color::from_rgb(0.91, 0.91, 0.91);

pub const LIGHT_TEXT_PRIMARY: Color = Color::from_rgb(0.04, 0.04, 0.04);
pub const LIGHT_TEXT_SECONDARY: Color = Color::from_rgb(0.32, 0.32, 0.32);
pub const LIGHT_TEXT_TERTIARY: Color = Color::from_rgb(0.64, 0.64, 0.64);

pub const LIGHT_LINE_SOFT: Color = Color::from_rgb(0.90, 0.90, 0.90);
pub const LIGHT_LINE_STRONG: Color = Color::from_rgb(0.83, 0.83, 0.83);

pub const LIGHT_ACCENT_PRIMARY: Color = Color::from_rgb(0.09, 0.09, 0.09);
pub const LIGHT_ACCENT_DEEP: Color = Color::from_rgb(0.02, 0.02, 0.02);
pub const LIGHT_ACCENT_SOFT: Color = Color::from_rgb(0.94, 0.94, 0.94);
pub const LIGHT_GALAXY_AURA: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.06,
};

pub const LIGHT_CONTROL_STRONG: Color = Color::from_rgb(0.04, 0.04, 0.04);
pub const LIGHT_CONTROL_STRONG_TEXT: Color = Color::from_rgb(0.98, 0.98, 0.98);

pub const LIGHT_DANGER: Color = Color::from_rgb(0.45, 0.45, 0.45);
pub const LIGHT_SUCCESS: Color = Color::from_rgb(0.25, 0.25, 0.25);
pub const LIGHT_WARNING: Color = Color::from_rgb(0.40, 0.40, 0.40);

// ---------------------------------------------------------------------------
// Dark theme
// ---------------------------------------------------------------------------

pub const DARK_SURFACE_BASE: Color = Color::from_rgb(0.0, 0.0, 0.0);
pub const DARK_SURFACE_PAPER: Color = Color::from_rgb(0.04, 0.04, 0.04);
pub const DARK_SURFACE_RAISED: Color = Color::from_rgb(0.08, 0.08, 0.08);
pub const DARK_SURFACE_SUBTLE: Color = Color::from_rgb(0.10, 0.10, 0.10);
pub const DARK_SURFACE_GALAXY_TINT: Color = Color::from_rgb(0.12, 0.12, 0.12);

pub const DARK_TEXT_PRIMARY: Color = Color::from_rgb(0.98, 0.98, 0.98);
pub const DARK_TEXT_SECONDARY: Color = Color::from_rgb(0.64, 0.64, 0.64);
pub const DARK_TEXT_TERTIARY: Color = Color::from_rgb(0.45, 0.45, 0.45);

pub const DARK_LINE_SOFT: Color = Color::from_rgb(0.15, 0.15, 0.15);
pub const DARK_LINE_STRONG: Color = Color::from_rgb(0.25, 0.25, 0.25);

pub const DARK_ACCENT_PRIMARY: Color = Color::from_rgb(0.90, 0.90, 0.90);
pub const DARK_ACCENT_DEEP: Color = Color::from_rgb(0.98, 0.98, 0.98);
pub const DARK_ACCENT_SOFT: Color = Color::from_rgb(0.10, 0.10, 0.10);
pub const DARK_GALAXY_AURA: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.08,
};

pub const DARK_CONTROL_STRONG: Color = Color::from_rgb(0.98, 0.98, 0.98);
pub const DARK_CONTROL_STRONG_TEXT: Color = Color::from_rgb(0.04, 0.04, 0.04);

pub const DARK_DANGER: Color = Color::from_rgb(0.55, 0.55, 0.55);
pub const DARK_SUCCESS: Color = Color::from_rgb(0.75, 0.75, 0.75);
pub const DARK_WARNING: Color = Color::from_rgb(0.60, 0.60, 0.60);
