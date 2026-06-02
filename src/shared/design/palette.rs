//! Concrete palette values.
//!
//! The cool-paper / blue-galaxy system expressed as `iced::Color`s so
//! every other module resolves tokens against a single source of
//! truth. Kept as a flat set of constants — no runtime state.

use iced::Color;

// ---------------------------------------------------------------------------
// Light theme
// ---------------------------------------------------------------------------

pub const LIGHT_SURFACE_BASE: Color = Color::from_rgb(
    0xF7 as f32 / 255.0,
    0xF9 as f32 / 255.0,
    0xFD as f32 / 255.0,
);
pub const LIGHT_SURFACE_PAPER: Color = Color::from_rgb(
    0xF2 as f32 / 255.0,
    0xF6 as f32 / 255.0,
    0xFC as f32 / 255.0,
);
pub const LIGHT_SURFACE_RAISED: Color = Color::from_rgb(1.0, 1.0, 1.0);
pub const LIGHT_SURFACE_SUBTLE: Color = Color::from_rgb(
    0xEA as f32 / 255.0,
    0xF0 as f32 / 255.0,
    0xFA as f32 / 255.0,
);
pub const LIGHT_SURFACE_GALAXY_TINT: Color =
    Color::from_rgb(0xDD as f32 / 255.0, 0xE8 as f32 / 255.0, 1.0);

pub const LIGHT_TEXT_PRIMARY: Color = Color::from_rgb(
    0x11 as f32 / 255.0,
    0x13 as f32 / 255.0,
    0x18 as f32 / 255.0,
);
pub const LIGHT_TEXT_SECONDARY: Color = Color::from_rgb(
    0x68 as f32 / 255.0,
    0x71 as f32 / 255.0,
    0x80 as f32 / 255.0,
);
pub const LIGHT_TEXT_TERTIARY: Color = Color::from_rgb(
    0x9B as f32 / 255.0,
    0xA6 as f32 / 255.0,
    0xB6 as f32 / 255.0,
);

pub const LIGHT_LINE_SOFT: Color = Color::from_rgb(
    0xD9 as f32 / 255.0,
    0xE1 as f32 / 255.0,
    0xEE as f32 / 255.0,
);
pub const LIGHT_LINE_STRONG: Color = Color::from_rgb(
    0xB8 as f32 / 255.0,
    0xC4 as f32 / 255.0,
    0xD6 as f32 / 255.0,
);

pub const LIGHT_ACCENT_PRIMARY: Color =
    Color::from_rgb(0x2F as f32 / 255.0, 0x6B as f32 / 255.0, 1.0);
pub const LIGHT_ACCENT_DEEP: Color = Color::from_rgb(
    0x12 as f32 / 255.0,
    0x39 as f32 / 255.0,
    0xA6 as f32 / 255.0,
);
pub const LIGHT_ACCENT_SOFT: Color = Color::from_rgb(0xDD as f32 / 255.0, 0xE8 as f32 / 255.0, 1.0);
pub const LIGHT_GALAXY_AURA: Color = Color {
    r: 106.0 / 255.0,
    g: 228.0 / 255.0,
    b: 1.0,
    a: 0.12,
};

pub const LIGHT_CONTROL_STRONG: Color = Color::from_rgb(
    0x11 as f32 / 255.0,
    0x13 as f32 / 255.0,
    0x18 as f32 / 255.0,
);
pub const LIGHT_CONTROL_STRONG_TEXT: Color = Color::from_rgb(1.0, 1.0, 1.0);

pub const LIGHT_DANGER: Color = Color::from_rgb(
    0xC9 as f32 / 255.0,
    0x2A as f32 / 255.0,
    0x2A as f32 / 255.0,
);
pub const LIGHT_SUCCESS: Color = Color::from_rgb(
    0x08 as f32 / 255.0,
    0x7F as f32 / 255.0,
    0x5B as f32 / 255.0,
);
pub const LIGHT_WARNING: Color = Color::from_rgb(
    0x9A as f32 / 255.0,
    0x67 as f32 / 255.0,
    0x00 as f32 / 255.0,
);

// ---------------------------------------------------------------------------
// Dark theme
// ---------------------------------------------------------------------------

pub const DARK_SURFACE_BASE: Color = Color::from_rgb(
    0x09 as f32 / 255.0,
    0x0D as f32 / 255.0,
    0x18 as f32 / 255.0,
);
pub const DARK_SURFACE_PAPER: Color = Color::from_rgb(
    0x0D as f32 / 255.0,
    0x14 as f32 / 255.0,
    0x24 as f32 / 255.0,
);
pub const DARK_SURFACE_RAISED: Color = Color::from_rgb(
    0x11 as f32 / 255.0,
    0x18 as f32 / 255.0,
    0x27 as f32 / 255.0,
);
pub const DARK_SURFACE_SUBTLE: Color = Color::from_rgb(
    0x16 as f32 / 255.0,
    0x20 as f32 / 255.0,
    0x33 as f32 / 255.0,
);
pub const DARK_SURFACE_GALAXY_TINT: Color = Color::from_rgb(
    0x17 as f32 / 255.0,
    0x2B as f32 / 255.0,
    0x57 as f32 / 255.0,
);

pub const DARK_TEXT_PRIMARY: Color = Color::from_rgb(
    0xF4 as f32 / 255.0,
    0xF7 as f32 / 255.0,
    0xFB as f32 / 255.0,
);
pub const DARK_TEXT_SECONDARY: Color = Color::from_rgb(
    0xAA as f32 / 255.0,
    0xB5 as f32 / 255.0,
    0xC6 as f32 / 255.0,
);
pub const DARK_TEXT_TERTIARY: Color = Color::from_rgb(
    0x7E as f32 / 255.0,
    0x8A as f32 / 255.0,
    0xA0 as f32 / 255.0,
);

pub const DARK_LINE_SOFT: Color = Color::from_rgb(
    0x2B as f32 / 255.0,
    0x36 as f32 / 255.0,
    0x4A as f32 / 255.0,
);
pub const DARK_LINE_STRONG: Color = Color::from_rgb(
    0x3F as f32 / 255.0,
    0x4E as f32 / 255.0,
    0x68 as f32 / 255.0,
);

pub const DARK_ACCENT_PRIMARY: Color =
    Color::from_rgb(0x6F as f32 / 255.0, 0xA0 as f32 / 255.0, 1.0);
pub const DARK_ACCENT_DEEP: Color = Color::from_rgb(0x9D as f32 / 255.0, 0xBD as f32 / 255.0, 1.0);
pub const DARK_ACCENT_SOFT: Color = Color::from_rgb(
    0x17 as f32 / 255.0,
    0x2B as f32 / 255.0,
    0x57 as f32 / 255.0,
);
pub const DARK_GALAXY_AURA: Color = Color {
    r: 106.0 / 255.0,
    g: 228.0 / 255.0,
    b: 1.0,
    a: 0.16,
};

pub const DARK_CONTROL_STRONG: Color = Color::from_rgb(
    0xF4 as f32 / 255.0,
    0xF7 as f32 / 255.0,
    0xFB as f32 / 255.0,
);
pub const DARK_CONTROL_STRONG_TEXT: Color = Color::from_rgb(
    0x11 as f32 / 255.0,
    0x13 as f32 / 255.0,
    0x18 as f32 / 255.0,
);

pub const DARK_DANGER: Color = Color::from_rgb(1.0, 0x8A as f32 / 255.0, 0x8A as f32 / 255.0);
pub const DARK_SUCCESS: Color = Color::from_rgb(
    0x63 as f32 / 255.0,
    0xE6 as f32 / 255.0,
    0xBE as f32 / 255.0,
);
pub const DARK_WARNING: Color = Color::from_rgb(1.0, 0xD1 as f32 / 255.0, 0x66 as f32 / 255.0);
