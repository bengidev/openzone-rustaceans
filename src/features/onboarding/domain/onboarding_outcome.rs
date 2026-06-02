//! Routing outcomes from the onboarding reducer.
//!
//! Keeping routing decisions out of the state lets the reducer stay
//! pure: tests can drive the flow without touching the filesystem or
//! host application.

use crate::shared::design::ThemeMode;

/// What the parent router should do after dispatching a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingOutcome {
    /// State updated locally; no transition.
    None,
    /// User toggled the theme.
    ThemeToggled(ThemeMode),
    /// User accepted (primary CTA on final slide).
    Completed,
    /// User skipped.
    Skipped,
}
