#![allow(dead_code, unused_imports)]

//! Internal design system tokens for OpenZone.
//!
//! Implements the visual design guidance: cool paper base, graphite
//! controls, blue galaxy live accent. Exposes typed color tokens,
//! spacing/radius scales, typography roles, and a theme struct that
//! resolves tokens to concrete values per mode.

pub mod palette;
pub mod theme;
pub mod tokens;

pub use theme::{OpenZoneTheme, ThemeMode};
pub use tokens::{
    AccentToken, ActionToken, BackgroundToken, BorderToken, ForegroundToken, RadiusToken,
    SpacingToken, StatusToken, TypeRole,
};
