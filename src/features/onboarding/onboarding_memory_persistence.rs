//! In-memory onboarding persistence for tests and previews.

use std::sync::{Arc, Mutex};

use super::onboarding_persistence::{OnboardingPersistence, OnboardingPersistenceError};

#[derive(Debug, Clone, Default)]
pub struct InMemoryOnboardingPersistence {
    completed: Arc<Mutex<bool>>,
}

impl InMemoryOnboardingPersistence {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn already_completed() -> Self {
        Self {
            completed: Arc::new(Mutex::new(true)),
        }
    }
}

impl OnboardingPersistence for InMemoryOnboardingPersistence {
    fn is_completed(&self) -> bool {
        *self.completed.lock().unwrap()
    }

    fn mark_completed(&self) -> Result<(), OnboardingPersistenceError> {
        *self.completed.lock().unwrap() = true;
        Ok(())
    }

    fn reset(&self) -> Result<(), OnboardingPersistenceError> {
        *self.completed.lock().unwrap() = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let store = InMemoryOnboardingPersistence::new();
        assert!(!store.is_completed());
        store.mark_completed().unwrap();
        assert!(store.is_completed());
    }

    #[test]
    fn reset_clears() {
        let store = InMemoryOnboardingPersistence::already_completed();
        store.reset().unwrap();
        assert!(!store.is_completed());
    }
}
