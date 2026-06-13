#![allow(dead_code, unused_imports)]

//! Internal design system tokens for OpenZone.
//!
//! Implements the visual design guidance: monochrome surfaces, graphite
//! controls, and grayscale live accent. Exposes typed color tokens,
//! spacing/radius scales, typography roles, and a theme struct that
//! resolves tokens to concrete values per mode.

pub mod design_palette;
pub mod design_theme;
pub mod design_tokens;

pub use design_theme::{OpenZoneTheme, ThemeMode};
pub use design_tokens::{
    AccentToken, ActionToken, BackgroundToken, BorderToken, ForegroundToken, RadiusToken,
    SpacingToken, StatusToken, TypeRole,
};
