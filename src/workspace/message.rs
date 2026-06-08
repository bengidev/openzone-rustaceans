#![allow(dead_code)]

//! The single workspace message type.
//!
//! Heterogeneous panel messages are erased and tagged with their origin
//! ([`PanelLocation`] + tab index) so the workspace can route them back
//! to the exact panel that produced them through one `update` path.
//! Raw key chords and resolved commands flow through the same enum so
//! key routing and command dispatch share the single reducer.

use crate::workspace::command::{Chord, Command};
use crate::workspace::location::PanelLocation;
use crate::workspace::panel::ErasedMessage;
use iced::widget::pane_grid;

/// Everything the workspace reducer can react to.
#[derive(Clone)]
pub enum WorkspaceMessage {
    /// An erased message produced by a panel's own view or subscription,
    /// tagged with the panel's location and tab index for routing.
    Panel {
        location: PanelLocation,
        tab: usize,
        message: ErasedMessage,
    },
    /// The user selected a tab within a pane's tab strip.
    TabSelected { location: PanelLocation, tab: usize },
    /// A pane in the center grid was clicked — focus follows the click.
    PaneClicked(pane_grid::Pane),
    /// A dock's tab strip or rail was clicked — focus moves to the dock.
    DockFocused(PanelLocation),
    /// A raw key chord from the keyboard subscription. The reducer
    /// applies panel-first capture: the focused panel may swallow it,
    /// otherwise it resolves against the workspace keymap.
    Key(Chord),
    /// A resolved workspace command (from a keymap hit, a menu, or a
    /// future command palette). Dispatched straight to `apply_command`.
    Command(Command),
    /// Flip the workspace between light and dark mode. Repaints this
    /// window via the daemon's per-window `theme` callback.
    ToggleTheme,
    /// A drag-and-drop interaction on the center pane grid. On
    /// `Dropped` the reducer reorders the panes; other phases are no-ops.
    PaneDragged(pane_grid::DragEvent),
}

impl std::fmt::Debug for WorkspaceMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceMessage::Panel { location, tab, .. } => f
                .debug_struct("Panel")
                .field("location", location)
                .field("tab", tab)
                .field("message", &"<erased>")
                .finish(),
            WorkspaceMessage::TabSelected { location, tab } => f
                .debug_struct("TabSelected")
                .field("location", location)
                .field("tab", tab)
                .finish(),
            WorkspaceMessage::PaneClicked(pane) => {
                f.debug_tuple("PaneClicked").field(pane).finish()
            }
            WorkspaceMessage::DockFocused(location) => {
                f.debug_tuple("DockFocused").field(location).finish()
            }
            WorkspaceMessage::Key(chord) => f.debug_tuple("Key").field(chord).finish(),
            WorkspaceMessage::Command(command) => f.debug_tuple("Command").field(command).finish(),
            WorkspaceMessage::ToggleTheme => f.debug_tuple("ToggleTheme").finish(),
            WorkspaceMessage::PaneDragged(event) => {
                f.debug_tuple("PaneDragged").field(event).finish()
            }
        }
    }
}
