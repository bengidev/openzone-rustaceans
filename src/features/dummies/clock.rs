#![allow(dead_code)]

//! Clock dummy panel — ticks via a panel-level subscription.
//!
//! Proves the [`Panel::subscription`] seam: the panel exposes a timer
//! subscription, the workspace batches it, and Iced starts/stops it
//! with the panel's lifecycle. Stands in for LLM-token / PTY streams
//! without building a real streaming feature.

use std::time::Duration;

use iced::widget::{column, text};
use iced::{Element, Length, Subscription};

use crate::workspace::panel::{downcast, erase, ErasedMessage, Panel, PanelKind};

/// Concrete message for the clock panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockMessage {
    Tick,
}

/// A panel counting subscription ticks since creation.
pub struct ClockPanel {
    ticks: u64,
}

impl ClockPanel {
    pub fn new() -> Self {
        Self { ticks: 0 }
    }

    /// Rehydrate from a snapshot handle; falls back to zero.
    pub fn from_snapshot(snapshot: serde_json::Value) -> Self {
        let ticks = snapshot
            .get("ticks")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        Self { ticks }
    }

    pub fn ticks(&self) -> u64 {
        self.ticks
    }
}

impl Default for ClockPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for ClockPanel {
    fn title(&self) -> String {
        String::from("Clock")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Clock
    }

    fn view(&self) -> Element<'_, ErasedMessage> {
        let display = text(format!("Ticks: {}", self.ticks)).size(28);
        let hint = text("updates once per second").size(12);

        column![display, hint]
            .spacing(8)
            .width(Length::Shrink)
            .into()
    }

    fn update(&mut self, message: ErasedMessage) {
        match downcast::<ClockMessage>(message) {
            Some(message) => match *message {
                ClockMessage::Tick => self.ticks += 1,
            },
            None => debug_assert!(false, "ClockPanel received a foreign message"),
        }
    }

    fn subscription(&self) -> Subscription<ErasedMessage> {
        iced::time::every(Duration::from_secs(1)).map(|_| erase(ClockMessage::Tick))
    }

    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({ "ticks": self.ticks })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_advances_count() {
        let mut panel = ClockPanel::new();
        panel.update(erase(ClockMessage::Tick));
        panel.update(erase(ClockMessage::Tick));
        assert_eq!(panel.ticks(), 2);
    }

    #[test]
    fn snapshot_round_trips_through_constructor() {
        let mut panel = ClockPanel::new();
        panel.update(erase(ClockMessage::Tick));
        let restored = ClockPanel::from_snapshot(panel.snapshot());
        assert_eq!(restored.ticks(), 1);
    }
}
