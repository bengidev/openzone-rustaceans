#![allow(dead_code)]

//! Scratch panel — the non-durable fallback every workspace starts with.
//!
//! A `ScratchPanel` is the empty-canvas placeholder shown when no
//! domain panel has been opened yet. It has no state of its own, no
//! persistence payload, and no meaningful messages. The composition
//! root wires a factory for it into the workspace so that every center
//! pane is guaranteed to have at least one tab on startup.

use iced::widget::{container, text_editor};
use iced::{Element, Length};

use crate::workspace::{AppStores, ErasedMessage, Panel, PanelKind};

#[derive(Clone, Debug)]
pub enum ScratchMessage {
    Edit(text_editor::Action),
}

/// Non-durable fallback panel — the "untitled" tab every empty center
/// pane starts with.
pub struct ScratchPanel {
    content: text_editor::Content,
}

impl ScratchPanel {
    /// Create a fresh scratch panel.
    pub fn new() -> Self {
        Self {
            content: text_editor::Content::new(),
        }
    }

    /// Rehydrate from a snapshot (ignores the value — scratch has no
    /// state).
    pub fn from_snapshot(_snapshot: serde_json::Value) -> Self {
        Self::new()
    }
}

impl Default for ScratchPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for ScratchPanel {
    fn title(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed("untitled")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Scratch
    }

    fn view<'a>(&'a self, _stores: &'a AppStores) -> Element<'a, ErasedMessage> {
        let input = text_editor(&self.content)
            .on_action(|action| crate::workspace::erase(ScratchMessage::Edit(action)));

        container(input)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn update(&mut self, message: ErasedMessage, _stores: &mut AppStores) {
        if let Some(msg) = crate::workspace::downcast::<ScratchMessage>(message) {
            match &*msg {
                ScratchMessage::Edit(action) => {
                    self.content.perform(action.clone());
                }
            }
        }
    }

    fn snapshot(&self, _stores: &AppStores) -> Option<serde_json::Value> {
        None
    }

    fn is_dirty(&self) -> bool {
        !self.content.text().is_empty()
    }

    fn close_request(&self) -> crate::workspace::workspace_panel::CloseRequest {
        if self.is_dirty() {
            crate::workspace::workspace_panel::CloseRequest::Confirm {
                message: std::borrow::Cow::Borrowed("Discard changes to untitled?"),
            }
        } else {
            crate::workspace::workspace_panel::CloseRequest::Allowed
        }
    }

    fn status_contribution(&self, sink: &mut crate::workspace::workspace_panel::StatusSink) {
        let pos = self.content.cursor().position;
        let line = pos.line + 1;
        let col = pos.column + 1;
        sink.push(format!("Ln {}, Col {}", line, col));
        sink.push("Plain Text");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::CloseRequest;
    use crate::workspace::StatusSink;

    #[test]
    fn test_scratch_panel_clean_state() {
        let panel = ScratchPanel::new();
        assert_eq!(panel.title(), "untitled");
        assert!(!panel.is_dirty());
        assert_eq!(panel.close_request(), CloseRequest::Allowed);
    }

    #[test]
    fn test_scratch_panel_status_contribution() {
        let panel = ScratchPanel::new();
        let mut segments = Vec::new();
        let mut sink = StatusSink::new(&mut segments);
        panel.status_contribution(&mut sink);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], "Ln 1, Col 1");
        assert_eq!(segments[1], "Plain Text");
    }

    #[test]
    fn test_scratch_panel_dirty_state_and_close_request() {
        let mut panel = ScratchPanel::new();
        let mut stores = AppStores::new();
        let action = text_editor::Action::Edit(text_editor::Edit::Insert('a'));
        panel.update(
            crate::workspace::erase(ScratchMessage::Edit(action)),
            &mut stores,
        );

        assert!(panel.is_dirty());
        match panel.close_request() {
            CloseRequest::Confirm { message } => {
                assert_eq!(message, "Discard changes to untitled?");
            }
            CloseRequest::Allowed => {
                panic!("Expected close confirmation for dirty panel");
            }
        }
    }
}
