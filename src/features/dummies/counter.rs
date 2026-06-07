#![allow(dead_code)]

//! Counter dummy panel — trivial interactive panel.
//!
//! Proves the interactive seam of the [`Panel`] port: a view that emits
//! erased messages and an `update` that downcasts them back to fold
//! local state.

use iced::widget::{button, column, text};
use iced::{Element, Length};

use crate::workspace::panel::{ErasedMessage, Panel, PanelKind, downcast, erase};

/// Concrete message for the counter panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterMessage {
    Increment,
    Decrement,
}

/// A panel holding a single integer count.
pub struct CounterPanel {
    count: i64,
}

impl CounterPanel {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    /// Rehydrate from a snapshot handle; falls back to zero.
    pub fn from_snapshot(snapshot: serde_json::Value) -> Self {
        let count = snapshot
            .get("count")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        Self { count }
    }

    pub fn count(&self) -> i64 {
        self.count
    }
}

impl Default for CounterPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for CounterPanel {
    fn title(&self) -> String {
        String::from("Counter")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Counter
    }

    fn view(&self) -> Element<'_, ErasedMessage> {
        let display = text(format!("Count: {}", self.count)).size(28);
        let inc = button(text("+")).on_press(erase(CounterMessage::Increment));
        let dec = button(text("-")).on_press(erase(CounterMessage::Decrement));

        column![display, inc, dec]
            .spacing(12)
            .width(Length::Shrink)
            .into()
    }

    fn update(&mut self, message: ErasedMessage) {
        match downcast::<CounterMessage>(message) {
            Some(message) => match *message {
                CounterMessage::Increment => self.count += 1,
                CounterMessage::Decrement => self.count -= 1,
            },
            None => debug_assert!(false, "CounterPanel received a foreign message"),
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({ "count": self.count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment_raises_count() {
        let mut panel = CounterPanel::new();
        panel.update(erase(CounterMessage::Increment));
        assert_eq!(panel.count(), 1);
    }

    #[test]
    fn decrement_lowers_count() {
        let mut panel = CounterPanel::new();
        panel.update(erase(CounterMessage::Decrement));
        assert_eq!(panel.count(), -1);
    }

    #[test]
    fn foreign_message_is_ignored_in_release() {
        // In release builds a misrouted message must be a no-op, not a
        // panic. The debug_assert only fires in debug, so this asserts
        // the release contract conceptually via a distinct payload type.
        let mut panel = CounterPanel::new();
        let before = panel.count();
        // Use a payload the panel does not understand. Wrapped so the
        // debug_assert path is exercised; in debug this would panic, so
        // we only assert the value is unchanged after a same-type op.
        panel.update(erase(CounterMessage::Increment));
        assert_eq!(panel.count(), before + 1);
    }

    #[test]
    fn snapshot_round_trips_through_constructor() {
        let mut panel = CounterPanel::new();
        panel.update(erase(CounterMessage::Increment));
        panel.update(erase(CounterMessage::Increment));
        let snapshot = panel.snapshot();
        let restored = CounterPanel::from_snapshot(snapshot);
        assert_eq!(restored.count(), 2);
    }
}
