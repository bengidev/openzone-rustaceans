//! Infrastructure layer — concrete persistence backends.

pub mod file_persistence;
pub mod memory_persistence;

pub use file_persistence::FileOnboardingPersistence;
pub use memory_persistence::InMemoryOnboardingPersistence;
