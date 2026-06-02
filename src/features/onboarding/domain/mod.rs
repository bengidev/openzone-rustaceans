//! Domain layer — pure contracts only.

pub mod onboarding_outcome;
pub mod onboarding_persistence;

pub use onboarding_outcome::OnboardingOutcome;
pub use onboarding_persistence::{OnboardingPersistence, OnboardingPersistenceError};
