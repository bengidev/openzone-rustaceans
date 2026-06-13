#![allow(dead_code)]

//! Panel location addressing.
//!
//! A [`PanelLocation`] uniquely names where a panel lives so focus,
//! routing, and (later) persistence and drag targets can address any
//! panel unambiguously. Center panes live in the pane grid; docks live
//! on the three collapsible edges and reuse the same pane-state and
//! panel abstractions as the center.

use iced::widget::pane_grid;
use serde::{Deserialize, Serialize};

/// One of the three collapsible edge docks.
///
/// Docks frame the center workspace: `Left` and `Right` flank it as
/// vertical rails, `Bottom` spans beneath it. The top edge is reserved
/// for application chrome (title bar), so there is no top dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockSide {
    Left,
    Right,
    Bottom,
}

impl DockSide {
    /// All dock sides in stable rendering order.
    pub const ALL: [DockSide; 3] = [DockSide::Left, DockSide::Right, DockSide::Bottom];

    /// Human-readable label for chrome and the status bar.
    pub fn label(self) -> &'static str {
        match self {
            DockSide::Left => "Left",
            DockSide::Right => "Right",
            DockSide::Bottom => "Bottom",
        }
    }
}

/// Where a panel lives in the workspace.
///
/// Focus, commands, and routing all address panels through this enum so
/// no call site needs to special-case "is this a center pane or a
/// dock?" beyond the one match each concern already performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelLocation {
    /// A pane in the center pane grid.
    Center(pane_grid::Pane),
    /// The tab stack hosted by an edge dock.
    Dock(DockSide),
}

impl PanelLocation {
    /// The dock side this location addresses, if it is a dock.
    pub fn dock_side(self) -> Option<DockSide> {
        match self {
            PanelLocation::Dock(side) => Some(side),
            PanelLocation::Center(_) => None,
        }
    }

    /// Whether this location addresses a center pane.
    pub fn is_center(self) -> bool {
        matches!(self, PanelLocation::Center(_))
    }
}
