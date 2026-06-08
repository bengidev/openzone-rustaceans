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
use crate::workspace::panel::{ErasedMessage, Panel, PanelKind};
use crate::workspace::stores::AppStores;

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

    /// Assemble a workspace from already-built parts. The composition
    /// root uses this to restore a persisted layout: the pane grid, edge
    /// docks, and focus location are reconstructed from a snapshot, then
    /// handed here with the shipped keymap and resolved theme. The
    /// caller guarantees `focused` addresses a location that exists in
    /// `panes`/`docks`.
    pub fn from_parts(
        panes: pane_grid::State<PaneState>,
        docks: Docks,
        focused: PanelLocation,
        theme_mode: ThemeMode,
    ) -> Self {
        Self {
            panes,
            docks,
            keymap: Keymap::default(),
            focused,
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
    ///
    /// `stores` is the app-root [`AppStores`] borrowed as a sibling
    /// field of the workspace; every domain mutation (Counter intents,
    /// Clock ticks, panel `on_close` slot release) flows through this
    /// one method. There is no interior mutability — the workspace
    /// reducer is the single writer of both layout state (via `&mut
    /// self`) and domain state (via `&mut AppStores`).
    pub fn update(&mut self, message: WorkspaceMessage, stores: &mut AppStores) {
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
                self.route_to_panel(location, tab, message, stores);
            }
            WorkspaceMessage::Key(chord) => self.handle_key(chord, stores),
            WorkspaceMessage::Command(command) => self.apply_command(command, stores),
            WorkspaceMessage::ToggleTheme => {
                self.theme_mode = self.theme_mode.toggle();
                self.theme = OpenZoneTheme::from_mode(self.theme_mode);
            }
            WorkspaceMessage::ClockTick => {
                // Single store-level Clock subscription: one mutation
                // per tick, every Clock panel reads the same value.
                stores.clock.tick();
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
    fn handle_key(&mut self, chord: Chord, stores: &mut AppStores) {
        if self
            .focused_active_panel()
            .is_some_and(|panel| panel.captures_chord(chord))
        {
            return;
        }

        if let Some(command) = self.keymap.resolve(chord) {
            self.apply_command(command, stores);
        }
    }

    /// Execute a workspace command.
    pub fn apply_command(&mut self, command: Command, stores: &mut AppStores) {
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
            Command::CloseActiveTab => self.close_active_tab(stores),
        }
    }

    /// Close the active tab in the focused pane or dock. The removed
    /// panel's [`Panel::on_close`] runs first so a store-backed panel
    /// (Counter) releases its slot before the box drops; an emptied
    /// center pane collapses out of the split tree, an emptied dock
    /// collapses to a rail.
    fn close_active_tab(&mut self, stores: &mut AppStores) {
        let focused = self.focused;
        let removed = self
            .pane_state_mut(focused)
            .and_then(|pane_state| pane_state.close_active());

        let Some(mut panel) = removed else {
            return;
        };

        // Release any store handle this panel held (e.g. a CounterId).
        // Runs *before* we touch the split tree so even if pane collapse
        // takes a different code path, the slot is always freed.
        panel.on_close(stores);
        drop(panel);

        let now_empty = self
            .pane_state(focused)
            .map(|pane_state| pane_state.is_empty())
            .unwrap_or(false);

        if !now_empty {
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

    /// Deliver an erased intent to the exact panel that produced it.
    /// A panel that has since been removed (stale tab index) is a no-op.
    /// The panel's `update` receives `&mut AppStores`, so a store-backed
    /// intent (Counter increment) lifts into a single store mutation
    /// without leaving the workspace reducer.
    fn route_to_panel(
        &mut self,
        location: PanelLocation,
        tab: usize,
        message: ErasedMessage,
        stores: &mut AppStores,
    ) {
        if let Some(pane_state) = self.pane_state_mut(location)
            && let Some(panel) = pane_state.tabs.get_mut(tab)
        {
            panel.update(message, stores);
        }
    }

    /// Whether any Clock panel exists anywhere in the layout. Drives
    /// the gating predicate for the single store-level Clock
    /// subscription: when no Clock panel is open, the timer is stopped
    /// and no orphan ticks reach the reducer.
    fn has_clock_panel(&self) -> bool {
        let center_has = self
            .panes
            .iter()
            .any(|(_, pane_state)| pane_has_clock(pane_state));
        if center_has {
            return true;
        }
        DockSide::ALL
            .iter()
            .any(|side| pane_has_clock(&self.docks.get(*side).tabs))
    }

    /// Batch every live panel's *panel-local* subscription, plus the
    /// single store-level Clock tick (gated on any Clock panel
    /// existing), into one workspace stream.
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

        if self.has_clock_panel() {
            streams.push(
                iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| WorkspaceMessage::ClockTick),
            );
        }

        Subscription::batch(streams)
    }
}

/// Whether a pane stack contains at least one Clock panel.
fn pane_has_clock(pane_state: &PaneState) -> bool {
    pane_state
        .tabs
        .iter()
        .any(|panel| panel.kind() == PanelKind::Clock)
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
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
    use crate::workspace::command::{Chord, Mods};
    use crate::workspace::panel::{Panel, erase};

    /// Build a two-tab workspace plus the app-root stores it views over.
    /// Returning the stores alongside lets each test run the reducer with
    /// the same `&mut AppStores` borrow the live app uses.
    fn three_tab_workspace() -> (Workspace, AppStores) {
        let mut stores = AppStores::new();
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
        ];
        let workspace = Workspace::single_pane(PaneState::new(tabs), ThemeMode::Dark);
        (workspace, stores)
    }

    fn workspace_with_right_dock() -> (Workspace, AppStores) {
        let mut stores = AppStores::new();
        let center = PaneState::new(vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
        ]);
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::empty(),
        );
        let workspace = Workspace::with_docks(center, docks, ThemeMode::Dark);
        (workspace, stores)
    }

    fn only_center_location(workspace: &Workspace) -> PanelLocation {
        let pane = *workspace.panes.iter().next().unwrap().0;
        PanelLocation::Center(pane)
    }

    #[test]
    fn lone_pane_is_focused_on_launch() {
        let (workspace, _stores) = three_tab_workspace();
        assert_eq!(workspace.focused, only_center_location(&workspace));
    }

    #[test]
    fn tab_selected_changes_active_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().active, 1);
    }

    #[test]
    fn tab_selected_also_focuses_its_pane() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );

        assert!(workspace.is_focused(location));
    }

    #[test]
    fn out_of_range_tab_selection_is_ignored() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 99 },
            &mut stores,
        );

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().active, 0);
    }

    #[test]
    fn pane_clicked_sets_focus() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let pane = *workspace.panes.iter().next().unwrap().0;

        workspace.update(WorkspaceMessage::PaneClicked(pane), &mut stores);

        assert_eq!(workspace.focused, PanelLocation::Center(pane));
    }

    /// Store mutation boundary, per the issue's acceptance test:
    /// one Counter intent routed through workspace `update` lifts into
    /// exactly one [`CounterStore`] mutation that every observing
    /// Counter panel in the layout reflects.
    #[test]
    fn counter_intent_lifts_to_a_single_store_mutation() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::Panel {
                location,
                tab: 0,
                message: erase(crate::features::dummies::counter::CounterMessage::Increment),
            },
            &mut stores,
        );

        // Exactly one slot, exactly one increment, end-state count == 1.
        assert_eq!(stores.counter.len(), 1);
        let snapshot = workspace.panes.iter().next().unwrap().1.tabs[0].snapshot(&stores);
        assert_eq!(snapshot.get("count").and_then(|v| v.as_i64()), Some(1));
    }

    #[test]
    fn panel_message_to_stale_tab_is_noop() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::Panel {
                location,
                tab: 50,
                message: erase(crate::features::dummies::counter::CounterMessage::Increment),
            },
            &mut stores,
        );
    }

    #[test]
    fn toggle_dock_opens_and_focuses() {
        let (mut workspace, mut stores) = workspace_with_right_dock();

        workspace.apply_command(Command::ToggleDock(DockSide::Right), &mut stores);

        assert!(workspace.docks.right.open);
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn toggle_dock_closes_without_changing_focus_location() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.apply_command(Command::ToggleDock(DockSide::Right), &mut stores);

        workspace.apply_command(Command::ToggleDock(DockSide::Right), &mut stores);

        assert!(!workspace.docks.right.open);
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn dock_focused_sets_focus() {
        let (mut workspace, mut stores) = workspace_with_right_dock();

        workspace.update(
            WorkspaceMessage::DockFocused(PanelLocation::Dock(DockSide::Right)),
            &mut stores,
        );

        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn tab_selected_in_dock_changes_active_tab() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        let location = PanelLocation::Dock(DockSide::Right);

        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 0 },
            &mut stores,
        );

        assert_eq!(workspace.docks.right.tabs.active, 0);
        assert_eq!(workspace.focused, location);
    }

    #[test]
    fn split_focused_creates_second_center_pane() {
        let (mut workspace, mut stores) = three_tab_workspace();
        assert_eq!(workspace.panes.len(), 1);

        workspace.apply_command(Command::SplitFocused, &mut stores);

        assert_eq!(workspace.panes.len(), 2);
        assert!(matches!(workspace.focused, PanelLocation::Center(_)));
    }

    #[test]
    fn split_focused_is_noop_when_dock_is_focused() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.focused = PanelLocation::Dock(DockSide::Right);

        workspace.apply_command(Command::SplitFocused, &mut stores);

        assert_eq!(workspace.panes.len(), 1);
    }

    #[test]
    fn close_active_tab_removes_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );

        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
        assert_eq!(
            workspace.panes.get(pane).unwrap().tabs[0].title(),
            "Counter"
        );
    }

    /// Closing a Counter panel must release its [`CounterStore`] slot.
    /// Without this the store would leak ids across panel lifecycles.
    #[test]
    fn closing_counter_tab_releases_store_slot() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        // Select tab 0 (the Counter) and close it.
        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 0 },
            &mut stores,
        );
        assert_eq!(stores.counter.len(), 1);

        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        assert_eq!(stores.counter.len(), 0);
    }

    #[test]
    fn closing_last_tab_collapses_center_pane() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let first = only_center_location(&workspace);
        workspace.apply_command(Command::SplitFocused, &mut stores);
        let second = workspace.focused;

        workspace.focused = first;
        workspace.apply_command(Command::CloseActiveTab, &mut stores);
        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        assert_eq!(workspace.panes.len(), 1);
        assert_eq!(workspace.focused, second);
    }

    #[test]
    fn closing_last_tab_in_only_pane_collapses_dock() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.focused = PanelLocation::Dock(DockSide::Right);
        workspace.docks.right.open = true;

        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        assert!(workspace.docks.right.tabs.is_empty());
        assert!(!workspace.docks.right.open);
    }

    #[test]
    fn key_resolves_to_split_command() {
        let (mut workspace, mut stores) = three_tab_workspace();

        workspace.update(
            WorkspaceMessage::Key(Chord::ch('d', Mods::CMD)),
            &mut stores,
        );

        assert_eq!(workspace.panes.len(), 2);
    }

    #[test]
    fn key_captured_by_text_panel_does_not_close_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );

        workspace.update(
            WorkspaceMessage::Key(Chord::ch('a', Mods::NONE)),
            &mut stores,
        );

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 2);
    }

    #[test]
    fn command_shortcut_reaches_keymap_while_text_is_focused() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );

        workspace.update(
            WorkspaceMessage::Key(Chord::ch('w', Mods::CMD)),
            &mut stores,
        );

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
    }

    /// Single store-level Clock subscription:
    /// one [`WorkspaceMessage::ClockTick`] folds into exactly one
    /// [`ClockStore::tick`], and every Clock panel in the layout
    /// re-renders against the same value.
    #[test]
    fn clock_tick_updates_store_once_for_all_observers() {
        let mut stores = AppStores::new();
        let center = PaneState::new(vec![
            Box::new(ClockPanel::new()) as Box<dyn Panel>,
            Box::new(ClockPanel::new()) as Box<dyn Panel>,
        ]);
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::empty(),
        );
        let mut workspace = Workspace::with_docks(center, docks, ThemeMode::Dark);

        workspace.update(WorkspaceMessage::ClockTick, &mut stores);
        workspace.update(WorkspaceMessage::ClockTick, &mut stores);

        // One store mutation per tick — the store's tick count is the
        // number of WorkspaceMessage::ClockTick the reducer saw.
        assert_eq!(stores.clock.ticks(), 2);

        // All three observing Clock panels read the same value: this
        // is what "removing a Clock tab does not leave orphan
        // subscriptions" depends on, since there is exactly one
        // subscription source for the whole fan-out.
        let center_pane = workspace.panes.iter().next().unwrap().1;
        let center_a = center_pane.tabs[0].snapshot(&stores);
        let center_b = center_pane.tabs[1].snapshot(&stores);
        let dock_a = workspace.docks.right.tabs.tabs[0].snapshot(&stores);
        assert_eq!(center_a, center_b);
        assert_eq!(center_a, dock_a);
        assert_eq!(center_a.get("ticks").and_then(|v| v.as_u64()), Some(2));
    }
}
