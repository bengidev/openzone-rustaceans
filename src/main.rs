//! OpenZone Rustaceans — composition root.
//!
//! This binary composes internal modules, chooses infrastructure, and
//! launches the Iced application.

mod features;
mod shared;

use std::sync::Arc;

use crate::features::onboarding::{
    FileOnboardingPersistence, InMemoryOnboardingPersistence, OnboardingPersistence,
};
use crate::shared::design::ThemeMode;

fn main() -> iced::Result {
    let persistence: Arc<dyn OnboardingPersistence> =
        match FileOnboardingPersistence::from_project_dirs() {
            Ok(p) => Arc::new(p),
            Err(_) => Arc::new(InMemoryOnboardingPersistence::new()),
        };

    features::onboarding::run(persistence, ThemeMode::Dark)
}
