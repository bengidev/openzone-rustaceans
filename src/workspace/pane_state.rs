#![allow(dead_code)]

//! Per-pane content: a tab stack.
//!
//! `pane_grid` manages splits *between* panes; `PaneState` manages tabs
//! *within* a pane. Keeping the two separate is deliberate (Q4/Q11): a
//! within-pane tab move and a between-pane split are distinct concerns.
//! Docks reuse this same `PaneState`.

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

    /// Build an empty pane (no tabs). Used for docks that are seeded
    /// without content; an empty center pane is collapsed by the split
    /// tree rather than displayed.
    pub fn empty() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
        }
    }

    /// Number of tabs in the stack.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Whether the stack has no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
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

    /// Close the active tab and return whether the stack is now empty.
    ///
    /// After removal the active index is clamped to the last surviving
    /// tab so it never dangles past the end. Closing the final tab
    /// leaves the stack empty (`true`), which the caller uses to collapse
    /// the owning pane or dock. Closing when already empty is a no-op.
    pub fn close_active(&mut self) -> bool {
        if self.tabs.is_empty() {
            return true;
        }
        if self.active < self.tabs.len() {
            self.tabs.remove(self.active);
        }
        if self.active >= self.tabs.len() && !self.tabs.is_empty() {
            self.active = self.tabs.len() - 1;
        }
        self.tabs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};

    fn three_tabs() -> PaneState {
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new()),
            Box::new(TextPanel::new()),
            Box::new(ClockPanel::new()),
        ];
        PaneState::new(tabs)
    }

    #[test]
    fn close_active_removes_the_selected_tab() {
        let mut pane = three_tabs();
        pane.select(1);
        let empty = pane.close_active();
        assert!(!empty);
        assert_eq!(pane.len(), 2);
        // Counter (0) and Clock (formerly 2, now 1) remain.
        assert_eq!(pane.tabs[0].title(), "Counter");
        assert_eq!(pane.tabs[1].title(), "Clock");
    }

    #[test]
    fn closing_last_index_clamps_active() {
        let mut pane = three_tabs();
        pane.select(2);
        pane.close_active();
        // Active was the final tab; it clamps to the new last tab.
        assert_eq!(pane.active, 1);
    }

    #[test]
    fn closing_every_tab_reports_empty() {
        let mut pane = three_tabs();
        assert!(!pane.close_active());
        assert!(!pane.close_active());
        assert!(pane.close_active());
        assert!(pane.is_empty());
    }

    #[test]
    fn close_active_on_empty_is_noop_and_empty() {
        let mut pane = PaneState::empty();
        assert!(pane.close_active());
        assert!(pane.is_empty());
    }
}
