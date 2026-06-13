//! Filesystem-backed onboarding persistence.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use super::onboarding_persistence::{OnboardingPersistence, OnboardingPersistenceError};

const SENTINEL_FILENAME: &str = "onboarding-completed.flag";

#[derive(Debug, Clone)]
pub struct FileOnboardingPersistence {
    sentinel_path: PathBuf,
}

impl FileOnboardingPersistence {
    pub fn from_project_dirs() -> Result<Self, OnboardingPersistenceError> {
        let proj_dirs = ProjectDirs::from("com", "openzone", "openzone")
            .ok_or(OnboardingPersistenceError::NoProjectDirs)?;
        Ok(Self::new_at(proj_dirs.data_dir()))
    }

    pub fn new_at<P: AsRef<Path>>(dir: P) -> Self {
        Self {
            sentinel_path: dir.as_ref().join(SENTINEL_FILENAME),
        }
    }
}

impl OnboardingPersistence for FileOnboardingPersistence {
    fn is_completed(&self) -> bool {
        self.sentinel_path.exists()
    }

    fn mark_completed(&self) -> Result<(), OnboardingPersistenceError> {
        if let Some(parent) = self.sentinel_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.sentinel_path)?;
        Ok(())
    }

    fn reset(&self) -> Result<(), OnboardingPersistenceError> {
        match fs::remove_file(&self.sentinel_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(OnboardingPersistenceError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn starts_incomplete() {
        let tmp = TempDir::new().unwrap();
        let store = FileOnboardingPersistence::new_at(tmp.path());
        assert!(!store.is_completed());
    }

    #[test]
    fn mark_completed_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let store = FileOnboardingPersistence::new_at(tmp.path());
        store.mark_completed().unwrap();
        store.mark_completed().unwrap();
        assert!(store.is_completed());
    }

    #[test]
    fn reset_clears_flag() {
        let tmp = TempDir::new().unwrap();
        let store = FileOnboardingPersistence::new_at(tmp.path());
        store.mark_completed().unwrap();
        store.reset().unwrap();
        assert!(!store.is_completed());
    }
}
