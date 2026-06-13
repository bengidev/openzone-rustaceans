#![allow(dead_code)]

//! The panel registry — the composition-root seam.
//!
//! Maps a [`PanelKind`] to a constructor that rehydrates a panel from a
//! snapshot. This is the single place features register their panel
//! factories; the shell never names a concrete feature type. It doubles
//! as the persistence seam (rebuild a saved layout) and the future
//! plugin-registration seam.
//!
//! Constructors take `&mut AppStores` so a store-backed panel
//! ([`crate::features::dummies::CounterPanel`]) can allocate its slot
//! at rehydrate time, seeded from the persisted handle.

use std::collections::HashMap;

use crate::workspace::workspace_panel::{Panel, PanelKind};
use crate::workspace::workspace_stores::AppStores;

/// Rebuilds a panel of a known kind from a persisted snapshot handle.
///
/// `&mut AppStores` is threaded through so that store-backed panels
/// (e.g. Counter) can allocate a fresh slot seeded with the persisted
/// value as part of their rehydration. Stateless panels (Text) ignore
/// the parameter; the global Clock store is only `restore`-folded.
pub type PanelConstructor = fn(serde_json::Value, &mut AppStores) -> Box<dyn Panel>;

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
    /// constructor is registered for the kind. The constructor receives
    /// `&mut stores` so a store-backed panel can allocate its slot now.
    pub fn build(
        &self,
        kind: PanelKind,
        snapshot: serde_json::Value,
        stores: &mut AppStores,
    ) -> Option<Box<dyn Panel>> {
        self.constructors
            .get(&kind)
            .map(|constructor| constructor(snapshot, stores))
    }
}
