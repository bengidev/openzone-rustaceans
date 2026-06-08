#![allow(dead_code)]

//! Text dummy panel — text-input panel.
//!
//! Proves a panel can host an interactive input widget and demonstrates
//! panel-first key capture: the input swallows plain typing while
//! workspace shortcuts (chords with the command accelerator) bubble up
//! to the workspace keymap.

use iced::widget::{column, text, text_input};
use iced::{Element, Length};

use crate::workspace::command::{Chord, KeyRef};
use crate::workspace::panel::{ErasedMessage, Panel, PanelKind, downcast, erase};

/// Concrete message for the text panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextMessage {
    Changed(String),
}

/// A panel holding an editable string buffer.
pub struct TextPanel {
    content: String,
}

impl TextPanel {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    /// Rehydrate from a snapshot handle; falls back to empty.
    pub fn from_snapshot(snapshot: serde_json::Value) -> Self {
        let content = snapshot
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        Self { content }
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Default for TextPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for TextPanel {
    fn title(&self) -> String {
        String::from("Text")
    }

    fn kind(&self) -> PanelKind {
        PanelKind::Text
    }

    fn view(&self) -> Element<'_, ErasedMessage> {
        let label = text("Type something:").size(16);
        let input = text_input("…", &self.content)
            .on_input(|value| erase(TextMessage::Changed(value)))
            .padding(8);

        column![label, input]
            .spacing(12)
            .width(Length::Fixed(320.0))
            .into()
    }

    fn update(&mut self, message: ErasedMessage) {
        match downcast::<TextMessage>(message) {
            Some(message) => match &*message {
                TextMessage::Changed(value) => self.content = value.clone(),
            },
            None => debug_assert!(false, "TextPanel received a foreign message"),
        }
    }

    /// The text input swallows plain typing and backspace so those keys
    /// edit the buffer instead of triggering workspace commands. Chords
    /// carrying the command accelerator (e.g. `Cmd+W`) are *not*
    /// captured — they bubble up to the workspace keymap. This is the
    /// Text dummy demonstrating typing versus global shortcuts.
    fn captures_chord(&self, chord: Chord) -> bool {
        if chord.mods.command {
            return false;
        }
        matches!(chord.key, KeyRef::Char(_) | KeyRef::Backspace)
    }

    fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({ "content": self.content })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::command::Mods;

    #[test]
    fn change_updates_content() {
        let mut panel = TextPanel::new();
        panel.update(erase(TextMessage::Changed(String::from("hello"))));
        assert_eq!(panel.content(), "hello");
    }

    #[test]
    fn snapshot_round_trips_through_constructor() {
        let mut panel = TextPanel::new();
        panel.update(erase(TextMessage::Changed(String::from("draft"))));
        let restored = TextPanel::from_snapshot(panel.snapshot());
        assert_eq!(restored.content(), "draft");
    }

    #[test]
    fn captures_plain_typing() {
        let panel = TextPanel::new();
        assert!(panel.captures_chord(Chord::ch('a', Mods::NONE)));
        assert!(panel.captures_chord(Chord::new(KeyRef::Backspace, Mods::NONE)));
    }

    #[test]
    fn does_not_capture_command_shortcuts() {
        let panel = TextPanel::new();
        // Cmd+W must reach the workspace keymap, not the text buffer.
        assert!(!panel.captures_chord(Chord::ch('w', Mods::CMD)));
    }
}
