#![allow(dead_code)]

//! Counter dummy panel — view over an app-root [`CounterStore`] slot.
//!
//! The panel is a *handle* (a [`CounterId`]) plus a render. It owns no
//! count: the canonical value lives in [`AppStores::counter`] at app
//! root. The view emits intents (`CounterMessage::Increment`/
//! `Decrement`) erased at the trait boundary; the workspace reducer
//! lifts each intent through [`Panel::update`] into a single
//! [`CounterStore`] mutation. There is no interior mutability anywhere.

use iced::widget::{button, column, text};
use iced::{Element, Length};

use crate::workspace::workspace_panel::{ErasedMessage, Panel, PanelKind, downcast, erase};
use crate::workspace::workspace_stores::{AppStores, CounterId};

/// Concrete intent for the counter panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterMessage {
    Increment,
    Decrement,
}

/// A panel addressing one slot in [`crate::workspace::workspace_stores::CounterStore`].
pub struct CounterPanel {
    id: CounterId,
}

impl CounterPanel {
    /// Allocate a fresh counter slot and bind this panel to it.
    pub fn new(stores: &mut AppStores) -> Self {
        Self {
            id: stores.counter.create(),
        }
    }

    /// Bind this panel to an existing live counter slot.
    ///
    /// Used when a second workspace window should observe the same
    /// app-root count as a panel in another window.
    pub fn with_id(id: CounterId) -> Self {
        Self { id }
    }

    /// Rehydrate from a snapshot handle: read the persisted count and
    /// allocate a store slot seeded with it. A missing or malformed
    /// snapshot falls back to zero so a corrupt layout file degrades
    /// gracefully rather than panicking on launch.
    pub fn from_snapshot(snapshot: serde_json::Value, stores: &mut AppStores) -> Self {
        let count = snapshot
            .get("count")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        Self {
            id: stores.counter.restore(count),
        }
    }

    /// The store id this panel addresses.
    pub fn id(&self) -> CounterId {
        self.id
    }
}

impl Panel for CounterPanel {
    fn title(&self) -> String {
        String::from("Counter")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Counter
    }

    fn view<'a>(&'a self, stores: &'a AppStores) -> Element<'a, ErasedMessage> {
        let count = stores.counter.count(self.id).unwrap_or(0);

        let display = text(format!("Count: {count}")).size(28);
        let inc = button(text("+")).on_press(erase(CounterMessage::Increment));
        let dec = button(text("-")).on_press(erase(CounterMessage::Decrement));

        column![display, inc, dec]
            .spacing(12)
            .width(Length::Shrink)
            .into()
    }

    fn update(&mut self, message: ErasedMessage, stores: &mut AppStores) {
        match downcast::<CounterMessage>(message) {
            Some(message) => match *message {
                CounterMessage::Increment => stores.counter.increment(self.id),
                CounterMessage::Decrement => stores.counter.decrement(self.id),
            },
            None => debug_assert!(false, "CounterPanel received a foreign message"),
        }
    }

    fn snapshot(&self, stores: &AppStores) -> Option<serde_json::Value> {
        let count = stores.counter.count(self.id).unwrap_or(0);
        Some(serde_json::json!({ "count": count }))
    }

    fn on_close(&mut self, stores: &mut AppStores) {
        stores.counter.release(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_intent_mutates_store_through_panel() {
        let mut stores = AppStores::new();
        let mut panel = CounterPanel::new(&mut stores);

        panel.update(erase(CounterMessage::Increment), &mut stores);

        assert_eq!(stores.counter.count(panel.id()), Some(1));
    }

    #[test]
    fn decrement_intent_mutates_store_through_panel() {
        let mut stores = AppStores::new();
        let mut panel = CounterPanel::new(&mut stores);

        panel.update(erase(CounterMessage::Decrement), &mut stores);

        assert_eq!(stores.counter.count(panel.id()), Some(-1));
    }

    #[test]
    fn snapshot_reads_from_store_not_panel() {
        let mut stores = AppStores::new();
        let panel = CounterPanel::new(&mut stores);
        // Mutate the store directly; the panel is a view, so its
        // snapshot must reflect the new store value with zero panel-side
        // bookkeeping.
        stores.counter.increment(panel.id());
        stores.counter.increment(panel.id());

        let snapshot = panel.snapshot(&stores).expect("durable");

        assert_eq!(snapshot.get("count").and_then(|v| v.as_i64()), Some(2));
    }

    #[test]
    fn snapshot_round_trips_through_constructor() {
        let mut stores_a = AppStores::new();
        let mut panel = CounterPanel::new(&mut stores_a);
        panel.update(erase(CounterMessage::Increment), &mut stores_a);
        panel.update(erase(CounterMessage::Increment), &mut stores_a);
        let snapshot = panel.snapshot(&stores_a).expect("durable");

        // Rehydrating in a fresh store seeds a slot at the persisted
        // count: the snapshot is the only thing crossing the boundary.
        let mut stores_b = AppStores::new();
        let restored = CounterPanel::from_snapshot(snapshot, &mut stores_b);
        assert_eq!(stores_b.counter.count(restored.id()), Some(2));
    }

    #[test]
    fn on_close_releases_store_slot() {
        let mut stores = AppStores::new();
        let mut panel = CounterPanel::new(&mut stores);
        let id = panel.id();
        assert_eq!(stores.counter.count(id), Some(0));

        panel.on_close(&mut stores);

        assert_eq!(stores.counter.count(id), None);
    }

    #[test]
    fn two_panels_observe_independent_slots() {
        let mut stores = AppStores::new();
        let mut a = CounterPanel::new(&mut stores);
        let mut b = CounterPanel::new(&mut stores);

        a.update(erase(CounterMessage::Increment), &mut stores);
        a.update(erase(CounterMessage::Increment), &mut stores);
        b.update(erase(CounterMessage::Increment), &mut stores);

        assert_eq!(stores.counter.count(a.id()), Some(2));
        assert_eq!(stores.counter.count(b.id()), Some(1));
    }
}
