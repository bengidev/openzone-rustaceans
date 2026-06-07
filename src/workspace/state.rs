#![allow(dead_code)]

//! Workspace state and reducer.
//!
//! The workspace owns the center `pane_grid`, each pane's tab stack
//! ([`PaneState`]), and a single centrally-owned focus. All panel
//! messages funnel through [`Workspace::update`] (single-writer). The
//! view layer reads this state by `&self`; only the reducer mutates.

use iced::widget::pane_grid;
use iced::Subscription;

use crate::shared::design::{OpenZoneTheme, ThemeMode};
use crate::workspace::location::PanelLocation;
use crate::workspace::message::WorkspaceMessage;
use crate::workspace::pane_state::PaneState;
use crate::workspace::panel::ErasedMessage;

/// The single-window workspace shell state.
pub struct Workspace {
    /// Iced's recursive split tree of panes (splits *between* panes).
    pub panes: pane_grid::State<PaneState>,
    /// The centrally-owned focused location. Chrome reads this; it stays
    /// in sync with the pane grid's own click focus.
    pub focused: PanelLocation,
    /// Resolved design theme for token lookups in the view.
    pub theme: OpenZoneTheme,
    pub theme_mode: ThemeMode,
}

impl Workspace {
    /// Build a single-pane workspace hosting `tabs`. The lone pane is
    /// focused on launch.
    pub fn single_pane(tabs: PaneState, theme_mode: ThemeMode) -> Self {
        let (panes, first) = pane_grid::State::new(tabs);
        Self {
            panes,
            focused: PanelLocation::Center(first),
            theme: OpenZoneTheme::from_mode(theme_mode),
            theme_mode,
        }
    }

    /// The pane backing a center location, if it still exists.
    fn pane_state(&self, location: PanelLocation) -> Option<&PaneState> {
        match location {
            PanelLocation::Center(pane) => self.panes.get(pane),
        }
    }

    fn pane_state_mut(&mut self, location: PanelLocation) -> Option<&mut PaneState> {
        match location {
            PanelLocation::Center(pane) => self.panes.get_mut(pane),
        }
    }

    /// Whether `location` is the focused location.
    pub fn is_focused(&self, location: PanelLocation) -> bool {
        self.focused == location
    }

    /// Fold a workspace message into state. Single mutation path.
    pub fn update(&mut self, message: WorkspaceMessage) {
        match message {
            WorkspaceMessage::PaneClicked(pane) => {
                // Focus follows the click; keep central focus in sync
                // with the pane grid.
                self.focused = PanelLocation::Center(pane);
            }
            WorkspaceMessage::TabSelected { location, tab } => {
                // Selecting a tab also focuses its pane.
                self.focused = location;
                if let Some(pane_state) = self.pane_state_mut(location) {
                    pane_state.select(tab);
                }
            }
            WorkspaceMessage::Panel {
                location,
                tab,
                message,
            } => {
                self.route_to_panel(location, tab, message);
            }
        }
    }

    /// Deliver an erased message to the exact panel that produced it.
    /// A panel that has since been removed (stale tab index) is a no-op.
    fn route_to_panel(&mut self, location: PanelLocation, tab: usize, message: ErasedMessage) {
        if let Some(pane_state) = self.pane_state_mut(location) {
            if let Some(panel) = pane_state.tabs.get_mut(tab) {
                panel.update(message);
            }
        }
    }

    /// Batch every live panel's subscription into one workspace stream.
    /// Iced starts/stops each as panels appear and drop. Subscription
    /// identity is the panel's own concern; the workspace wraps each
    /// stream so the resulting message carries the panel's location and
    /// tab index for routing back through `update`.
    pub fn subscription(&self) -> Subscription<WorkspaceMessage> {
        let mut streams = Vec::new();
        for (pane, pane_state) in self.panes.iter() {
            let location = PanelLocation::Center(*pane);
            for (tab, panel) in pane_state.tabs.iter().enumerate() {
                // `Subscription::map` requires a non-capturing closure in
                // Iced 0.14, so the routing metadata is threaded through
                // `with` (zipped onto each message) rather than captured.
                let tagged = panel
                    .subscription()
                    .with((location, tab))
                    .map(|((location, tab), message)| WorkspaceMessage::Panel {
                        location,
                        tab,
                        message,
                    });
                streams.push(tagged);
            }
        }
        Subscription::batch(streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::dummies::clock::ClockMessage;
    use crate::features::dummies::{CounterPanel, TextPanel};
    use crate::workspace::panel::{erase, Panel};

    fn three_tab_workspace() -> Workspace {
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new()),
            Box::new(TextPanel::new()),
        ];
        Workspace::single_pane(PaneState::new(tabs), ThemeMode::Dark)
    }

    fn only_location(workspace: &Workspace) -> PanelLocation {
        let pane = *workspace.panes.iter().next().unwrap().0;
        PanelLocation::Center(pane)
    }

    #[test]
    fn lone_pane_is_focused_on_launch() {
        let workspace = three_tab_workspace();
        assert_eq!(workspace.focused, only_location(&workspace));
    }

    #[test]
    fn tab_selected_changes_active_tab() {
        let mut workspace = three_tab_workspace();
        let location = only_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        let pane = match location {
            PanelLocation::Center(pane) => pane,
        };
        assert_eq!(workspace.panes.get(pane).unwrap().active, 1);
    }

    #[test]
    fn tab_selected_also_focuses_its_pane() {
        let mut workspace = three_tab_workspace();
        let location = only_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        assert!(workspace.is_focused(location));
    }

    #[test]
    fn out_of_range_tab_selection_is_ignored() {
        let mut workspace = three_tab_workspace();
        let location = only_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected {
            location,
            tab: 99,
        });

        let pane = match location {
            PanelLocation::Center(pane) => pane,
        };
        assert_eq!(workspace.panes.get(pane).unwrap().active, 0);
    }

    #[test]
    fn pane_clicked_sets_focus() {
        let mut workspace = three_tab_workspace();
        let pane = *workspace.panes.iter().next().unwrap().0;

        workspace.update(WorkspaceMessage::PaneClicked(pane));

        assert_eq!(workspace.focused, PanelLocation::Center(pane));
    }

    #[test]
    fn panel_message_routes_to_addressed_panel() {
        let mut workspace = three_tab_workspace();
        let location = only_location(&workspace);

        // Counter sits at tab 0; an erased increment should fold its
        // state via the routed update path.
        workspace.update(WorkspaceMessage::Panel {
            location,
            tab: 0,
            message: erase(
                crate::features::dummies::counter::CounterMessage::Increment,
            ),
        });

        let pane = match location {
            PanelLocation::Center(pane) => pane,
        };
        let counter = workspace.panes.get(pane).unwrap().tabs[0]
            .snapshot()
            .get("count")
            .and_then(|v| v.as_i64())
            .unwrap();
        assert_eq!(counter, 1);
    }

    #[test]
    fn panel_message_to_stale_tab_is_noop() {
        let mut workspace = three_tab_workspace();
        let location = only_location(&workspace);

        // Tab 50 does not exist; routing must silently no-op rather than
        // panic.
        workspace.update(WorkspaceMessage::Panel {
            location,
            tab: 50,
            message: erase(ClockMessage::Tick),
        });
    }
}
