//! Typed token enums.
//!
//! Consumers resolve tokens through [`OpenZoneTheme`](crate::theme::OpenZoneTheme).
//! The enums are the stable vocabulary — the concrete palette may be
//! retuned without touching call sites.

/// Background surface tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundToken {
    /// Main app background.
    Primary,
    /// Paper-like sections and large calm surfaces.
    Secondary,
    /// Raised panels, menus, grouped content.
    Elevated,
    /// Secondary fills, disabled fills, quiet containers.
    Tertiary,
    /// Very light accent wash for selected fields and command hints.
    GalaxyTint,
}

/// Foreground (text / fill) tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundToken {
    /// Main copy, titles, strong labels.
    Primary,
    /// Supporting copy, metadata, inactive labels.
    Secondary,
    /// Disabled text, hints, quiet dividers.
    Muted,
    /// Live accent: active, current, focus, command hint.
    Accent,
}

/// Border / separator tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderToken {
    /// Standard separators.
    Default,
    /// Active outlines and emphasized boundaries.
    Strong,
    /// Subtle / decorative.
    Subtle,
}

/// Status / semantic tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusToken {
    /// Informational — secondary emphasis.
    Info,
    /// Success, verified, connected.
    Success,
    /// Warning.
    Warning,
    /// Error / destructive.
    Danger,
}

/// Accent depth tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentToken {
    /// Default live accent.
    Primary,
    /// Pressed / strong emphasis.
    Deep,
    /// Accent background / wash.
    Soft,
}

/// Action control tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionToken {
    /// Primary commit controls — near-black / near-white.
    Strong,
    /// Text on strong controls.
    StrongText,
}

/// Radius scale (logical units).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusToken {
    /// Progress bars, tiny state marks.
    Xs,
    /// Inputs, chips, compact controls.
    Sm,
    /// Panels, grouped controls, demo cards.
    Md,
    /// Large containers, sheets.
    Lg,
    /// Status chips and sequence markers only.
    Pill,
}

impl RadiusToken {
    pub fn value(self) -> f32 {
        match self {
            RadiusToken::Xs => 4.0,
            RadiusToken::Sm => 8.0,
            RadiusToken::Md => 12.0,
            RadiusToken::Lg => 18.0,
            RadiusToken::Pill => 999.0,
        }
    }
}

/// Spacing scale (logical units).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacingToken {
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
}

impl SpacingToken {
    pub fn value(self) -> f32 {
        match self {
            SpacingToken::S1 => 4.0,
            SpacingToken::S2 => 8.0,
            SpacingToken::S3 => 12.0,
            SpacingToken::S4 => 16.0,
            SpacingToken::S5 => 20.0,
            SpacingToken::S6 => 24.0,
            SpacingToken::S7 => 32.0,
            SpacingToken::S8 => 40.0,
        }
    }
}

/// Typography role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeRole {
    DisplayXl,
    DisplayLg,
    DisplayMd,
    BodyLg,
    BodyMd,
    LabelMd,
    MonoSm,
    MonoXs,
}

impl TypeRole {
    /// Logical size in points.
    pub fn size(self) -> f32 {
        match self {
            TypeRole::DisplayXl => 56.0,
            TypeRole::DisplayLg => 42.0,
            TypeRole::DisplayMd => 32.0,
            TypeRole::BodyLg => 21.0,
            TypeRole::BodyMd => 16.0,
            TypeRole::LabelMd => 13.0,
            TypeRole::MonoSm => 12.0,
            TypeRole::MonoXs => 10.0,
        }
    }

    /// Line-height ratio.
    pub fn line_height(self) -> f32 {
        match self {
            TypeRole::DisplayXl => 0.98,
            TypeRole::DisplayLg => 1.08,
            TypeRole::DisplayMd => 1.12,
            TypeRole::BodyLg => 1.42,
            TypeRole::BodyMd => 1.55,
            TypeRole::LabelMd => 1.15,
            TypeRole::MonoSm => 1.20,
            TypeRole::MonoXs => 1.20,
        }
    }

    /// Tracking (letter-spacing, em).
    pub fn tracking(self) -> f32 {
        match self {
            TypeRole::DisplayXl => -0.045,
            TypeRole::DisplayLg => -0.040,
            TypeRole::DisplayMd => -0.030,
            TypeRole::BodyLg => -0.015,
            TypeRole::BodyMd => 0.0,
            TypeRole::LabelMd => 0.02,
            TypeRole::MonoSm => 0.04,
            TypeRole::MonoXs => 0.06,
        }
    }
}
