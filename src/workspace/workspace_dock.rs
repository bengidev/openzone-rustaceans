#![allow(dead_code)]

//! Edge docks.
//!
//! A [`Dock`] wraps a [`PaneState`] (the same tab-stack model the center
//! panes use) plus a tri-state visibility flag ([`DockVisibility`]).
//! [`Docks`] owns the three edge docks and answers the focus/routing/
//! render questions the workspace asks.
//!
//! Visibility states:
//! - **Hidden** — no rail, no body; consumes no layout space.
//! - **Collapsed** — rail only; keeps tabs.
//! - **Open** — body shown at remembered extent; keeps tabs.
//!
//! Closing an open dock hides it (→ Hidden), not collapses it.
//! Collapsing is a distinct user action (→ Collapsed).

use serde::{Deserialize, Serialize};

use crate::workspace::workspace_location::DockSide;
use crate::workspace::workspace_pane_state::PaneState;

/// Tri-state dock visibility.
///
/// - `Hidden`    — no rail, no body; consumes no layout space.
/// - `Collapsed` — rail only; retains tabs.
/// - `Open`      — body shown at remembered extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DockVisibility {
    Hidden,
    Collapsed,
    Open,
}

/// Default open width for left/right docks. Kept in sync with [`SIDE_DOCK_WIDTH`].
const DEFAULT_SIDE_EXTENT: f32 = 280.0;
/// Default open height for the bottom dock. Kept in sync with [`BOTTOM_DOCK_HEIGHT`].
const DEFAULT_BOTTOM_EXTENT: f32 = 200.0;

/// One edge dock: a tab stack plus its visibility state.
pub struct Dock {
    /// The dock's tabbed content, sharing the center pane model.
    pub tabs: PaneState,
    /// Whether the dock body, rail, or nothing is shown.
    pub visibility: DockVisibility,
    /// Remembered open width (side docks) or height (bottom dock).
    pub extent: f32,
}

impl Dock {
    /// Build a dock from its tabs. Docks start hidden so the shell
    /// opens to the center workspace; the user reveals docks on demand.
    pub fn new(tabs: PaneState) -> Self {
        Self::with_extent(tabs, DockVisibility::Hidden, DEFAULT_SIDE_EXTENT)
    }

    /// Build a dock that starts open.
    pub fn open_with(tabs: PaneState) -> Self {
        Self::with_extent(tabs, DockVisibility::Open, DEFAULT_SIDE_EXTENT)
    }

    /// Build a dock with an explicit remembered open extent.
    pub fn with_extent(tabs: PaneState, visibility: DockVisibility, extent: f32) -> Self {
        Self {
            tabs,
            visibility,
            extent,
        }
    }

    /// Set visibility to Open.
    pub fn open(&mut self) {
        self.visibility = DockVisibility::Open;
    }

    /// Set visibility to Collapsed.
    pub fn collapse(&mut self) {
        self.visibility = DockVisibility::Collapsed;
    }

    /// Set visibility to Hidden.
    pub fn hide(&mut self) {
        self.visibility = DockVisibility::Hidden;
    }

    /// Whether the dock body is shown.
    pub fn is_open(&self) -> bool {
        self.visibility == DockVisibility::Open
    }

    /// Whether the dock shows a rail only.
    pub fn is_collapsed(&self) -> bool {
        self.visibility == DockVisibility::Collapsed
    }

    /// Whether the dock is hidden (no rail, no body).
    pub fn is_hidden(&self) -> bool {
        self.visibility == DockVisibility::Hidden
    }

    /// Whether the dock shows anything at all (rail or body).
    pub fn is_visible(&self) -> bool {
        self.visibility != DockVisibility::Hidden
    }

    /// Whether the dock has no tabs left.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

/// The three edge docks of the workspace.
///
/// A dock side is always present in the layout engine even with no tabs;
/// an empty dock simply renders nothing (no rail, no body). This keeps
/// [`PanelLocation::Dock`] total — addressing any side is always valid —
/// while the view decides what, if anything, to draw.
pub struct Docks {
    pub left: Dock,
    pub right: Dock,
    pub bottom: Dock,
}

impl Docks {
    /// Build the dock set from per-side tab stacks.
    pub fn new(left: PaneState, right: PaneState, bottom: PaneState) -> Self {
        Self {
            left: Dock::new(left),
            right: Dock::new(right),
            bottom: Dock::with_extent(bottom, DockVisibility::Hidden, DEFAULT_BOTTOM_EXTENT),
        }
    }

    /// Build a dock set with every side empty (no tabs). Useful for
    /// tests and for a shell booted without seeded dock content.
    pub fn empty() -> Self {
        Self {
            left: Dock::new(PaneState::empty()),
            right: Dock::new(PaneState::empty()),
            bottom: Dock::with_extent(
                PaneState::empty(),
                DockVisibility::Hidden,
                DEFAULT_BOTTOM_EXTENT,
            ),
        }
    }

    /// Shared reference to a dock by side.
    pub fn get(&self, side: DockSide) -> &Dock {
        match side {
            DockSide::Left => &self.left,
            DockSide::Right => &self.right,
            DockSide::Bottom => &self.bottom,
        }
    }

    /// Mutable reference to a dock by side.
    pub fn get_mut(&mut self, side: DockSide) -> &mut Dock {
        match side {
            DockSide::Left => &mut self.left,
            DockSide::Right => &mut self.right,
            DockSide::Bottom => &mut self.bottom,
        }
    }

    /// Set a dock's visibility.
    pub fn set_visibility(&mut self, side: DockSide, visibility: DockVisibility) {
        self.get_mut(side).visibility = visibility;
    }

    /// Whether a dock is currently visible *and* has content to show.
    /// A hidden dock or an empty dock is not visible.
    pub fn is_visible(&self, side: DockSide) -> bool {
        let dock = self.get(side);
        dock.visibility != DockVisibility::Hidden && !dock.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::dummies::TextPanel;
    use crate::workspace::workspace_panel::Panel;

    fn one_tab() -> PaneState {
        let tabs: Vec<Box<dyn Panel>> = vec![Box::new(TextPanel::new())];
        PaneState::new(tabs)
    }

    #[test]
    fn docks_start_hidden() {
        let docks = Docks::new(one_tab(), one_tab(), one_tab());
        assert!(!docks.left.is_open());
        assert!(docks.left.is_hidden());
        assert!(!docks.right.is_open());
        assert!(docks.right.is_hidden());
        assert!(!docks.bottom.is_open());
        assert!(docks.bottom.is_hidden());
    }

    #[test]
    fn set_visibility_transitions() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        docks.set_visibility(DockSide::Left, DockVisibility::Open);
        assert!(docks.left.is_open());
        docks.set_visibility(DockSide::Left, DockVisibility::Collapsed);
        assert!(docks.left.is_collapsed());
        docks.set_visibility(DockSide::Left, DockVisibility::Hidden);
        assert!(docks.left.is_hidden());
    }

    #[test]
    fn collapsing_retains_tabs() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        docks.set_visibility(DockSide::Right, DockVisibility::Open);
        docks.set_visibility(DockSide::Right, DockVisibility::Collapsed);
        docks.set_visibility(DockSide::Right, DockVisibility::Hidden);
        assert_eq!(docks.right.tabs.len(), 1);
    }

    #[test]
    fn empty_dock_is_not_visible_even_when_open() {
        let mut docks = Docks::empty();
        docks.set_visibility(DockSide::Bottom, DockVisibility::Open);
        assert!(docks.bottom.is_open());
        assert!(!docks.is_visible(DockSide::Bottom));
    }

    #[test]
    fn open_dock_with_tabs_is_visible() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        docks.set_visibility(DockSide::Left, DockVisibility::Open);
        assert!(docks.is_visible(DockSide::Left));
    }
}
