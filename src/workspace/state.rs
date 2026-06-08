#![allow(dead_code)]

//! Workspace state and reducer.
//!
//! The workspace owns the center `pane_grid`, each pane's tab stack
//! ([`PaneState`]), edge [`Docks`], a workspace [`Keymap`], and a single
//! centrally-owned focus. All panel messages funnel through
//! [`Workspace::update`] (single-writer). The view layer reads this state
//! by `&self`; only the reducer mutates.

use iced::Subscription;
use iced::widget::pane_grid::{self, Axis};

use crate::shared::design::{OpenZoneTheme, ThemeMode};
use crate::workspace::command::{Chord, Command, Keymap};
use crate::workspace::dock::Docks;
use crate::workspace::location::{DockSide, PanelLocation};
use crate::workspace::message::WorkspaceMessage;
use crate::workspace::pane_state::PaneState;
use crate::workspace::panel::{ErasedMessage, Panel};

/// The single-window workspace shell state.
pub struct Workspace {
    /// Iced's recursive split tree of panes (splits *between* panes).
    pub panes: pane_grid::State<PaneState>,
    /// Collapsible edge docks framing the center workspace.
    pub docks: Docks,
    /// Chord-to-command bindings for workspace-level shortcuts.
    pub keymap: Keymap,
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
        Self::with_docks(tabs, Docks::empty(), theme_mode)
    }

    /// Build a workspace with center tabs and edge docks.
    pub fn with_docks(center: PaneState, docks: Docks, theme_mode: ThemeMode) -> Self {
        let (panes, first) = pane_grid::State::new(center);
        Self {
            panes,
            docks,
            keymap: Keymap::default(),
            focused: PanelLocation::Center(first),
            theme: OpenZoneTheme::from_mode(theme_mode),
            theme_mode,
        }
    }

    /// The pane backing a location, if it still exists.
    fn pane_state(&self, location: PanelLocation) -> Option<&PaneState> {
        match location {
            PanelLocation::Center(pane) => self.panes.get(pane),
            PanelLocation::Dock(side) => Some(&self.docks.get(side).tabs),
        }
    }

    fn pane_state_mut(&mut self, location: PanelLocation) -> Option<&mut PaneState> {
        match location {
            PanelLocation::Center(pane) => self.panes.get_mut(pane),
            PanelLocation::Dock(side) => Some(&mut self.docks.get_mut(side).tabs),
        }
    }

    /// The focused panel's active tab, if any.
    fn focused_active_panel(&self) -> Option<&dyn Panel> {
        self.pane_state(self.focused)?.active_panel()
    }

    /// Whether `location` is the focused location.
    pub fn is_focused(&self, location: PanelLocation) -> bool {
        self.focused == location
    }

    /// Fold a workspace message into state. Single mutation path.
    pub fn update(&mut self, message: WorkspaceMessage) {
        match message {
            WorkspaceMessage::PaneClicked(pane) => {
                self.focused = PanelLocation::Center(pane);
            }
            WorkspaceMessage::DockFocused(location) => {
                self.focused = location;
            }
            WorkspaceMessage::TabSelected { location, tab } => {
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
            WorkspaceMessage::Key(chord) => self.handle_key(chord),
            WorkspaceMessage::Command(command) => self.apply_command(command),
            WorkspaceMessage::ToggleTheme => {
                self.theme_mode = self.theme_mode.toggle();
                self.theme = OpenZoneTheme::from_mode(self.theme_mode);
            }
            WorkspaceMessage::PaneDragged(event) => {
                if let pane_grid::DragEvent::Dropped { pane, target } = event {
                    self.panes.drop(pane, target);
                    self.focused = PanelLocation::Center(pane);
                }
            }
        }
    }

    /// Panel-first key routing: the focused panel may swallow a chord;
    /// otherwise the workspace keymap resolves it to a command.
    fn handle_key(&mut self, chord: Chord) {
        if self
            .focused_active_panel()
            .is_some_and(|panel| panel.captures_chord(chord))
        {
            return;
        }

        if let Some(command) = self.keymap.resolve(chord) {
            self.apply_command(command);
        }
    }

    /// Execute a workspace command.
    pub fn apply_command(&mut self, command: Command) {
        match command {
            Command::ToggleDock(side) => {
                let opened = self.docks.toggle(side);
                if opened {
                    self.focused = PanelLocation::Dock(side);
                }
            }
            Command::SplitFocused => {
                if let PanelLocation::Center(pane) = self.focused
                    && let Some((new_pane, _)) =
                        self.panes.split(Axis::Vertical, pane, PaneState::empty())
                {
                    self.focused = PanelLocation::Center(new_pane);
                }
            }
            Command::CloseActiveTab => self.close_active_tab(),
        }
    }

    /// Close the active tab in the focused pane or dock. An emptied center
    /// pane collapses out of the split tree; an emptied dock collapses.
    fn close_active_tab(&mut self) {
        let focused = self.focused;
        let emptied = self
            .pane_state_mut(focused)
            .is_some_and(|pane_state| pane_state.close_active());

        if !emptied {
            return;
        }

        match focused {
            PanelLocation::Center(pane) => {
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focused = PanelLocation::Center(sibling);
                }
            }
            PanelLocation::Dock(side) => {
                self.docks.get_mut(side).open = false;
            }
        }
    }

    /// Deliver an erased message to the exact panel that produced it.
    /// A panel that has since been removed (stale tab index) is a no-op.
    fn route_to_panel(&mut self, location: PanelLocation, tab: usize, message: ErasedMessage) {
        if let Some(pane_state) = self.pane_state_mut(location)
            && let Some(panel) = pane_state.tabs.get_mut(tab)
        {
            panel.update(message);
        }
    }

    /// Batch every live panel's subscription into one workspace stream.
    pub fn subscription(&self) -> Subscription<WorkspaceMessage> {
        let mut streams = Vec::new();

        for (pane, pane_state) in self.panes.iter() {
            let location = PanelLocation::Center(*pane);
            streams.extend(panel_subscriptions(location, pane_state));
        }

        for side in DockSide::ALL {
            let location = PanelLocation::Dock(side);
            streams.extend(panel_subscriptions(location, &self.docks.get(side).tabs));
        }

        Subscription::batch(streams)
    }
}

fn panel_subscriptions(
    location: PanelLocation,
    pane_state: &PaneState,
) -> Vec<Subscription<WorkspaceMessage>> {
    pane_state
        .tabs
        .iter()
        .enumerate()
        .map(|(tab, panel)| {
            panel
                .subscription()
                .with((location, tab))
                .map(|((location, tab), message)| WorkspaceMessage::Panel {
                    location,
                    tab,
                    message,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::dummies::clock::ClockMessage;
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
    use crate::workspace::command::{Chord, Mods};
    use crate::workspace::panel::{Panel, erase};

    fn three_tab_workspace() -> Workspace {
        let tabs: Vec<Box<dyn Panel>> =
            vec![Box::new(CounterPanel::new()), Box::new(TextPanel::new())];
        Workspace::single_pane(PaneState::new(tabs), ThemeMode::Dark)
    }

    fn workspace_with_right_dock() -> Workspace {
        let center = PaneState::new(vec![
            Box::new(CounterPanel::new()),
            Box::new(TextPanel::new()),
        ]);
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::empty(),
        );
        Workspace::with_docks(center, docks, ThemeMode::Dark)
    }

    fn only_center_location(workspace: &Workspace) -> PanelLocation {
        let pane = *workspace.panes.iter().next().unwrap().0;
        PanelLocation::Center(pane)
    }

    #[test]
    fn lone_pane_is_focused_on_launch() {
        let workspace = three_tab_workspace();
        assert_eq!(workspace.focused, only_center_location(&workspace));
    }

    #[test]
    fn tab_selected_changes_active_tab() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().active, 1);
    }

    #[test]
    fn tab_selected_also_focuses_its_pane() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        assert!(workspace.is_focused(location));
    }

    #[test]
    fn out_of_range_tab_selection_is_ignored() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 99 });

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
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
        let location = only_center_location(&workspace);

        workspace.update(WorkspaceMessage::Panel {
            location,
            tab: 0,
            message: erase(crate::features::dummies::counter::CounterMessage::Increment),
        });

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
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
        let location = only_center_location(&workspace);

        workspace.update(WorkspaceMessage::Panel {
            location,
            tab: 50,
            message: erase(ClockMessage::Tick),
        });
    }

    #[test]
    fn toggle_dock_opens_and_focuses() {
        let mut workspace = workspace_with_right_dock();

        workspace.apply_command(Command::ToggleDock(DockSide::Right));

        assert!(workspace.docks.right.open);
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn toggle_dock_closes_without_changing_focus_location() {
        let mut workspace = workspace_with_right_dock();
        workspace.apply_command(Command::ToggleDock(DockSide::Right));

        workspace.apply_command(Command::ToggleDock(DockSide::Right));

        assert!(!workspace.docks.right.open);
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn dock_focused_sets_focus() {
        let mut workspace = workspace_with_right_dock();

        workspace.update(WorkspaceMessage::DockFocused(PanelLocation::Dock(
            DockSide::Right,
        )));

        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn tab_selected_in_dock_changes_active_tab() {
        let mut workspace = workspace_with_right_dock();
        let location = PanelLocation::Dock(DockSide::Right);

        workspace.update(WorkspaceMessage::TabSelected { location, tab: 0 });

        assert_eq!(workspace.docks.right.tabs.active, 0);
        assert_eq!(workspace.focused, location);
    }

    #[test]
    fn split_focused_creates_second_center_pane() {
        let mut workspace = three_tab_workspace();
        assert_eq!(workspace.panes.len(), 1);

        workspace.apply_command(Command::SplitFocused);

        assert_eq!(workspace.panes.len(), 2);
        assert!(matches!(workspace.focused, PanelLocation::Center(_)));
    }

    #[test]
    fn split_focused_is_noop_when_dock_is_focused() {
        let mut workspace = workspace_with_right_dock();
        workspace.focused = PanelLocation::Dock(DockSide::Right);

        workspace.apply_command(Command::SplitFocused);

        assert_eq!(workspace.panes.len(), 1);
    }

    #[test]
    fn close_active_tab_removes_tab() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        workspace.apply_command(Command::CloseActiveTab);

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
        assert_eq!(
            workspace.panes.get(pane).unwrap().tabs[0].title(),
            "Counter"
        );
    }

    #[test]
    fn closing_last_tab_collapses_center_pane() {
        let mut workspace = three_tab_workspace();
        let first = only_center_location(&workspace);
        workspace.apply_command(Command::SplitFocused);
        let second = workspace.focused;

        workspace.focused = first;
        workspace.apply_command(Command::CloseActiveTab);
        workspace.apply_command(Command::CloseActiveTab);

        assert_eq!(workspace.panes.len(), 1);
        assert_eq!(workspace.focused, second);
    }

    #[test]
    fn closing_last_tab_in_only_pane_collapses_dock() {
        let mut workspace = workspace_with_right_dock();
        workspace.focused = PanelLocation::Dock(DockSide::Right);
        workspace.docks.right.open = true;

        workspace.apply_command(Command::CloseActiveTab);

        assert!(workspace.docks.right.tabs.is_empty());
        assert!(!workspace.docks.right.open);
    }

    #[test]
    fn key_resolves_to_split_command() {
        let mut workspace = three_tab_workspace();

        workspace.update(WorkspaceMessage::Key(Chord::ch('d', Mods::CMD)));

        assert_eq!(workspace.panes.len(), 2);
    }

    #[test]
    fn key_captured_by_text_panel_does_not_close_tab() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        workspace.update(WorkspaceMessage::Key(Chord::ch('a', Mods::NONE)));

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 2);
    }

    #[test]
    fn command_shortcut_reaches_keymap_while_text_is_focused() {
        let mut workspace = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(WorkspaceMessage::TabSelected { location, tab: 1 });

        workspace.update(WorkspaceMessage::Key(Chord::ch('w', Mods::CMD)));

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
    }
}
