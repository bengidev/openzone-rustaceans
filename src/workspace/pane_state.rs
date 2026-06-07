#![allow(dead_code)]

//! Per-pane content: a tab stack.
//!
//! `pane_grid` manages splits *between* panes; `PaneState` manages tabs
//! *within* a pane. Keeping the two separate is deliberate (Q4/Q11): a
//! within-pane tab move and a between-pane split are distinct concerns.
//! Docks reuse this same `PaneState` in a later slice.

use crate::workspace::panel::Panel;

/// A stack of tabbed panels with one active tab.
pub struct PaneState {
    pub tabs: Vec<Box<dyn Panel>>,
    pub active: usize,
}

impl PaneState {
    /// Build a pane from its initial tabs; the first tab is active.
    pub fn new(tabs: Vec<Box<dyn Panel>>) -> Self {
        Self { tabs, active: 0 }
    }

    /// The currently active panel, if any.
    pub fn active_panel(&self) -> Option<&dyn Panel> {
        self.tabs.get(self.active).map(|panel| panel.as_ref())
    }

    /// Mutable access to the currently active panel, if any.
    pub fn active_panel_mut(&mut self) -> Option<&mut (dyn Panel + 'static)> {
        self.tabs.get_mut(self.active).map(|panel| panel.as_mut())
    }

    /// Select a tab by index. Out-of-range selections are ignored so a
    /// stale message can never point `active` at a missing tab.
    pub fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }
}
