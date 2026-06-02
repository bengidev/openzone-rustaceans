//! Onboarding state reducer.
//!
//! Multi-slide navigation with hold-to-zoom orb interaction.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Subscription;

use crate::shared::design::{OpenZoneTheme, ThemeMode};

use crate::features::onboarding::application::onboarding_dynamics::dynamics_for_progress;
use crate::features::onboarding::application::onboarding_messages::OnboardingMessage;
use crate::features::onboarding::domain::{
    OnboardingOutcome, OnboardingPersistence, OnboardingPersistenceError,
};

/// Total number of onboarding slides.
pub const SLIDE_COUNT: usize = 4;

pub struct OnboardingState {
    pub theme: OpenZoneTheme,
    pub theme_mode: ThemeMode,
    pub started_at: Instant,
    pub now: Instant,
    pub persistence: Arc<dyn OnboardingPersistence>,
    pub current_slide: usize,
    pub is_holding: bool,
    pub hold_progress: f32,
    pub displayed_speed: f32,
    pub displayed_zoom: f32,
}

impl std::fmt::Debug for OnboardingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnboardingState")
            .field("theme_mode", &self.theme_mode)
            .field("current_slide", &self.current_slide)
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
            current_slide: 0,
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
            OnboardingMessage::NextSlide => {
                if self.current_slide < SLIDE_COUNT - 1 {
                    self.current_slide += 1;
                }
                OnboardingOutcome::None
            }
            OnboardingMessage::PreviousSlide => {
                if self.current_slide > 0 {
                    self.current_slide -= 1;
                }
                OnboardingOutcome::None
            }
            OnboardingMessage::EnterPressed => {
                if self.current_slide == SLIDE_COUNT - 1 {
                    OnboardingOutcome::Completed
                } else {
                    self.current_slide += 1;
                    OnboardingOutcome::None
                }
            }
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

    pub fn subscription(&self) -> Subscription<OnboardingMessage> {
        iced::time::every(Duration::from_millis(33)).map(OnboardingMessage::Tick)
    }

    pub fn is_final_slide(&self) -> bool {
        self.current_slide == SLIDE_COUNT - 1
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
    use crate::features::onboarding::infrastructure::InMemoryOnboardingPersistence;

    #[test]
    fn enter_on_final_slide_yields_completed() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        state.current_slide = SLIDE_COUNT - 1;
        let outcome = state.update(OnboardingMessage::EnterPressed);
        assert_eq!(outcome, OnboardingOutcome::Completed);
    }

    #[test]
    fn enter_advances_slide_before_final() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        assert_eq!(state.current_slide, 0);
        let outcome = state.update(OnboardingMessage::EnterPressed);
        assert_eq!(outcome, OnboardingOutcome::None);
        assert_eq!(state.current_slide, 1);
    }

    #[test]
    fn next_slide_clamps_at_end() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        state.current_slide = SLIDE_COUNT - 1;
        state.update(OnboardingMessage::NextSlide);
        assert_eq!(state.current_slide, SLIDE_COUNT - 1);
    }

    #[test]
    fn previous_slide_clamps_at_start() {
        let mut state = OnboardingState::new(
            Arc::new(InMemoryOnboardingPersistence::new()),
            ThemeMode::Dark,
        );
        assert_eq!(state.current_slide, 0);
        state.update(OnboardingMessage::PreviousSlide);
        assert_eq!(state.current_slide, 0);
    }
}
