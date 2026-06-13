#![allow(dead_code)]

//! Edge docks.
//!
//! A [`Dock`] wraps a [`PaneState`] (the same tab-stack model the center
//! panes use) plus an open/collapsed flag. [`Docks`] owns the three edge
//! docks and answers the focus/routing/render questions the workspace
//! asks. A collapsed dock keeps its tabs — collapsing only hides the
//! body and shows a minimal rail; reopening restores the prior tabs and
//! active selection.

use crate::workspace::workspace_location::DockSide;
use crate::workspace::workspace_pane_state::PaneState;

/// One edge dock: a tab stack plus its open/collapsed state.
pub struct Dock {
    /// The dock's tabbed content, sharing the center pane model.
    pub tabs: PaneState,
    /// Whether the dock body is shown. Collapsed docks render as a rail
    /// but retain their tabs.
    pub open: bool,
}

impl Dock {
    /// Build a dock from its tabs. Docks start collapsed so the shell
    /// opens to the center workspace; the user reveals docks on demand.
    pub fn new(tabs: PaneState) -> Self {
        Self { tabs, open: false }
    }

    /// Build a dock that starts open.
    pub fn open_with(tabs: PaneState) -> Self {
        Self { tabs, open: true }
    }

    /// Flip open/collapsed and report the new state.
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
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
            bottom: Dock::new(bottom),
        }
    }

    /// Build a dock set with every side empty (no tabs). Useful for
    /// tests and for a shell booted without seeded dock content.
    pub fn empty() -> Self {
        Self {
            left: Dock::new(PaneState::empty()),
            right: Dock::new(PaneState::empty()),
            bottom: Dock::new(PaneState::empty()),
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

    /// Toggle a dock's open state, returning the new state. Toggling an
    /// empty dock open is allowed but the view renders nothing until it
    /// has tabs; this keeps the command total over all sides.
    pub fn toggle(&mut self, side: DockSide) -> bool {
        self.get_mut(side).toggle()
    }

    /// Whether a dock is currently open *and* has content to show.
    pub fn is_visible(&self, side: DockSide) -> bool {
        let dock = self.get(side);
        dock.open && !dock.is_empty()
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
    fn docks_start_collapsed() {
        let docks = Docks::new(one_tab(), one_tab(), one_tab());
        assert!(!docks.left.open);
        assert!(!docks.right.open);
        assert!(!docks.bottom.open);
    }

    #[test]
    fn toggle_opens_then_collapses() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        assert!(docks.toggle(DockSide::Left));
        assert!(docks.left.open);
        assert!(!docks.toggle(DockSide::Left));
        assert!(!docks.left.open);
    }

    #[test]
    fn collapsing_retains_tabs() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        docks.toggle(DockSide::Right);
        docks.toggle(DockSide::Right);
        assert_eq!(docks.right.tabs.len(), 1);
    }

    #[test]
    fn empty_dock_is_not_visible_even_when_open() {
        let mut docks = Docks::empty();
        docks.toggle(DockSide::Bottom);
        assert!(docks.bottom.open);
        assert!(!docks.is_visible(DockSide::Bottom));
    }

    #[test]
    fn open_dock_with_tabs_is_visible() {
        let mut docks = Docks::new(one_tab(), one_tab(), one_tab());
        docks.toggle(DockSide::Left);
        assert!(docks.is_visible(DockSide::Left));
    }
}
