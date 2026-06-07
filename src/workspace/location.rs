#![allow(dead_code)]

//! Panel location addressing.
//!
//! A [`PanelLocation`] uniquely names where a panel lives so focus,
//! routing, and (later) persistence and drag targets can address any
//! panel unambiguously. Step 1 of the build spine only has center
//! panes; dock sides arrive in a later slice without changing call
//! sites that already match on this enum.

use iced::widget::pane_grid;

/// Where a panel lives in the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelLocation {
    /// A pane in the center pane grid.
    Center(pane_grid::Pane),
    // Dock(DockSide) — added in build-spine step 2 (docks + commands).
}
