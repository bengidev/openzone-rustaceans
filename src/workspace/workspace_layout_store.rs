#![allow(dead_code)]

//! Filesystem persistence for the workspace layout snapshot.
//!
//! Mirrors the onboarding persistence pattern: a [`LayoutStore`] trait
//! names the capability (load/save/clear a [`LayoutSnapshot`]); the
//! shipped [`FileLayoutStore`] writes pretty JSON under the OS data
//! directory. The composition root loads a snapshot on boot and saves
//! one when the workspace window closes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::workspace::workspace_persistence::LayoutSnapshot;

const LAYOUT_FILENAME: &str = "workspace-layout.json";

/// Persists the workspace layout snapshot across relaunches.
pub trait LayoutStore: Send + Sync {
    /// Load the saved layout, or `None` if nothing is stored yet.
    /// A corrupt or unreadable store also yields `None` so the shell
    /// falls back to its seeded default rather than failing to launch.
    fn load(&self) -> Option<LayoutSnapshot>;

    /// Persist the given layout, replacing any prior snapshot.
    fn save(&self, snapshot: &LayoutSnapshot) -> Result<(), LayoutStoreError>;

    /// Remove the stored layout. Default no-op for out-of-tree impls.
    fn clear(&self) -> Result<(), LayoutStoreError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutStoreError {
    #[error("could not resolve a project data directory")]
    NoProjectDirs,

    #[error("io error while persisting workspace layout: {0}")]
    Io(#[from] io::Error),

    #[error("failed to serialize workspace layout: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// JSON-file-backed layout store under the OS data directory.
#[derive(Debug, Clone)]
pub struct FileLayoutStore {
    layout_path: PathBuf,
}

impl FileLayoutStore {
    /// Build a store rooted at the platform data directory, matching the
    /// onboarding store's `com.openzone.openzone` qualifier.
    pub fn from_project_dirs() -> Result<Self, LayoutStoreError> {
        let proj_dirs = ProjectDirs::from("com", "openzone", "openzone")
            .ok_or(LayoutStoreError::NoProjectDirs)?;
        Ok(Self::new_at(proj_dirs.data_dir()))
    }

    pub fn new_at<P: AsRef<Path>>(dir: P) -> Self {
        Self {
            layout_path: dir.as_ref().join(LAYOUT_FILENAME),
        }
    }
}

impl LayoutStore for FileLayoutStore {
    fn load(&self) -> Option<LayoutSnapshot> {
        let bytes = fs::read(&self.layout_path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn save(&self, snapshot: &LayoutSnapshot) -> Result<(), LayoutStoreError> {
        if let Some(parent) = self.layout_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(snapshot)?;
        fs::write(&self.layout_path, json)?;
        Ok(())
    }

    fn clear(&self) -> Result<(), LayoutStoreError> {
        match fs::remove_file(&self.layout_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(LayoutStoreError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_dock::DockVisibility;
    use crate::workspace::workspace_persistence::{
        CenterNode, DockSnapshot, FocusSnapshot, PaneSnapshot,
    };
    use tempfile::TempDir;

    fn sample_snapshot() -> LayoutSnapshot {
        let empty_dock = DockSnapshot {
            tabs: PaneSnapshot {
                tabs: Vec::new(),
                active: 0,
            },
            visibility: DockVisibility::Hidden,
            extent: crate::workspace::workspace_layout_metrics::SIDE_DOCK_WIDTH,
        };
        LayoutSnapshot {
            center: CenterNode::Pane(PaneSnapshot {
                tabs: Vec::new(),
                active: 0,
            }),
            left: empty_dock.clone(),
            right: empty_dock.clone(),
            bottom: empty_dock,
            focus: FocusSnapshot::Center(0),
        }
    }

    #[test]
    fn load_is_none_before_any_save() {
        let tmp = TempDir::new().unwrap();
        let store = FileLayoutStore::new_at(tmp.path());
        assert!(store.load().is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let tmp = TempDir::new().unwrap();
        let store = FileLayoutStore::new_at(tmp.path());
        let snapshot = sample_snapshot();

        store.save(&snapshot).unwrap();
        let loaded = store.load().expect("a snapshot was saved");

        assert_eq!(loaded, snapshot);
    }

    #[test]
    fn save_creates_missing_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("a").join("b");
        let store = FileLayoutStore::new_at(&nested);

        store.save(&sample_snapshot()).unwrap();
        assert!(nested.join(LAYOUT_FILENAME).exists());
    }

    #[test]
    fn corrupt_file_loads_as_none() {
        let tmp = TempDir::new().unwrap();
        let store = FileLayoutStore::new_at(tmp.path());
        fs::write(tmp.path().join(LAYOUT_FILENAME), b"{ not valid json").unwrap();

        assert!(store.load().is_none());
    }

    #[test]
    fn clear_removes_the_snapshot() {
        let tmp = TempDir::new().unwrap();
        let store = FileLayoutStore::new_at(tmp.path());
        store.save(&sample_snapshot()).unwrap();

        store.clear().unwrap();
        assert!(store.load().is_none());
    }

    #[test]
    fn clear_is_ok_when_nothing_stored() {
        let tmp = TempDir::new().unwrap();
        let store = FileLayoutStore::new_at(tmp.path());
        assert!(store.clear().is_ok());
    }
}
