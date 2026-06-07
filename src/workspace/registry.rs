#![allow(dead_code)]

//! The panel registry — the composition-root seam.
//!
//! Maps a [`PanelKind`] to a constructor that rehydrates a panel from a
//! snapshot. This is the single place features register their panel
//! factories; the shell never names a concrete feature type. It doubles
//! as the persistence seam (rebuild a saved layout) and the future
//! plugin-registration seam.

use std::collections::HashMap;

use crate::workspace::panel::{Panel, PanelKind};

/// Rebuilds a panel of a known kind from a persisted snapshot handle.
pub type PanelConstructor = fn(serde_json::Value) -> Box<dyn Panel>;

/// `PanelKind -> constructor` table wired at the composition root.
#[derive(Default, Clone)]
pub struct PanelRegistry {
    constructors: HashMap<PanelKind, PanelConstructor>,
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self {
            constructors: HashMap::new(),
        }
    }

    /// Register a constructor for a kind. Returns `&mut self` to allow
    /// fluent chaining at the composition root.
    pub fn register(&mut self, kind: PanelKind, constructor: PanelConstructor) -> &mut Self {
        self.constructors.insert(kind, constructor);
        self
    }

    /// Whether a kind has a registered constructor.
    pub fn contains(&self, kind: PanelKind) -> bool {
        self.constructors.contains_key(&kind)
    }

    /// Build a panel of `kind` from `snapshot`. Returns `None` if no
    /// constructor is registered for the kind.
    pub fn build(&self, kind: PanelKind, snapshot: serde_json::Value) -> Option<Box<dyn Panel>> {
        self.constructors
            .get(&kind)
            .map(|constructor| constructor(snapshot))
    }
}
