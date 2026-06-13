//! Messages produced by the onboarding view.

use std::time::Instant;

#[derive(Debug, Clone)]
pub enum OnboardingMessage {
    Tick(Instant),
    ToggleTheme,
    OrbPressed,
    OrbReleased,
    FeatureSelected(usize),
    FeatureHovered(Option<usize>),
    EnterPressed,
    Skipped,
}
