#![allow(dead_code)]

//! The single workspace message type.
//!
//! Heterogeneous panel messages are erased and tagged with their origin
//! ([`PanelLocation`] + tab index) so the workspace can route them back
//! to the exact panel that produced them through one `update` path.

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
        }
    }
}
