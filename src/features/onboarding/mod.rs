#![allow(dead_code)]

//! Internal onboarding module — first-run onboarding flow.
//!
//! Layered along Clean Architecture lines:
//!
//! * [`domain`] — pure contracts: outcomes, persistence trait.
//! * [`application`] — state reducer, orb dynamics, slide navigation.
//! * [`infrastructure`] — concrete persistence backends.
//! * [`presenter`] — Iced view + canvas program.
//!
//! The module exposes only the composition-facing façade.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presenter;

pub use application::{OnboardingMessage, OnboardingState, mark_completed};
pub use domain::{OnboardingOutcome, OnboardingPersistence};
pub use infrastructure::{FileOnboardingPersistence, InMemoryOnboardingPersistence};
pub use presenter::view;

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
    .window_size(iced::Size::new(840.0, 820.0))
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
            crate::shared::design::ThemeMode::Dark => Theme::TokyoNight,
            crate::shared::design::ThemeMode::Light => Theme::CatppuccinLatte,
        }
    }
}
