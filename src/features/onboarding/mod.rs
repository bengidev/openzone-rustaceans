#![allow(dead_code)]

//! Internal onboarding module — first-run onboarding flow.
//!
//! Flat layout with `onboarding_`-prefixed modules:
//!
//! * [`onboarding_outcome`] — pure routing outcomes.
//! * [`onboarding_persistence`] — persistence trait contract.
//! * [`onboarding_state`] — state reducer.
//! * [`onboarding_messages`] — message enum.
//! * [`onboarding_dynamics`] — orb animation dynamics.
//! * [`onboarding_feature_card_dynamics`] — feature card animation helpers.
//! * [`onboarding_file_persistence`] — filesystem persistence backend.
//! * [`onboarding_memory_persistence`] — in-memory persistence backend.
//! * [`onboarding_view`] — Iced view.
//! * [`onboarding_feature_card_icon`] — wireframe feature icons.
//! * [`onboarding_galaxy_orb`] — galaxy orb canvas program.
//! * [`onboarding_scene_backdrop`] — animated scene backdrop.
//!
//! The module exposes only the composition-facing façade.

pub mod onboarding_dynamics;
pub mod onboarding_feature_card_dynamics;
pub mod onboarding_feature_card_icon;
pub mod onboarding_file_persistence;
pub mod onboarding_galaxy_orb;
pub mod onboarding_memory_persistence;
pub mod onboarding_messages;
pub mod onboarding_outcome;
pub mod onboarding_persistence;
pub mod onboarding_scene_backdrop;
pub mod onboarding_state;
pub mod onboarding_view;

pub use onboarding_messages::OnboardingMessage;
pub use onboarding_outcome::OnboardingOutcome;
pub use onboarding_persistence::OnboardingPersistence;
pub use onboarding_state::{OnboardingState, mark_completed};
pub use onboarding_view::view;

use iced::{Element, Subscription, Task, Theme};
use std::sync::Arc;

/// Launch the onboarding Iced application.
///
/// This is the high-level entry point for the composition root. It
/// wires the state, subscription, and view into an Iced window.
pub fn run(
    persistence: Arc<dyn OnboardingPersistence>,
    theme_mode: crate::shared::design::ThemeMode,
) -> iced::Result {
    iced::application(
        move || {
            let state = OnboardingState::new(persistence.clone(), theme_mode);
            (OnboardingApp { state }, Task::none())
        },
        OnboardingApp::update,
        OnboardingApp::view,
    )
    .title(OnboardingApp::title)
    .subscription(OnboardingApp::subscription)
    .theme(OnboardingApp::theme)
    .window_size(iced::Size::new(960.0, 680.0))
    .run()
}

/// Iced application wrapper for the onboarding flow.
pub struct OnboardingApp {
    state: OnboardingState,
}

impl OnboardingApp {
    fn update(&mut self, message: OnboardingMessage) -> Task<OnboardingMessage> {
        let outcome = self.state.update(message);
        match outcome {
            OnboardingOutcome::Completed => {
                let _ = mark_completed(&self.state.persistence);
                iced::exit()
            }
            OnboardingOutcome::Skipped => {
                let _ = mark_completed(&self.state.persistence);
                iced::exit()
            }
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, OnboardingMessage> {
        view(&self.state)
    }

    fn subscription(&self) -> Subscription<OnboardingMessage> {
        self.state.subscription()
    }

    fn title(&self) -> String {
        String::from("OpenZone")
    }

    fn theme(&self) -> Theme {
        match self.state.theme_mode {
            crate::shared::design::ThemeMode::Dark => Theme::Dark,
            crate::shared::design::ThemeMode::Light => Theme::Light,
        }
    }
}
