#![allow(dead_code)]

//! Command palette state, command identity, and filtering.
//!
//! The workspace owns [`PaletteState`] (open/query/selection UI state).
//! The app root seeds available commands via [`CommandItem`]s at construction.
//! Filtering is case-insensitive substring matching. Non-empty queries are
//! capped at [`MAX_RESULTS`]; an empty query shows the full catalog.

use std::borrow::Cow;

use crate::workspace::workspace_location::DockSide;

/// Typed command identifier for palette commands.
///
/// Workspace commands and app commands share this enum so the palette
/// can surface both without knowing which layer executes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    OpenDock(DockSide),
    CollapseDock(DockSide),
    HideDock(DockSide),
    SplitFocused,
    CloseActiveTab,
    ToggleTheme,
    NewWindow,
}

/// A palette result: human-readable label paired with a [`CommandId`].
#[derive(Debug, Clone)]
pub struct CommandItem {
    pub label: Cow<'static, str>,
    pub id: CommandId,
}

impl CommandItem {
    pub fn new(label: impl Into<Cow<'static, str>>, id: CommandId) -> Self {
        Self {
            label: label.into(),
            id,
        }
    }
}

/// Maximum results shown in the palette.
pub const MAX_RESULTS: usize = 10;

/// The palette's UI state — owned by the workspace.
#[derive(Debug, Clone)]
pub struct PaletteState {
    /// Whether the palette overlay is open.
    pub open: bool,
    /// Current search query string.
    pub query: String,
    /// Filtered results (already capped at [`MAX_RESULTS`]).
    pub filtered: Vec<CommandItem>,
    /// Index of the highlighted result (0-based; 0 when empty).
    pub selected: usize,
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
        }
    }

    /// Re-filter from `all` using case-insensitive substring matching.
    /// Non-empty queries are capped at [`MAX_RESULTS`]; an empty query shows
    /// the full catalog. Resets selection to 0.
    pub fn filter(&mut self, all: &[CommandItem]) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered = all.to_vec();
        } else {
            self.filtered = all
                .iter()
                .filter(|item| item.label.to_lowercase().contains(&q))
                .take(MAX_RESULTS)
                .cloned()
                .collect();
        }
        self.selected = 0;
    }

    /// Move selection up (wraps).
    pub fn select_prev(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.filtered.len() - 1
        } else {
            self.selected - 1
        };
    }

    /// Move selection down (wraps).
    pub fn select_next(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = if self.selected + 1 >= self.filtered.len() {
            0
        } else {
            self.selected + 1
        };
    }

    /// Set selection to an absolute index, clamped to bounds.
    pub fn select_at(&mut self, index: usize) {
        if !self.filtered.is_empty() {
            self.selected = index.min(self.filtered.len() - 1);
        }
    }

    /// Dismiss the palette and return the selected command, if any.
    pub fn take_selection(&mut self) -> Option<CommandId> {
        let cmd = self.filtered.get(self.selected).map(|item| item.id);
        self.open = false;
        self.query.clear();
        self.filtered.clear();
        self.selected = 0;
        cmd
    }

    /// Dismiss without selecting.
    pub fn dismiss(&mut self) {
        self.open = false;
        self.query.clear();
        self.filtered.clear();
        self.selected = 0;
    }
}

impl Default for PaletteState {
    fn default() -> Self {
        Self::new()
    }
}

/// Seed the palette's available commands.
///
/// Called by the app root at workspace construction. Adding a new
/// workspace or app command means adding its [`CommandItem`] here and
/// wiring its dispatch in [`Workspace::dispatch_palette_command`].
pub fn default_command_items() -> Vec<CommandItem> {
    vec![
        CommandItem::new("Open Activity Dock", CommandId::OpenDock(DockSide::Left)),
        CommandItem::new(
            "Open Conversation Dock",
            CommandId::OpenDock(DockSide::Right),
        ),
        CommandItem::new("Open Output Dock", CommandId::OpenDock(DockSide::Bottom)),
        CommandItem::new(
            "Collapse Activity Dock",
            CommandId::CollapseDock(DockSide::Left),
        ),
        CommandItem::new(
            "Collapse Conversation Dock",
            CommandId::CollapseDock(DockSide::Right),
        ),
        CommandItem::new(
            "Collapse Output Dock",
            CommandId::CollapseDock(DockSide::Bottom),
        ),
        CommandItem::new("Hide Activity Dock", CommandId::HideDock(DockSide::Left)),
        CommandItem::new(
            "Hide Conversation Dock",
            CommandId::HideDock(DockSide::Right),
        ),
        CommandItem::new("Hide Output Dock", CommandId::HideDock(DockSide::Bottom)),
        CommandItem::new("Split Pane", CommandId::SplitFocused),
        CommandItem::new("Close Active Tab", CommandId::CloseActiveTab),
        CommandItem::new("Toggle Theme", CommandId::ToggleTheme),
        CommandItem::new("New Window", CommandId::NewWindow),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<CommandItem> {
        (0..15)
            .map(|i| CommandItem::new(format!("Command {i}"), CommandId::NewWindow))
            .collect()
    }

    #[test]
    fn empty_query_shows_full_catalog() {
        let all = default_command_items();
        let mut palette = PaletteState::new();
        palette.filter(&all);
        assert_eq!(palette.filtered.len(), all.len());
        assert!(
            palette
                .filtered
                .iter()
                .any(|item| item.id == CommandId::NewWindow)
        );
    }

    #[test]
    fn non_empty_query_is_capped_at_max_results() {
        let all = sample_items();
        let mut palette = PaletteState::new();
        palette.query = "command".into();
        palette.filter(&all);
        assert_eq!(palette.filtered.len(), MAX_RESULTS);
    }

    #[test]
    fn filter_is_case_insensitive() {
        let all = default_command_items();
        let mut palette = PaletteState::new();
        palette.query = "NEW WINDOW".into();
        palette.filter(&all);
        assert_eq!(palette.filtered.len(), 1);
        assert_eq!(palette.filtered[0].id, CommandId::NewWindow);
    }

    #[test]
    fn selection_wraps() {
        let all = default_command_items();
        let mut palette = PaletteState::new();
        palette.filter(&all);
        let last = palette.filtered.len() - 1;
        palette.selected = 0;
        palette.select_prev();
        assert_eq!(palette.selected, last);
        palette.select_next();
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn take_selection_clears_state() {
        let all = default_command_items();
        let mut palette = PaletteState::new();
        palette.open = true;
        palette.query = "new".into();
        palette.filter(&all);
        let cmd = palette.take_selection();
        assert_eq!(cmd, Some(CommandId::NewWindow));
        assert!(!palette.open);
        assert!(palette.query.is_empty());
        assert!(palette.filtered.is_empty());
    }

    #[test]
    fn dismiss_clears_without_returning_command() {
        let all = default_command_items();
        let mut palette = PaletteState::new();
        palette.open = true;
        palette.filter(&all);
        palette.dismiss();
        assert!(!palette.open);
        assert!(palette.filtered.is_empty());
    }
}
