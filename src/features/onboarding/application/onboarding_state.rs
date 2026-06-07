//! Onboarding state reducer.
//!
//! Single-page onboarding with hold-to-zoom orb interaction and
//! selectable main-feature highlight cards.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Subscription;

use crate::shared::design::{OpenZoneTheme, ThemeMode};

use crate::features::onboarding::application::feature_card_dynamics::{approach, highlight_target};
use crate::features::onboarding::application::onboarding_dynamics::dynamics_for_progress;
use crate::features::onboarding::application::onboarding_messages::OnboardingMessage;
use crate::features::onboarding::domain::{
    OnboardingOutcome, OnboardingPersistence, OnboardingPersistenceError,
};

/// Number of main-feature highlight cards on the onboarding page.
pub const FEATURE_COUNT: usize = 4;

pub struct OnboardingState {
    pub theme: OpenZoneTheme,
    pub theme_mode: ThemeMode,
    pub started_at: Instant,
    pub now: Instant,
    pub persistence: Arc<dyn OnboardingPersistence>,
    pub selected_feature: usize,
    pub hovered_feature: Option<usize>,
    pub feature_glow: [f32; FEATURE_COUNT],
    pub is_holding: bool,
    pub hold_progress: f32,
    pub displayed_speed: f32,
    pub displayed_zoom: f32,
}

impl std::fmt::Debug for OnboardingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnboardingState")
            .field("theme_mode", &self.theme_mode)
            .field("selected_feature", &self.selected_feature)
            .field("hovered_feature", &self.hovered_feature)
            .field("is_holding", &self.is_holding)
            .field("hold_progress", &self.hold_progress)
            .finish()
    }
}

impl OnboardingState {
    pub fn new(persistence: Arc<dyn OnboardingPersistence>, theme_mode: ThemeMode) -> Self {
        let now = Instant::now();
        let (initial_speed, initial_zoom) = dynamics_for_progress(0.0);
        Self {
            theme: OpenZoneTheme::from_mode(theme_mode),
            theme_mode,
            started_at: now,
            now,
            persistence,
            selected_feature: 0,
            hovered_feature: None,
            feature_glow: [0.0; FEATURE_COUNT],
            is_holding: false,
            hold_progress: 0.0,
            displayed_speed: initial_speed,
            displayed_zoom: initial_zoom,
        }
    }

    pub fn update(&mut self, message: OnboardingMessage) -> OnboardingOutcome {
        match message {
            OnboardingMessage::Tick(now) => {
                let dt = now.saturating_duration_since(self.now).as_secs_f32();
                self.now = now;
                self.advance_orb_progress(dt);
                self.advance_feature_glow(dt);
                OnboardingOutcome::None
            }
            OnboardingMessage::ToggleTheme => {
                self.theme_mode = self.theme_mode.toggle();
                self.theme = OpenZoneTheme::from_mode(self.theme_mode);
                OnboardingOutcome::ThemeToggled(self.theme_mode)
            }
            OnboardingMessage::OrbPressed => {
                self.is_holding = true;
                OnboardingOutcome::None
            }
            OnboardingMessage::OrbReleased => {
                self.is_holding = false;
                OnboardingOutcome::None
            }
            OnboardingMessage::FeatureSelected(index) => {
                if index < FEATURE_COUNT {
                    self.selected_feature = index;
                }
                OnboardingOutcome::None
            }
            OnboardingMessage::FeatureHovered(index) => {
                self.hovered_feature = index.filter(|i| *i < FEATURE_COUNT);
                OnboardingOutcome::None
            }
            OnboardingMessage::EnterPressed => OnboardingOutcome::Completed,
            OnboardingMessage::Skipped => OnboardingOutcome::Skipped,
        }
    }

    fn advance_orb_progress(&mut self, dt: f32) {
        const HOLD_RAMP_PER_SEC: f32 = 0.6;
        const RELEASE_RAMP_PER_SEC: f32 = 0.9;

        let dt = dt.clamp(0.0, 0.25);
        let delta = if self.is_holding {
            HOLD_RAMP_PER_SEC * dt
        } else {
            -RELEASE_RAMP_PER_SEC * dt
        };
        self.hold_progress = (self.hold_progress + delta).clamp(0.0, 1.0);

        let (speed, zoom) = dynamics_for_progress(self.hold_progress);
        self.displayed_speed = speed;
        self.displayed_zoom = zoom;
    }

    fn advance_feature_glow(&mut self, dt: f32) {
        for index in 0..FEATURE_COUNT {
            let hovered = self.hovered_feature == Some(index);
            let selected = self.selected_feature == index;
            let target = highlight_target(selected, hovered);
            self.feature_glow[index] = approach(self.feature_glow[index], target, dt, 9.0);
        }
    }

    pub fn subscription(&self) -> Subscription<OnboardingMessage> {
        iced::time::every(Duration::from_millis(1)).map(OnboardingMessage::Tick)
    }
}

pub fn mark_completed(
    persistence: &Arc<dyn OnboardingPersistence>,
) -> Result<(), OnboardingPersistenceError> {
    persistence.mark_completed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::onboarding::infrastructure::memory_persistence::InMemoryOnboardingPersistence;

    #[test]
    fn enter_yields_completed() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        let outcome = state.update(OnboardingMessage::EnterPressed);
        assert_eq!(outcome, OnboardingOutcome::Completed);
    }

    #[test]
    fn feature_selection_updates_index() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        assert_eq!(state.selected_feature, 0);
        state.update(OnboardingMessage::FeatureSelected(2));
        assert_eq!(state.selected_feature, 2);
    }

    #[test]
    fn feature_selection_clamps_to_valid_range() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        state.update(OnboardingMessage::FeatureSelected(99));
        assert_eq!(state.selected_feature, 0);
    }
}
