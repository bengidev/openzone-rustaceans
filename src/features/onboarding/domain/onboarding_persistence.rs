//! Persistence contract for the onboarding flag.
//!
//! The onboarding window is shown once on first launch. After the
//! user dismisses it, the implementation marks a sentinel; on
//! subsequent launches the router queries this trait and routes
//! straight to the main experience.

use std::io;

/// Persists whether the onboarding has been completed.
pub trait OnboardingPersistence: Send + Sync {
    /// Returns `true` if onboarding has already been completed.
    fn is_completed(&self) -> bool;

    /// Marks onboarding as completed. Idempotent.
    fn mark_completed(&self) -> Result<(), OnboardingPersistenceError>;

    /// Clears the flag so onboarding reappears on next launch.
    /// Default is a no-op for out-of-tree implementations.
    fn reset(&self) -> Result<(), OnboardingPersistenceError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OnboardingPersistenceError {
    #[error("could not resolve a project data directory")]
    NoProjectDirs,

    #[error("io error while persisting onboarding state: {0}")]
    Io(#[from] io::Error),
}
