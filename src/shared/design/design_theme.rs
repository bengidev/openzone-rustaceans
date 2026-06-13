//! Theme resolver.
//!
//! [`OpenZoneTheme`] holds a [`ThemeMode`] and exposes typed accessors
//! that resolve tokens to concrete `iced::Color` values. Cheap to
//! clone — the palette lives in module-level constants.

use iced::Color;

use crate::shared::design::design_palette as palette;
use crate::shared::design::design_tokens::{
    AccentToken, ActionToken, BackgroundToken, BorderToken, ForegroundToken, StatusToken,
};

/// Light / dark mode selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn toggle(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }
}

/// Resolved theme. Holds the active mode so call sites can branch on
/// it when a token alone is not enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenZoneTheme {
    pub mode: ThemeMode,
}

impl OpenZoneTheme {
    pub fn from_mode(mode: ThemeMode) -> Self {
        Self { mode }
    }

    pub fn dark() -> Self {
        Self::from_mode(ThemeMode::Dark)
    }

    pub fn light() -> Self {
        Self::from_mode(ThemeMode::Light)
    }

    // -- background ---------------------------------------------------------

    pub fn background(&self, token: BackgroundToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                BackgroundToken::Primary => palette::LIGHT_SURFACE_BASE,
                BackgroundToken::Secondary => palette::LIGHT_SURFACE_PAPER,
                BackgroundToken::Elevated => palette::LIGHT_SURFACE_RAISED,
                BackgroundToken::Tertiary => palette::LIGHT_SURFACE_SUBTLE,
                BackgroundToken::GalaxyTint => palette::LIGHT_SURFACE_GALAXY_TINT,
            },
            ThemeMode::Dark => match token {
                BackgroundToken::Primary => palette::DARK_SURFACE_BASE,
                BackgroundToken::Secondary => palette::DARK_SURFACE_PAPER,
                BackgroundToken::Elevated => palette::DARK_SURFACE_RAISED,
                BackgroundToken::Tertiary => palette::DARK_SURFACE_SUBTLE,
                BackgroundToken::GalaxyTint => palette::DARK_SURFACE_GALAXY_TINT,
            },
        }
    }

    // -- foreground ---------------------------------------------------------

    pub fn foreground(&self, token: ForegroundToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                ForegroundToken::Primary => palette::LIGHT_TEXT_PRIMARY,
                ForegroundToken::Secondary => palette::LIGHT_TEXT_SECONDARY,
                ForegroundToken::Muted => palette::LIGHT_TEXT_TERTIARY,
                ForegroundToken::Accent => palette::LIGHT_ACCENT_PRIMARY,
            },
            ThemeMode::Dark => match token {
                ForegroundToken::Primary => palette::DARK_TEXT_PRIMARY,
                ForegroundToken::Secondary => palette::DARK_TEXT_SECONDARY,
                ForegroundToken::Muted => palette::DARK_TEXT_TERTIARY,
                ForegroundToken::Accent => palette::DARK_ACCENT_PRIMARY,
            },
        }
    }

    // -- border -------------------------------------------------------------

    pub fn border(&self, token: BorderToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                BorderToken::Default => palette::LIGHT_LINE_SOFT,
                BorderToken::Strong => palette::LIGHT_LINE_STRONG,
                BorderToken::Subtle => palette::LIGHT_LINE_SOFT,
            },
            ThemeMode::Dark => match token {
                BorderToken::Default => palette::DARK_LINE_SOFT,
                BorderToken::Strong => palette::DARK_LINE_STRONG,
                BorderToken::Subtle => palette::DARK_LINE_SOFT,
            },
        }
    }

    // -- status -------------------------------------------------------------

    pub fn status(&self, token: StatusToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                StatusToken::Info => palette::LIGHT_ACCENT_DEEP,
                StatusToken::Success => palette::LIGHT_SUCCESS,
                StatusToken::Warning => palette::LIGHT_WARNING,
                StatusToken::Danger => palette::LIGHT_DANGER,
            },
            ThemeMode::Dark => match token {
                StatusToken::Info => palette::DARK_ACCENT_DEEP,
                StatusToken::Success => palette::DARK_SUCCESS,
                StatusToken::Warning => palette::DARK_WARNING,
                StatusToken::Danger => palette::DARK_DANGER,
            },
        }
    }

    // -- accent depth -------------------------------------------------------

    pub fn accent(&self, token: AccentToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                AccentToken::Primary => palette::LIGHT_ACCENT_PRIMARY,
                AccentToken::Deep => palette::LIGHT_ACCENT_DEEP,
                AccentToken::Soft => palette::LIGHT_ACCENT_SOFT,
            },
            ThemeMode::Dark => match token {
                AccentToken::Primary => palette::DARK_ACCENT_PRIMARY,
                AccentToken::Deep => palette::DARK_ACCENT_DEEP,
                AccentToken::Soft => palette::DARK_ACCENT_SOFT,
            },
        }
    }

    // -- actions ------------------------------------------------------------

    pub fn action(&self, token: ActionToken) -> Color {
        match self.mode {
            ThemeMode::Light => match token {
                ActionToken::Strong => palette::LIGHT_CONTROL_STRONG,
                ActionToken::StrongText => palette::LIGHT_CONTROL_STRONG_TEXT,
            },
            ThemeMode::Dark => match token {
                ActionToken::Strong => palette::DARK_CONTROL_STRONG,
                ActionToken::StrongText => palette::DARK_CONTROL_STRONG_TEXT,
            },
        }
    }

    /// Atmospheric galaxy aura used sparingly for depth.
    pub fn galaxy_aura(&self) -> Color {
        match self.mode {
            ThemeMode::Light => palette::LIGHT_GALAXY_AURA,
            ThemeMode::Dark => palette::DARK_GALAXY_AURA,
        }
    }
}
