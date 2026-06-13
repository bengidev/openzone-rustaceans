#![allow(dead_code)]

//! Clock dummy panel — view over the global [`ClockStore`] tick.
//!
//! Stateless from the panel's point of view: it holds nothing and reads
//! `stores.clock.ticks()` on every render. The 1 Hz timer that drives
//! the tick lives **once at the workspace level** — gated on whether
//! any Clock panel exists — and mutates [`ClockStore`] exactly once per
//! tick. Every Clock panel observes the same value (single-source fan-
//! out); removing the last Clock tab also removes the addressing reason
//! for the subscription, so Iced stops it without orphan streams.

use iced::widget::{column, text};
use iced::{Element, Length, Subscription};

use crate::workspace::workspace_panel::{ErasedMessage, Panel, PanelKind};
use crate::workspace::workspace_stores::AppStores;

/// A panel that reads the global tick count from [`ClockStore`]. It
/// holds no per-instance state — every Clock panel observes the same
/// store value, which is the property the issue's "single store-level
/// Clock subscription" point relies on.
pub struct ClockPanel;

impl ClockPanel {
    pub fn new() -> Self {
        Self
    }

    /// Rehydrate from a snapshot handle. The Clock store is global, so
    /// the persisted tick count is folded into [`ClockStore::restore`]
    /// (max-of-seen) rather than a per-panel field.
    pub fn from_snapshot(snapshot: serde_json::Value, stores: &mut AppStores) -> Self {
        let ticks = snapshot
            .get("ticks")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        stores.clock.restore(ticks);
        Self
    }
}

impl Default for ClockPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for ClockPanel {
    fn title(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("Clock")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Clock
    }

    fn view<'a>(&'a self, stores: &'a AppStores) -> Element<'a, ErasedMessage> {
        let display = text(format!("Ticks: {}", stores.clock.ticks())).size(28);
        let hint = text("updates once per second").size(12);

        column![display, hint]
            .spacing(8)
            .width(Length::Shrink)
            .into()
    }

    fn update(&mut self, _message: ErasedMessage, _stores: &mut AppStores) {
        // Clock has no per-panel intents; the global tick is driven by
        // the workspace-level subscription, not by panel messages. A
        // message arriving here is a routing bug.
        debug_assert!(false, "ClockPanel does not consume per-panel messages");
    }

    /// No per-panel subscription. The 1 Hz tick lives at the workspace
    /// layer so a single timer fans out to every Clock panel rather
    /// than spawning N parallel timers — see
    /// `Workspace::subscription`.
    fn subscription(&self) -> Subscription<ErasedMessage> {
        Subscription::none()
    }

    fn snapshot(&self, stores: &AppStores) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "ticks": stores.clock.ticks() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_reads_global_tick() {
        let mut stores = AppStores::new();
        let panel = ClockPanel::new();

        // The store advances; the panel reflects the new value with no
        // panel-side bookkeeping.
        stores.clock.tick();
        stores.clock.tick();

        assert_eq!(
            panel
                .snapshot(&stores)
                .expect("durable")
                .get("ticks")
                .and_then(|v| v.as_u64()),
            Some(2)
        );
    }

    #[test]
    fn restore_brings_global_tick_forward() {
        let mut stores = AppStores::new();
        let _panel = ClockPanel::from_snapshot(serde_json::json!({ "ticks": 5 }), &mut stores);
        assert_eq!(stores.clock.ticks(), 5);
    }

    #[test]
    fn two_panels_share_one_store_value() {
        let mut stores = AppStores::new();
        let a = ClockPanel::new();
        let b = ClockPanel::new();
        stores.clock.tick();

        // Both observers report the same value: the issue's
        // "every Clock panel reads the same value" invariant.
        assert_eq!(a.snapshot(&stores), b.snapshot(&stores));
        assert_eq!(
            a.snapshot(&stores)
                .expect("durable")
                .get("ticks")
                .and_then(|v| v.as_u64()),
            Some(1)
        );
    }
}
