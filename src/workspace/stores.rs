#![allow(dead_code)]

//! App-root domain stores — the single owners of Counter and Clock state.
//!
//! These stores are **app-root** state: in the live application they are
//! sibling fields on `OpenZone`, never owned by [`Workspace`]. The
//! workspace borrows them as `&mut AppStores` through its `update` path
//! (single-writer, no interior mutability locks). Panels become view-
//! over-handle: a [`crate::features::dummies::CounterPanel`] holds a
//! [`CounterId`] and reads the count from [`CounterStore`]; a
//! [`crate::features::dummies::ClockPanel`] reads the global tick from
//! [`ClockStore`].
//!
//! Keeping stores at app root rather than inside [`Workspace`] is what
//! lets the reducer split-borrow them alongside per-window workspace
//! state without entangling the two.

use std::collections::HashMap;

/// Stable handle assigned to each [`crate::features::dummies::CounterPanel`].
///
/// Multiple Counter panels may exist; each owns an independent count
/// keyed by this id. Ids are monotonically allocated and never reused.
pub type CounterId = u64;

/// The app-root counter store: one count per allocated [`CounterId`].
#[derive(Debug, Default, Clone)]
pub struct CounterStore {
    next_id: u64,
    counts: HashMap<CounterId, i64>,
}

impl CounterStore {
    /// Build an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a fresh counter, seeded to zero. The returned id is the
    /// rehydration handle the panel later uses to read and address its
    /// count through the store.
    pub fn create(&mut self) -> CounterId {
        let id = self.next_id;
        self.next_id += 1;
        self.counts.insert(id, 0);
        id
    }

    /// Allocate a fresh counter seeded to `count`. Used when a panel is
    /// rehydrated from a layout snapshot so it lights up at its persisted
    /// value rather than zero.
    pub fn restore(&mut self, count: i64) -> CounterId {
        let id = self.next_id;
        self.next_id += 1;
        self.counts.insert(id, count);
        id
    }

    /// Release a counter slot. Called when the addressing panel is
    /// closed; a stale read after release returns `None` and panel views
    /// degrade gracefully to `0`.
    pub fn release(&mut self, id: CounterId) {
        self.counts.remove(&id);
    }

    /// Current count for a counter, if its slot is still live.
    pub fn count(&self, id: CounterId) -> Option<i64> {
        self.counts.get(&id).copied()
    }

    /// `+1` to the addressed counter. Stale ids are a silent no-op so a
    /// late-arriving intent for a closed panel does nothing.
    pub fn increment(&mut self, id: CounterId) {
        if let Some(value) = self.counts.get_mut(&id) {
            *value = value.saturating_add(1);
        }
    }

    /// `-1` to the addressed counter. Stale ids are a silent no-op.
    pub fn decrement(&mut self, id: CounterId) {
        if let Some(value) = self.counts.get_mut(&id) {
            *value = value.saturating_sub(1);
        }
    }

    /// Number of live counters. Test/diagnostics only.
    pub fn len(&self) -> usize {
        self.counts.len()
    }
}

/// The app-root clock store. Exactly one global tick counter — every
/// Clock panel reads the same value.
///
/// There is no per-panel clock id: Clock panels are pure observers of
/// the single tick number, so the store's identity *is* its tick count.
/// A single workspace-level subscription drives `tick` once per second
/// and every Clock panel re-renders with the new value.
#[derive(Debug, Default, Clone)]
pub struct ClockStore {
    ticks: u64,
}

impl ClockStore {
    /// Build a store at zero ticks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the global tick counter by one. Called exactly once per
    /// store-level subscription tick from the workspace reducer.
    pub fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }

    /// Current global tick count.
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Bring the tick counter forward to at least `ticks` if a panel is
    /// rehydrated from a snapshot. Multiple Clock panels may rehydrate
    /// from differing snapshots; the highest persisted tick wins so the
    /// fan-out invariant ("every Clock panel reads the same value")
    /// holds the moment restore completes.
    pub fn restore(&mut self, ticks: u64) {
        if ticks > self.ticks {
            self.ticks = ticks;
        }
    }
}

/// The two app-root stores bundled for borrow ergonomics.
///
/// Composition root pattern: hold one [`AppStores`] field on the app
/// root next to the [`crate::workspace::Workspace`], and pass
/// `&mut AppStores` into the workspace reducer. The split borrow
/// between `app.workspace` and `app.stores` is what the issue's
/// "sibling fields on the app root" guidance buys.
#[derive(Debug, Default, Clone)]
pub struct AppStores {
    pub counter: CounterStore,
    pub clock: ClockStore,
}

impl AppStores {
    /// Build an empty store bundle.
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_create_starts_at_zero() {
        let mut store = CounterStore::new();
        let id = store.create();
        assert_eq!(store.count(id), Some(0));
    }

    #[test]
    fn counter_increment_and_decrement_round_trip() {
        let mut store = CounterStore::new();
        let id = store.create();
        store.increment(id);
        store.increment(id);
        store.decrement(id);
        assert_eq!(store.count(id), Some(1));
    }

    #[test]
    fn counter_restore_seeds_initial_value() {
        let mut store = CounterStore::new();
        let id = store.restore(7);
        assert_eq!(store.count(id), Some(7));
    }

    #[test]
    fn counter_release_drops_slot() {
        let mut store = CounterStore::new();
        let id = store.create();
        store.release(id);
        assert_eq!(store.count(id), None);
    }

    #[test]
    fn counter_intent_to_released_id_is_noop() {
        let mut store = CounterStore::new();
        let id = store.create();
        store.release(id);
        store.increment(id); // must not panic, must not resurrect.
        assert_eq!(store.count(id), None);
    }

    #[test]
    fn counter_ids_are_unique_and_independent() {
        let mut store = CounterStore::new();
        let a = store.create();
        let b = store.create();
        assert_ne!(a, b);

        store.increment(a);
        store.increment(a);
        store.increment(b);
        assert_eq!(store.count(a), Some(2));
        assert_eq!(store.count(b), Some(1));
    }

    #[test]
    fn clock_tick_advances_global_counter() {
        let mut store = ClockStore::new();
        store.tick();
        store.tick();
        assert_eq!(store.ticks(), 2);
    }

    #[test]
    fn clock_restore_keeps_max_seen_value() {
        let mut store = ClockStore::new();
        store.tick();
        store.tick();
        store.restore(1); // older snapshot — must not lower the count.
        assert_eq!(store.ticks(), 2);
        store.restore(7); // newer snapshot — adopt it.
        assert_eq!(store.ticks(), 7);
    }
}
