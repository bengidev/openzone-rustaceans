//! Application layer — state reducer and orb dynamics.

pub mod feature_card_dynamics;
pub mod onboarding_dynamics;
pub mod onboarding_messages;
pub mod onboarding_state;

pub use onboarding_messages::OnboardingMessage;
pub use onboarding_state::{OnboardingState, mark_completed};
