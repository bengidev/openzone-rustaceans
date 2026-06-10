#![allow(dead_code)]

//! Workspace state and reducer.
//!
//! The workspace owns the center `pane_grid`, each pane's tab stack
//! ([`PaneState`]), edge [`Docks`], a workspace [`Keymap`], and a single
//! centrally-owned focus. All panel messages funnel through
//! [`Workspace::update`] (single-writer). The view layer reads this state
//! by `&self`; only the reducer mutates.

use iced::event::{self, Event};
use iced::mouse;
use iced::widget::pane_grid::{self, Axis};
use iced::{Point, Size, Subscription};

use crate::shared::design::{OpenZoneTheme, ThemeMode};
use crate::workspace::command::{Chord, Command, Keymap};
use crate::workspace::dock::Docks;
use crate::workspace::drag::{
    Direction, DockRegions, DragState, DropTarget, PaneBounds, SplitPaneTarget, TabStripTarget,
};
use crate::workspace::location::{DockSide, PanelLocation};
use crate::workspace::message::WorkspaceMessage;
use crate::workspace::pane_state::PaneState;
use crate::workspace::panel::{ErasedMessage, Panel, PanelKind};
use crate::workspace::stores::AppStores;

/// Per-window workspace shell state.
///
/// Each OS window owns one `Workspace` (pane tree, docks, focus). Domain
/// data lives in app-root [`AppStores`], keyed by `window::Id` only at
/// the daemon routing layer.
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
    /// Active tab drag state. `Some` while the user is dragging a tab.
    pub drag_state: Option<DragState>,
    /// Latest logical window size for drag bounds and drop-target preview.
    pub window_size: Size,
}

/// Default workspace window size — matches `workspace_window_settings` in
/// the composition root and the single-window harness.
pub const DEFAULT_WINDOW_SIZE: Size = Size {
    width: 1100.0,
    height: 760.0,
};

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
            drag_state: None,
            window_size: DEFAULT_WINDOW_SIZE,
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
            drag_state: None,
            window_size: DEFAULT_WINDOW_SIZE,
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
            WorkspaceMessage::NewWindow => {}
            WorkspaceMessage::ClockTick => {
                stores.clock.tick();
            }
            WorkspaceMessage::PaneDragged(event) => {
                if let pane_grid::DragEvent::Dropped { pane, target } = event {
                    self.panes.drop(pane, target);
                    self.focused = PanelLocation::Center(pane);
                }
            }
            WorkspaceMessage::TabDragStarted { location, tab } => {
                self.drag_state = Some(DragState::new(location, tab));
            }
            WorkspaceMessage::CursorMoved(cursor) => {
                if self.drag_state.is_some() {
                    let (pane_bounds, (rails, bodies)) = self.drag_geometry();
                    if let Some(drag) = self.drag_state.as_mut() {
                        drag.pointer_moved = true;
                        drag.target = crate::workspace::drag::compute_drop_target(
                            cursor,
                            &pane_bounds,
                            &rails,
                            &bodies,
                        );
                    }
                }
            }
            WorkspaceMessage::WindowResized(size) => {
                self.window_size = size;
            }
            WorkspaceMessage::TabDragDropped => {
                if let Some(drag) = self.drag_state.take() {
                    self.apply_drop(drag, stores);
                }
            }
        }
    }

    /// Pane and dock rectangles used for drag hit-testing and preview.
    fn drag_geometry(&self) -> (Vec<PaneBounds>, DockRegions) {
        let grid = crate::workspace::drag::compute_grid_bounds(&self.docks, self.window_size);
        let pane_bounds = crate::workspace::drag::compute_pane_bounds(&self.panes, grid);
        let dock_regions =
            crate::workspace::drag::compute_dock_regions(&self.docks, self.window_size);
        (pane_bounds, dock_regions)
    }

    /// Apply a completed tab drag operation.
    fn apply_drop(&mut self, drag: DragState, _stores: &mut AppStores) {
        if matches!(drag.target, DropTarget::None) && !drag.pointer_moved {
            self.focused = drag.source_location;
            if let Some(pane_state) = self.pane_state_mut(drag.source_location) {
                pane_state.select(drag.source_tab);
            }
            return;
        }

        // Extract the dragged panel from its source.
        let source = drag.source_location;
        let tab_idx = drag.source_tab;
        let panel = self.pane_state_mut(source).and_then(|ps| {
            if tab_idx < ps.tabs.len() {
                Some(ps.tabs.remove(tab_idx))
            } else {
                None
            }
        });

        let Some(panel) = panel else {
            return;
        };

        // Clamp active index if the removed tab was active.
        if let Some(ps) = self.pane_state_mut(source)
            && ps.active >= ps.tabs.len()
            && !ps.tabs.is_empty()
        {
            ps.active = ps.tabs.len() - 1;
        }

        match drag.target {
            DropTarget::TabStrip(target) => {
                if let Some(ps) = self.pane_state_mut(target.location) {
                    let insert_at = target.index.min(ps.tabs.len());
                    ps.tabs.insert(insert_at, panel);
                    ps.active = insert_at;
                }
                self.focused = target.location;
            }
            DropTarget::SplitPane(target) => {
                let axis = match target.direction {
                    Direction::Left | Direction::Right => Axis::Vertical,
                    Direction::Up | Direction::Down => Axis::Horizontal,
                };
                let new_pane = PaneState::new(vec![panel]);
                if let Some((split_pane, _)) =
                    self.panes.split(axis, target.pane, PaneState::empty())
                {
                    if let Some(ps) = self.panes.get_mut(split_pane) {
                        *ps = new_pane;
                    }
                    self.focused = PanelLocation::Center(split_pane);
                }
            }
            DropTarget::Dock(side) => {
                let dock = self.docks.get_mut(side);
                dock.tabs.tabs.push(panel);
                dock.tabs.active = dock.tabs.tabs.len() - 1;
                dock.open = true;
                self.focused = PanelLocation::Dock(side);
            }
            DropTarget::None => {
                // Tear-off: panel already removed from source, just drop it.
            }
        }

        // Clean up empty source panes/docks.
        self.cleanup_empty_source(source);
    }

    /// Remove an empty center pane or collapse an empty dock after a
    /// tab was dragged away.
    fn cleanup_empty_source(&mut self, location: PanelLocation) {
        let is_empty = self
            .pane_state(location)
            .map(|ps| ps.is_empty())
            .unwrap_or(false);

        if !is_empty {
            return;
        }

        match location {
            PanelLocation::Center(pane) => {
                if self.panes.len() > 1
                    && let Some((_, sibling)) = self.panes.close(pane)
                    && self.focused == PanelLocation::Center(pane)
                {
                    self.focused = PanelLocation::Center(sibling);
                }
            }
            PanelLocation::Dock(side) => {
                self.docks.get_mut(side).open = false;
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

    /// Whether any Clock panel exists anywhere in the layout. The
    /// composition root uses this to gate the single app-level Clock
    /// subscription across all workspace windows.
    pub fn has_clock_panel(&self) -> bool {
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

    /// Batch every live panel's panel-local subscription for this window.
    ///
    /// The 1 Hz Clock tick is owned once at app root in the multi-window
    /// daemon; the single-window [`crate::workspace::run`] harness adds
    /// it back when composing subscriptions.
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

        if self.drag_state.is_some() {
            streams.push(tab_drag_events());
        }

        Subscription::batch(streams)
    }
}

/// Track cursor movement and mouse release while a tab drag is active.
fn tab_drag_events() -> Subscription<WorkspaceMessage> {
    event::listen_with(|event, _status, _window| match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(WorkspaceMessage::CursorMoved(position))
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(WorkspaceMessage::TabDragDropped)
        }
        _ => None,
    })
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
    use crate::workspace::stores::CounterId;

    /// Build a two-tab workspace plus the app-root stores it views over.
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

    #[test]
    fn closing_counter_tab_releases_store_slot() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
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

        assert_eq!(stores.clock.ticks(), 2);

        let center_pane = workspace.panes.iter().next().unwrap().1;
        let center_a = center_pane.tabs[0].snapshot(&stores);
        let center_b = center_pane.tabs[1].snapshot(&stores);
        let dock_a = workspace.docks.right.tabs.tabs[0].snapshot(&stores);
        assert_eq!(center_a, center_b);
        assert_eq!(center_a, dock_a);
        assert_eq!(center_a.get("ticks").and_then(|v| v.as_u64()), Some(2));
    }

    fn workspace_with_shared_counter(counter_id: CounterId) -> Workspace {
        let center = PaneState::new(vec![
            Box::new(CounterPanel::with_id(counter_id)) as Box<dyn Panel>,
            Box::new(ClockPanel::new()) as Box<dyn Panel>,
        ]);
        Workspace::single_pane(center, ThemeMode::Dark)
    }

    #[test]
    fn two_workspaces_observe_same_counter_after_single_mutation() {
        let mut stores = AppStores::new();
        let counter_id = stores.counter.create();
        let mut workspace_a = workspace_with_shared_counter(counter_id);
        let workspace_b = workspace_with_shared_counter(counter_id);
        let location_a = only_center_location(&workspace_a);

        workspace_a.update(
            WorkspaceMessage::Panel {
                location: location_a,
                tab: 0,
                message: erase(crate::features::dummies::counter::CounterMessage::Increment),
            },
            &mut stores,
        );

        let snapshot_a = workspace_a.panes.iter().next().unwrap().1.tabs[0].snapshot(&stores);
        let snapshot_b = workspace_b.panes.iter().next().unwrap().1.tabs[0].snapshot(&stores);

        assert_eq!(snapshot_a, snapshot_b);
        assert_eq!(snapshot_a.get("count").and_then(|v| v.as_i64()), Some(1));
    }

    #[test]
    fn clock_store_tick_reaches_two_workspaces() {
        let mut stores = AppStores::new();
        let workspace_a = workspace_with_shared_counter(stores.counter.create());
        let workspace_b = Workspace::single_pane(
            PaneState::new(vec![Box::new(ClockPanel::new()) as Box<dyn Panel>]),
            ThemeMode::Dark,
        );

        stores.clock.tick();
        stores.clock.tick();
        stores.clock.tick();

        let clock_a = workspace_a.panes.iter().next().unwrap().1.tabs[1].snapshot(&stores);
        let clock_b = workspace_b.panes.iter().next().unwrap().1.tabs[0].snapshot(&stores);

        assert_eq!(clock_a, clock_b);
        assert_eq!(clock_a.get("ticks").and_then(|v| v.as_u64()), Some(3));
    }

    // ─── Tab drag-and-drop reducer tests ───

    #[test]
    fn tab_drag_started_sets_drag_state() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        assert!(workspace.drag_state.is_some());
        let drag = workspace.drag_state.as_ref().unwrap();
        assert_eq!(drag.source_location, location);
        assert_eq!(drag.source_tab, 0);
    }

    #[test]
    fn cursor_moved_updates_drop_target() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        // Cursor in the center of the pane body (below tab strip)
        workspace.update(
            WorkspaceMessage::CursorMoved(iced::Point::new(400.0, 350.0)),
            &mut stores,
        );

        let drag = workspace.drag_state.as_ref().unwrap();
        assert!(matches!(drag.target, DropTarget::TabStrip(_)));
    }

    #[test]
    fn tab_drag_dropped_moves_tab_to_dock() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        workspace.drag_state.as_mut().unwrap().target = DropTarget::Dock(DockSide::Right);

        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        assert!(workspace.drag_state.is_none());
        assert!(workspace.docks.right.open);
        assert_eq!(workspace.docks.right.tabs.len(), 1);
        assert_eq!(workspace.docks.right.tabs.tabs[0].title(), "Counter");

        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
    }

    #[test]
    fn tab_drag_dropped_splits_pane() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 1 },
            &mut stores,
        );

        let pane = match location {
            PanelLocation::Center(p) => p,
            _ => panic!(),
        };

        workspace.drag_state.as_mut().unwrap().target = DropTarget::SplitPane(SplitPaneTarget {
            pane,
            direction: Direction::Right,
        });

        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        assert!(workspace.drag_state.is_none());
        assert_eq!(workspace.panes.len(), 2);
        assert!(matches!(workspace.focused, PanelLocation::Center(_)));
    }

    #[test]
    fn tab_drag_dropped_none_removes_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        {
            let drag = workspace.drag_state.as_mut().unwrap();
            drag.target = DropTarget::None;
            drag.pointer_moved = true;
        }

        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        assert!(workspace.drag_state.is_none());
        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
        assert_eq!(workspace.panes.get(pane).unwrap().tabs[0].title(), "Text");
    }

    #[test]
    fn tab_drag_dropped_without_movement_selects_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 1 },
            &mut stores,
        );
        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        assert!(workspace.drag_state.is_none());
        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 2);
        assert_eq!(workspace.panes.get(pane).unwrap().active, 1);
        assert_eq!(workspace.focused, location);
    }

    #[test]
    fn native_pane_grid_drag_still_works() {
        let (mut workspace, mut stores) = three_tab_workspace();
        workspace.apply_command(Command::SplitFocused, &mut stores);
        assert_eq!(workspace.panes.len(), 2);

        let panes: Vec<pane_grid::Pane> = workspace.panes.iter().map(|(p, _)| *p).collect();

        workspace.update(
            WorkspaceMessage::PaneDragged(pane_grid::DragEvent::Dropped {
                pane: panes[0],
                target: pane_grid::Target::Pane(panes[1], pane_grid::Region::Center),
            }),
            &mut stores,
        );

        assert_eq!(workspace.panes.len(), 2);
        assert_eq!(workspace.focused, PanelLocation::Center(panes[0]));
    }
}
