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
use iced::window;
use iced::{Point, Size, Subscription};

use crate::shared::design::{OpenZoneTheme, ThemeMode};
use crate::workspace::workspace_command::{Chord, Command, Keymap};
use crate::workspace::workspace_dock::{DockVisibility, Docks};
use crate::workspace::workspace_drag::{
    Direction, DockRegions, DragState, DropTarget, PaneBounds, SplitPaneTarget, TabStripTarget,
    WindowDropGeometry, resolve_drop_target_in_geometry,
};
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_message::WorkspaceMessage;
use crate::workspace::workspace_pane_state::PaneState;
use crate::workspace::workspace_panel::{CloseRequest, ErasedMessage, Panel, PanelKind};
use crate::workspace::workspace_stores::AppStores;

/// Drop preview shown on a workspace window while a tab is dragged from
/// another OS window.
#[derive(Debug, Clone)]
pub struct CrossWindowDropPreview {
    pub drag: DragState,
    pub target: DropTarget,
    pub cursor: Point,
}

/// Per-window workspace shell state.
///
/// Each OS window owns one `Workspace` (pane tree, docks, focus). Domain
/// data lives in app-root [`AppStores`], keyed by `window::Id` only at
/// the daemon routing layer.
#[derive(Debug, Clone)]
pub struct CloseConfirmation {
    pub location: PanelLocation,
    pub tab: usize,
    pub message: std::borrow::Cow<'static, str>,
}

/// Signature for a default dock surface factory.
/// The composition root supplies these; the shell calls them when
/// opening an empty dock (if registered) or restoring one after persist.
pub type DockSurfaceFactory = fn(&mut AppStores) -> Box<dyn Panel>;
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
    /// Drop-zone preview while another window's tab hovers over this one.
    pub cross_window_drop_preview: Option<CrossWindowDropPreview>,
    /// Panel waiting to be placed in a new OS window after a tear-off drop.
    torn_off_panel: Option<Box<dyn Panel>>,
    /// Factory for creating scratch panels as fallback when panes empty.
    scratch_factory: Option<fn() -> Box<dyn Panel>>,
    /// Default surface factory for the left (Activity) dock.
    left_dock_factory: Option<DockSurfaceFactory>,
    /// Default surface factory for the right (Conversation) dock.
    right_dock_factory: Option<DockSurfaceFactory>,
    /// Default surface factory for the bottom (Output) dock.
    bottom_dock_factory: Option<DockSurfaceFactory>,
    /// Optional close confirmation when closing a dirty tab.
    pub close_confirmation: Option<CloseConfirmation>,
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
            cross_window_drop_preview: None,
            torn_off_panel: None,
            scratch_factory: None,
            left_dock_factory: None,
            right_dock_factory: None,
            bottom_dock_factory: None,
            close_confirmation: None,
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
            cross_window_drop_preview: None,
            torn_off_panel: None,
            scratch_factory: None,
            left_dock_factory: None,
            right_dock_factory: None,
            bottom_dock_factory: None,
            close_confirmation: None,
        }
    }

    /// Take a panel queued by a tear-off drop, if any.
    pub fn take_torn_off_panel(&mut self) -> Option<Box<dyn Panel>> {
        self.torn_off_panel.take()
    }

    pub fn set_scratch_factory(&mut self, factory: fn() -> Box<dyn Panel>) {
        self.scratch_factory = Some(factory);
    }

    /// Set the default surface factory for a dock side.
    /// Supplied by the composition root; the shell calls it when
    /// opening an empty dock.
    pub fn set_dock_factory(&mut self, side: DockSide, factory: DockSurfaceFactory) {
        match side {
            DockSide::Left => self.left_dock_factory = Some(factory),
            DockSide::Right => self.right_dock_factory = Some(factory),
            DockSide::Bottom => self.bottom_dock_factory = Some(factory),
        }
    }

    /// Whether a dock side has a registered default surface factory.
    pub fn has_dock_factory(&self, side: DockSide) -> bool {
        match side {
            DockSide::Left => self.left_dock_factory.is_some(),
            DockSide::Right => self.right_dock_factory.is_some(),
            DockSide::Bottom => self.bottom_dock_factory.is_some(),
        }
    }

    /// Call the default surface factory for `side`, if registered.
    fn create_dock_default_surface(
        &self,
        side: DockSide,
        stores: &mut AppStores,
    ) -> Option<Box<dyn Panel>> {
        let factory = match side {
            DockSide::Left => self.left_dock_factory,
            DockSide::Right => self.right_dock_factory,
            DockSide::Bottom => self.bottom_dock_factory,
        }?;
        Some(factory(stores))
    }

    /// After restore, fill empty open docks from factories.
    /// Hides open docks that have neither tabs nor a factory.
    pub fn populate_empty_docks(&mut self, stores: &mut AppStores) {
        for side in DockSide::ALL {
            let dock = self.docks.get(side);
            if !dock.is_open() || !dock.is_empty() {
                continue;
            }
            // Create default surface from factory, or hide dock.
            if let Some(panel) = self.create_dock_default_surface(side, stores) {
                let dock = self.docks.get_mut(side);
                dock.tabs.tabs.push(panel);
                dock.tabs.active = 0;
            } else {
                self.docks.get_mut(side).visibility = DockVisibility::Hidden;
            }
        }
    }

    pub(crate) fn ensure_scratch_fallback(&mut self) {
        let Some(factory) = self.scratch_factory else {
            return;
        };
        let empty_panes: Vec<_> = self
            .panes
            .iter()
            .filter(|(_, ps)| ps.is_empty())
            .map(|(pane, _)| *pane)
            .collect();
        for pane in empty_panes {
            if let Some(ps) = self.panes.get_mut(pane) {
                ps.tabs.push(factory());
                ps.active = 0;
                self.focused = PanelLocation::Center(pane);
            }
        }
    }

    /// Seed logical window size before the first resize event arrives.
    pub fn with_window_size(mut self, size: Size) -> Self {
        self.window_size = size;
        self
    }

    /// Whether a tab drag is active in this window.
    pub fn is_tab_drag_active(&self) -> bool {
        self.drag_state.is_some()
    }

    /// Resolve a drop target for `cursor` in this window's client coordinates.
    pub fn resolve_drop_at(&self, cursor: Point, drag: Option<&DragState>) -> DropTarget {
        resolve_drop_target_in_geometry(cursor, &self.drop_geometry(), &self.docks, drag)
    }

    /// Precomputed geometry bundle for cross-window drop hit-testing.
    pub fn drop_geometry(&self) -> WindowDropGeometry {
        let (grid, pane_bounds, (rails, bodies)) = self.drag_geometry();
        WindowDropGeometry {
            window_size: self.window_size,
            grid_bounds: grid,
            pane_bounds,
            dock_rails: rails.to_vec(),
            dock_bodies: bodies.to_vec(),
        }
    }

    /// Whether `cursor` lies inside this window's client area.
    pub fn contains_client_point(&self, cursor: Point) -> bool {
        iced::Rectangle::new(Point::ORIGIN, self.window_size).contains(cursor)
    }

    /// Update the active drag's resolved target from app-root routing.
    pub fn update_drag_target(
        &mut self,
        target: DropTarget,
        cursor: Point,
        target_window: Option<window::Id>,
        cursor_window: window::Id,
    ) {
        if let Some(drag) = self.drag_state.as_mut() {
            drag.pointer_moved = true;
            drag.cursor = cursor;
            drag.target = target;
            drag.target_window = target_window;
            drag.cursor_window = Some(cursor_window);
        }
    }

    pub fn clear_cross_window_drop_preview(&mut self) {
        self.cross_window_drop_preview = None;
    }

    pub fn set_cross_window_drop_preview(&mut self, preview: CrossWindowDropPreview) {
        self.cross_window_drop_preview = Some(preview);
    }

    /// Remove the dragged tab from this window without placing it.
    pub fn extract_dragged_panel(&mut self, drag: &DragState) -> Option<Box<dyn Panel>> {
        let source = drag.source_location;
        let tab_idx = drag.source_tab;
        let panel = self.pane_state_mut(source).and_then(|ps| {
            if tab_idx < ps.tabs.len() {
                Some(ps.tabs.remove(tab_idx))
            } else {
                None
            }
        })?;
        if let Some(ps) = self.pane_state_mut(source)
            && ps.active >= ps.tabs.len()
            && !ps.tabs.is_empty()
        {
            ps.active = ps.tabs.len() - 1;
        }
        Some(panel)
    }

    /// Place a panel dragged from another window at `target`.
    pub fn apply_incoming_panel_drop(
        &mut self,
        panel: Box<dyn Panel>,
        target: DropTarget,
    ) -> Option<Box<dyn Panel>> {
        self.commit_drop(target, None, panel)
    }

    pub(crate) fn restore_dragged_panel(
        &mut self,
        source: PanelLocation,
        tab_idx: usize,
        panel: Box<dyn Panel>,
    ) {
        self.restore_tab_at_source(source, tab_idx, panel);
    }

    pub(crate) fn cleanup_after_drag_source(&mut self, location: PanelLocation) {
        self.cleanup_empty_source(location);
    }

    /// Finish a local tab drag, including tear-off to a new window.
    pub fn finish_local_tab_drag(&mut self, drag: DragState, stores: &mut AppStores) {
        self.apply_drop(drag, stores);
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

    /// Title of a tab at `location` / `tab`, for drag ghost rendering.
    pub fn tab_title(&self, location: PanelLocation, tab: usize) -> Option<String> {
        self.pane_state(location)
            .and_then(|pane| pane.tabs.get(tab))
            .map(|panel| panel.title().into_owned())
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
        if self.close_confirmation.is_some() {
            match message {
                WorkspaceMessage::ConfirmCloseDiscard { .. } => {
                    if let Some(confirm) = self.close_confirmation.take() {
                        self.close_tab_immediately(confirm.location, confirm.tab, stores);
                    }
                }
                WorkspaceMessage::ConfirmCloseCancel => {
                    self.close_confirmation = None;
                }
                WorkspaceMessage::WindowResized(size) => {
                    self.window_size = size;
                }
                _ => {}
            }
            return;
        }

        match message {
            WorkspaceMessage::PaneClicked(pane) => {
                self.focused = PanelLocation::Center(pane);
            }
            WorkspaceMessage::DockFocused(location) => {
                self.focused = location;
                if let PanelLocation::Dock(side) = location {
                    let dock = self.docks.get(side);
                    if dock.is_collapsed() && !dock.is_empty() {
                        self.docks.set_visibility(side, DockVisibility::Open);
                    }
                }
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
            WorkspaceMessage::Key(k) => {
                self.handle_key(k, stores);
            }
            WorkspaceMessage::Command(command) => self.apply_command(command, stores),
            WorkspaceMessage::ToggleTheme => {
                self.theme_mode = self.theme_mode.toggle();
                self.theme = OpenZoneTheme::from_mode(self.theme_mode);
            }
            WorkspaceMessage::NewWindow => {}
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
                if let Some(drag) = self.drag_state.as_ref() {
                    let (grid, pane_bounds, (rails, bodies)) = self.drag_geometry();
                    let target = crate::workspace::workspace_drag::compute_drop_target(
                        cursor,
                        grid,
                        &pane_bounds,
                        &rails,
                        &bodies,
                        &self.docks,
                        Some(drag),
                    );
                    if let Some(drag) = self.drag_state.as_mut() {
                        drag.pointer_moved = true;
                        drag.cursor = cursor;
                        drag.target = target;
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
            WorkspaceMessage::ConfirmCloseDiscard { location, tab } => {
                self.close_tab_immediately(location, tab, stores);
                self.close_confirmation = None;
            }
            WorkspaceMessage::ConfirmCloseCancel => {
                self.close_confirmation = None;
            }
        }
    }

    /// Pane and dock rectangles used for drag hit-testing and preview.
    fn drag_geometry(&self) -> (iced::Rectangle, Vec<PaneBounds>, DockRegions) {
        let grid =
            crate::workspace::workspace_drag::compute_grid_bounds(&self.docks, self.window_size);
        let pane_bounds = crate::workspace::workspace_drag::compute_pane_bounds(&self.panes, grid);
        let dock_regions =
            crate::workspace::workspace_drag::compute_dock_regions(&self.docks, self.window_size);
        (grid, pane_bounds, dock_regions)
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

        if let Some(ps) = self.pane_state_mut(source)
            && ps.active >= ps.tabs.len()
            && !ps.tabs.is_empty()
        {
            ps.active = ps.tabs.len() - 1;
        }

        if let Some(panel) = self.commit_drop(drag.target, Some((source, tab_idx)), panel) {
            self.restore_tab_at_source(source, tab_idx, panel);
            return;
        }

        self.cleanup_empty_source(source);
    }

    /// Place the dragged panel at the resolved target. Returns `Some(panel)`
    /// when the target cannot be applied so the caller can restore the source tab.
    pub(crate) fn commit_drop(
        &mut self,
        target: DropTarget,
        source: Option<(PanelLocation, usize)>,
        panel: Box<dyn Panel>,
    ) -> Option<Box<dyn Panel>> {
        match target {
            DropTarget::TabStrip(strip_target) => {
                let mut insert_at = strip_target.index;
                if let Some((source, tab_idx)) = source
                    && source == strip_target.location
                    && tab_idx < insert_at
                {
                    insert_at -= 1;
                }
                let Some(pane_state) = self.pane_state_mut(strip_target.location) else {
                    return Some(panel);
                };
                let insert_at = insert_at.min(pane_state.tabs.len());
                pane_state.tabs.insert(insert_at, panel);
                pane_state.active = insert_at;
                self.focused = strip_target.location;
                None
            }
            DropTarget::SplitPane(split_target) => {
                let axis = match split_target.direction {
                    Direction::Left | Direction::Right => Axis::Vertical,
                    Direction::Up | Direction::Down => Axis::Horizontal,
                };
                let mut new_pane_state = PaneState::new(vec![panel]);
                let Some((new_pane, _)) =
                    self.panes
                        .split(axis, split_target.pane, PaneState::empty())
                else {
                    return Some(new_pane_state.tabs.remove(0));
                };
                if let Some(pane_state) = self.panes.get_mut(new_pane) {
                    *pane_state = new_pane_state;
                }
                if matches!(split_target.direction, Direction::Left | Direction::Up) {
                    self.panes.swap(new_pane, split_target.pane);
                }
                let focused_pane = match split_target.direction {
                    Direction::Left | Direction::Up => split_target.pane,
                    Direction::Right | Direction::Down => new_pane,
                };
                self.focused = PanelLocation::Center(focused_pane);
                None
            }
            DropTarget::Dock(side) => {
                let dock = self.docks.get_mut(side);
                dock.tabs.tabs.push(panel);
                dock.tabs.active = dock.tabs.tabs.len() - 1;
                dock.visibility = DockVisibility::Open;
                self.focused = PanelLocation::Dock(side);
                None
            }
            DropTarget::None => {
                self.torn_off_panel = Some(panel);
                None
            }
        }
    }

    fn restore_tab_at_source(
        &mut self,
        source: PanelLocation,
        tab_idx: usize,
        panel: Box<dyn Panel>,
    ) {
        if let Some(pane_state) = self.pane_state_mut(source) {
            let restore_at = tab_idx.min(pane_state.tabs.len());
            pane_state.tabs.insert(restore_at, panel);
            pane_state.active = restore_at;
        }
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
                if let Some(factory) = self.scratch_factory
                    && let Some(ps) = self.panes.get_mut(pane)
                {
                    ps.tabs.push(factory());
                    ps.active = 0;
                    self.focused = PanelLocation::Center(pane);
                    return;
                }
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focused = PanelLocation::Center(sibling);
                }
            }
            PanelLocation::Dock(side) => {
                let was_focused = self.focused == PanelLocation::Dock(side);
                self.docks.get_mut(side).visibility = DockVisibility::Hidden;
                if was_focused {
                    self.return_focus_to_workbench();
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
            Command::OpenDock(side) => {
                self.docks.set_visibility(side, DockVisibility::Open);
                if self.docks.get(side).is_empty() {
                    if let Some(panel) = self.create_dock_default_surface(side, stores) {
                        let dock = self.docks.get_mut(side);
                        dock.tabs.tabs.push(panel);
                        dock.tabs.active = 0;
                        self.focused = PanelLocation::Dock(side);
                    }
                } else {
                    self.focused = PanelLocation::Dock(side);
                }
            }
            Command::CollapseDock(side) => {
                let was_focused = self.focused == PanelLocation::Dock(side);
                self.docks.set_visibility(side, DockVisibility::Collapsed);
                if was_focused {
                    self.return_focus_to_workbench();
                }
            }
            Command::HideDock(side) => {
                let was_focused = self.focused == PanelLocation::Dock(side);
                self.docks.set_visibility(side, DockVisibility::Hidden);
                if was_focused {
                    self.return_focus_to_workbench();
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

    /// Return focus to the first center pane.
    fn return_focus_to_workbench(&mut self) {
        let first_pane = self.panes.iter().next().map(|(pane, _)| *pane);
        if let Some(pane) = first_pane {
            self.focused = PanelLocation::Center(pane);
        }
    }

    /// Close the active tab in the focused pane or dock. The removed
    /// panel's [`Panel::on_close`] runs first so a store-backed panel
    /// (Counter) releases its slot before the box drops; an emptied
    /// center pane collapses out of the split tree, an emptied dock
    /// collapses to a rail.
    fn close_active_tab(&mut self, stores: &mut AppStores) {
        let focused = self.focused;
        let tab_index = self.pane_state(focused).map(|ps| ps.active);
        let close_req = self
            .pane_state(focused)
            .and_then(|ps| ps.active_panel())
            .map(|p| p.close_request());

        if let Some(tab_idx) = tab_index
            && let Some(CloseRequest::Confirm { message }) = close_req
        {
            self.close_confirmation = Some(CloseConfirmation {
                location: focused,
                tab: tab_idx,
                message,
            });
            return;
        }

        if let Some(tab_idx) = tab_index {
            self.close_tab_immediately(focused, tab_idx, stores);
        }
    }

    /// Close a tab immediately, performing any required pane cleanup/collapsing.
    pub fn close_tab_immediately(
        &mut self,
        location: PanelLocation,
        tab: usize,
        stores: &mut AppStores,
    ) {
        let removed = self.pane_state_mut(location).and_then(|p| p.close_tab(tab));

        let Some(mut panel) = removed else {
            return;
        };

        panel.on_close(stores);
        drop(panel);

        let now_empty = self
            .pane_state(location)
            .map(|pane_state| pane_state.is_empty())
            .unwrap_or(false);

        if !now_empty {
            return;
        }

        match location {
            PanelLocation::Center(pane) => {
                if let Some(factory) = self.scratch_factory
                    && let Some(ps) = self.panes.get_mut(pane)
                {
                    ps.tabs.push(factory());
                    ps.active = 0;
                    self.focused = PanelLocation::Center(pane);
                    return;
                }
                if let Some((_, sibling)) = self.panes.close(pane) {
                    self.focused = PanelLocation::Center(sibling);
                }
            }
            PanelLocation::Dock(side) => {
                let was_focused = self.focused == PanelLocation::Dock(side);
                self.docks.get_mut(side).visibility = DockVisibility::Hidden;
                if was_focused {
                    self.return_focus_to_workbench();
                }
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

        Subscription::batch(streams)
    }
}

pub(crate) fn tab_drag_subscription() -> Subscription<WorkspaceMessage> {
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
    use crate::workspace::workspace_command::{Chord, Mods};
    use crate::workspace::workspace_dock::DockVisibility;
    use crate::workspace::workspace_panel::{Panel, erase};
    use crate::workspace::workspace_stores::CounterId;

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
        let snapshot = workspace.panes.iter().next().unwrap().1.tabs[0]
            .snapshot(&stores)
            .expect("durable");
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
    fn open_dock_opens_and_focuses() {
        let (mut workspace, mut stores) = workspace_with_right_dock();

        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        assert!(workspace.docks.right.is_open());
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn hide_dock_returns_focus_to_workbench() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        workspace.apply_command(Command::HideDock(DockSide::Right), &mut stores);

        assert!(workspace.docks.right.is_hidden());
        assert!(matches!(workspace.focused, PanelLocation::Center(_)));
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
        workspace.docks.right.visibility = DockVisibility::Open;

        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        assert!(workspace.docks.right.tabs.is_empty());
        assert!(workspace.docks.right.is_hidden());
    }

    #[test]
    fn closing_last_tab_in_focused_dock_returns_focus_to_workbench() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.focused = PanelLocation::Dock(DockSide::Right);
        workspace.docks.right.visibility = DockVisibility::Open;

        workspace.apply_command(Command::CloseActiveTab, &mut stores);

        assert!(workspace.docks.right.is_hidden());
        assert!(matches!(workspace.focused, PanelLocation::Center(_)));
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
    fn close_confirmation_blocks_underlying_workspace_messages() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.close_confirmation = Some(CloseConfirmation {
            location,
            tab: 0,
            message: std::borrow::Cow::Borrowed("Discard changes to untitled?"),
        });

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
        let pane_state = workspace.panes.get(pane).unwrap();
        assert_eq!(pane_state.active, 0);
        assert_eq!(pane_state.len(), 2);
        assert!(workspace.close_confirmation.is_some());
    }

    #[test]
    fn discard_confirmation_closes_stored_tab_after_blocked_input() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.close_confirmation = Some(CloseConfirmation {
            location,
            tab: 0,
            message: std::borrow::Cow::Borrowed("Discard changes to untitled?"),
        });

        workspace.update(
            WorkspaceMessage::ConfirmCloseDiscard { location, tab: 1 },
            &mut stores,
        );

        let PanelLocation::Center(pane) = location else {
            panic!("expected center location");
        };
        let pane_state = workspace.panes.get(pane).unwrap();
        assert_eq!(pane_state.len(), 1);
        assert_eq!(pane_state.tabs[0].title(), "Text");
        assert!(workspace.close_confirmation.is_none());
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
        let workspace = Workspace::with_docks(center, docks, ThemeMode::Dark);

        stores.clock.tick();
        stores.clock.tick();

        assert_eq!(stores.clock.ticks(), 2);

        let center_pane = workspace.panes.iter().next().unwrap().1;
        let center_a = center_pane.tabs[0].snapshot(&stores).expect("durable");
        let center_b = center_pane.tabs[1].snapshot(&stores).expect("durable");
        let dock_a = workspace.docks.right.tabs.tabs[0]
            .snapshot(&stores)
            .expect("durable");
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

        let snapshot_a = workspace_a.panes.iter().next().unwrap().1.tabs[0]
            .snapshot(&stores)
            .expect("durable");
        let snapshot_b = workspace_b.panes.iter().next().unwrap().1.tabs[0]
            .snapshot(&stores)
            .expect("durable");

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

        let clock_a = workspace_a.panes.iter().next().unwrap().1.tabs[1]
            .snapshot(&stores)
            .expect("durable");
        let clock_b = workspace_b.panes.iter().next().unwrap().1.tabs[0]
            .snapshot(&stores)
            .expect("durable");

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
        assert!(workspace.docks.right.is_open());
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
    fn tab_drag_dropped_none_queues_torn_off_panel() {
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
        let torn = workspace
            .take_torn_off_panel()
            .expect("tear-off should queue the panel");
        assert_eq!(torn.title(), "Counter");

        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
        assert_eq!(workspace.panes.get(pane).unwrap().tabs[0].title(), "Text");
    }

    #[test]
    fn tab_drag_reorder_within_same_pane_adjusts_insert_index() {
        let mut stores = AppStores::new();
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
            Box::new(ClockPanel::new()),
        ];
        let mut workspace = Workspace::single_pane(PaneState::new(tabs), ThemeMode::Dark);
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        {
            let drag = workspace.drag_state.as_mut().unwrap();
            drag.target = DropTarget::TabStrip(TabStripTarget { location, index: 2 });
            drag.pointer_moved = true;
        }

        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        let titles: Vec<_> = workspace
            .panes
            .get(pane)
            .unwrap()
            .tabs
            .iter()
            .map(|t| t.title())
            .collect();
        assert_eq!(titles, vec!["Text", "Counter", "Clock"]);
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
    fn tab_drag_split_failure_restores_panel() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.apply_command(Command::SplitFocused, &mut stores);
        let panes: Vec<pane_grid::Pane> = workspace.panes.iter().map(|(p, _)| *p).collect();
        let stale_pane = panes[1];
        workspace.panes.close(stale_pane);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );

        workspace.drag_state.as_mut().unwrap().target = DropTarget::SplitPane(SplitPaneTarget {
            pane: stale_pane,
            direction: Direction::Right,
        });
        workspace.drag_state.as_mut().unwrap().pointer_moved = true;

        workspace.update(WorkspaceMessage::TabDragDropped, &mut stores);

        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 2);
        assert_eq!(
            workspace.panes.get(pane).unwrap().tabs[0].title(),
            "Counter"
        );
    }

    #[test]
    fn extract_dragged_panel_removes_source_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);
        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut stores,
        );
        let drag = workspace.drag_state.as_ref().unwrap().clone();
        let panel = workspace
            .extract_dragged_panel(&drag)
            .expect("panel should be extracted");
        assert_eq!(panel.title(), "Counter");
        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        assert_eq!(workspace.panes.get(pane).unwrap().len(), 1);
    }

    #[test]
    fn incoming_panel_drop_inserts_into_target_tab_strip() {
        let mut stores = AppStores::new();
        let mut target = Workspace::single_pane(
            PaneState::new(vec![Box::new(TextPanel::new())]),
            ThemeMode::Dark,
        );
        let location = only_center_location(&target);
        let panel = Box::new(CounterPanel::new(&mut stores)) as Box<dyn Panel>;
        assert!(
            target
                .apply_incoming_panel_drop(
                    panel,
                    DropTarget::TabStrip(TabStripTarget { location, index: 1 }),
                )
                .is_none()
        );
        let PanelLocation::Center(pane) = location else {
            panic!("expected center");
        };
        let titles: Vec<_> = target
            .panes
            .get(pane)
            .unwrap()
            .tabs
            .iter()
            .map(|t| t.title())
            .collect();
        assert_eq!(titles, vec!["Text", "Counter"]);
    }

    #[test]
    fn incoming_panel_drop_returns_panel_for_missing_pane() {
        let mut stores = AppStores::new();
        let mut target = Workspace::single_pane(
            PaneState::new(vec![Box::new(TextPanel::new())]),
            ThemeMode::Dark,
        );
        let pane0 = *target.panes.iter().next().unwrap().0;
        let (missing_pane, _) = target
            .panes
            .split(Axis::Vertical, pane0, PaneState::empty())
            .expect("split for stale pane handle");
        target.panes.close(missing_pane);
        let panel = Box::new(CounterPanel::new(&mut stores)) as Box<dyn Panel>;
        let failed = target.apply_incoming_panel_drop(
            panel,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(missing_pane),
                index: 0,
            }),
        );
        assert_eq!(failed.unwrap().title(), "Counter");
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

    // -- Default dock surface factory tests ------------------------------------------

    fn workspace_with_empty_docks() -> (Workspace, AppStores) {
        let stores = AppStores::new();
        let center = PaneState::new(vec![Box::new(TextPanel::new())]);
        let workspace = Workspace::with_docks(center, Docks::empty(), ThemeMode::Dark);
        (workspace, stores)
    }

    #[test]
    fn open_empty_dock_without_factory_leaves_it_empty() {
        let (mut workspace, mut stores) = workspace_with_empty_docks();
        // No factory registered — Right dock should open but remain empty.
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        assert!(workspace.docks.right.is_open());
        assert!(workspace.docks.right.is_empty());
        // Focus does NOT move to an empty dock with no factory.
        assert!(workspace.focused.is_center());
    }

    #[test]
    fn open_empty_dock_with_factory_populates_default_surface() {
        let (mut workspace, mut stores) = workspace_with_empty_docks();
        workspace.set_dock_factory(DockSide::Right, |_stores| Box::new(TextPanel::new()));

        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        assert!(workspace.docks.right.is_open());
        assert_eq!(workspace.docks.right.tabs.len(), 1);
        assert_eq!(workspace.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn has_dock_factory_reports_factory_presence() {
        let (mut workspace, _stores) = workspace_with_empty_docks();
        assert!(!workspace.has_dock_factory(DockSide::Left));

        workspace.set_dock_factory(DockSide::Left, |_stores| Box::new(TextPanel::new()));
        assert!(workspace.has_dock_factory(DockSide::Left));
        assert!(!workspace.has_dock_factory(DockSide::Right));
    }

    #[test]
    fn populate_empty_docks_fills_open_empty_docks_from_factories() {
        let (mut workspace, mut stores) = workspace_with_empty_docks();
        // Set up: Left dock open but empty, Right dock open with factory.
        workspace.docks.left.visibility = DockVisibility::Open;
        workspace.docks.right.visibility = DockVisibility::Open;
        workspace.set_dock_factory(DockSide::Left, |_stores| Box::new(TextPanel::new()));
        workspace.set_dock_factory(DockSide::Right, |_stores| Box::new(ClockPanel::new()));

        workspace.populate_empty_docks(&mut stores);

        assert_eq!(workspace.docks.left.tabs.len(), 1);
        assert_eq!(workspace.docks.right.tabs.len(), 1);
        // Bottom was not open, stays empty.
        assert!(workspace.docks.bottom.is_empty());
    }

    #[test]
    fn populate_empty_docks_hides_open_empty_docks_without_factory() {
        let (mut workspace, mut stores) = workspace_with_empty_docks();
        workspace.docks.right.visibility = DockVisibility::Open;
        // No factory for Right.

        workspace.populate_empty_docks(&mut stores);

        assert!(workspace.docks.right.is_hidden());
    }

    #[test]
    fn open_dock_with_existing_tabs_ignores_factory() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.set_dock_factory(DockSide::Right, |_stores| Box::new(TextPanel::new()));

        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        // The dock already had a ClockPanel tab; factory must not add another.
        assert_eq!(workspace.docks.right.tabs.len(), 1);
        assert!(workspace.docks.right.is_open());
    }
}
