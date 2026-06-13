#![allow(dead_code)]

//! Scratch panel — the non-durable fallback every workspace starts with.
//!
//! A `ScratchPanel` is the empty-canvas placeholder shown when no
//! domain panel has been opened yet. It has no state of its own, no
//! persistence payload, and no meaningful messages. The composition
//! root wires a factory for it into the workspace so that every center
//! pane is guaranteed to have at least one tab on startup.

use iced::widget::{container, text};
use iced::{Element, Length};

use crate::workspace::panel::{ErasedMessage, Panel, PanelKind};
use crate::workspace::stores::AppStores;

/// Non-durable fallback panel — the "untitled" tab every empty center
/// pane starts with.
pub struct ScratchPanel;

impl ScratchPanel {
    /// Create a fresh scratch panel.
    pub fn new() -> Self {
        Self
    }

    /// Rehydrate from a snapshot (ignores the value — scratch has no
    /// state).
    pub fn from_snapshot(_snapshot: serde_json::Value) -> Self {
        Self
    }
}

impl Default for ScratchPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for ScratchPanel {
    fn title(&self) -> String {
        String::from("untitled")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Scratch
    }

    fn view<'a>(&'a self, _stores: &'a AppStores) -> Element<'a, ErasedMessage> {
        container(text("Scratch"))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn update(&mut self, message: ErasedMessage, _stores: &mut AppStores) {
        debug_assert!(
            message.downcast_ref::<()>().is_some(),
            "ScratchPanel received unexpected message"
        );
    }

    fn snapshot(&self, _stores: &AppStores) -> serde_json::Value {
        serde_json::json!({})
    }
}
