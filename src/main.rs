//! OpenZone Rustaceans — composition root.
//!
//! This binary composes internal modules, chooses infrastructure, and
//! launches the Iced runtime as a multi-window **daemon**. It is the one
//! place that names concrete feature panels and wires them into the
//! shell registry; the workspace shell itself never depends on a
//! concrete feature.
//!
//! Window model: the app boots showing the onboarding window. When the
//! user presses *Enter OpenZone*, the workspace opens in its own,
//! separate OS window and the onboarding window closes (clean handoff).
//! Additional workspace windows can be opened from the title bar or with
//! `Cmd+Shift+N`. Onboarding is never overridden in place.

mod features;
mod shared;
mod workspace;

use std::collections::HashMap;
use std::sync::Arc;

use iced::event::{self, Event};
use iced::{Element, Size, Subscription, Task, Theme, window};

use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
use crate::features::onboarding::onboarding_file_persistence::FileOnboardingPersistence;
use crate::features::onboarding::onboarding_memory_persistence::InMemoryOnboardingPersistence;
use crate::features::onboarding::{
    OnboardingMessage, OnboardingOutcome, OnboardingPersistence, OnboardingState, mark_completed,
    view as onboarding_view,
};
use crate::shared::design::ThemeMode;
use crate::workspace::workspace_state::tab_drag_subscription;
use crate::workspace::workspace_stores::CounterId;
use crate::workspace::{
    AppStores, Chord, CrossWindowDropPreview, Docks, DragState, DropTarget, FileLayoutStore,
    LayoutStore, Mods, PaneState, Panel, PanelKind, PanelRegistry, Workspace, WorkspaceMessage,
    chord_from_keyboard_event,
};

fn main() -> iced::Result {
    let persistence = load_persistence();
    let layout_store = load_layout_store();
    let theme_mode = ThemeMode::Dark;

    iced::daemon(
        move || {
            // Boot opens the onboarding window. The daemon owns no window
            // until we ask for one, so the first `window::open` is what
            // makes anything visible.
            let (onboarding_window, open) = window::open(onboarding_window_settings());
            let app = OpenZone {
                onboarding: Some(OnboardingState::new(persistence.clone(), theme_mode)),
                onboarding_window: Some(onboarding_window),
                workspaces: HashMap::new(),
                stores: AppStores::new(),
                persistence: persistence.clone(),
                registry: build_registry(),
                layout_store: layout_store.clone(),
                theme_mode,
            };
            (app, open.discard())
        },
        OpenZone::update,
        OpenZone::view,
    )
    .title(OpenZone::title)
    .subscription(OpenZone::subscription)
    .theme(OpenZone::theme)
    .run()
}

/// The top-level multi-window application state.
///
/// Global state stays thin: panel registry, layout store, and app-root
/// [`AppStores`]. Each workspace OS window owns a fat [`Workspace`]
/// (pane tree, docks, focus) keyed by [`window::Id`].
struct OpenZone {
    onboarding: Option<OnboardingState>,
    onboarding_window: Option<window::Id>,
    workspaces: HashMap<window::Id, Workspace>,
    /// App-root domain stores shared across every workspace window.
    stores: AppStores,
    persistence: Arc<dyn OnboardingPersistence>,
    /// The composition seam the shell uses to rehydrate panels from
    /// persisted handles. It knows panel kinds, not concrete types.
    registry: PanelRegistry,
    /// Layout persistence: a saved snapshot is restored when entering the
    /// first workspace window and written when the last one closes.
    layout_store: Arc<dyn LayoutStore>,
    theme_mode: ThemeMode,
}

impl OpenZone {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Onboarding(message) => self.update_onboarding(message),
            Message::Workspace { window, message } => {
                if matches!(message, WorkspaceMessage::NewWindow) {
                    return self.open_additional_workspace();
                }
                if let Some(source) = self.tab_drag_source() {
                    match message {
                        WorkspaceMessage::CursorMoved(cursor) => {
                            self.route_tab_drag_cursor(source, window, cursor);
                            return Task::none();
                        }
                        WorkspaceMessage::TabDragDropped => {
                            return self.handle_tab_drag_dropped(source);
                        }
                        _ => {}
                    }
                }
                if let Some(workspace) = self.workspaces.get_mut(&window) {
                    workspace.update(message, &mut self.stores);
                    if let Some(panel) = workspace.take_torn_off_panel() {
                        return self.open_workspace_with_panel(panel);
                    }
                }
                Task::none()
            }
            Message::Key { window, chord } => {
                if let Some(workspace) = self.workspaces.get_mut(&window) {
                    workspace.update(WorkspaceMessage::Key(chord), &mut self.stores);
                }
                Task::none()
            }
            Message::OpenWorkspace => self.open_additional_workspace(),
            Message::ClockTick => {
                self.stores.clock.tick();
                Task::none()
            }
            Message::WindowClosed(id) => self.handle_window_closed(id),
        }
    }

    /// Fold an onboarding message; on completion, hand off to a new
    /// workspace window.
    fn update_onboarding(&mut self, message: OnboardingMessage) -> Task<Message> {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return Task::none();
        };

        match onboarding.update(message) {
            OnboardingOutcome::Completed | OnboardingOutcome::Skipped => {
                let _ = mark_completed(&self.persistence);
                self.enter_workspace()
            }
            _ => Task::none(),
        }
    }

    /// Open the first workspace window and close onboarding.
    fn enter_workspace(&mut self) -> Task<Message> {
        let settings = workspace_window_settings();
        let size = settings.size;
        let (workspace_window, open) = window::open(settings);
        let workspace = self.restore_or_build_workspace().with_window_size(size);
        self.workspaces.insert(workspace_window, workspace);

        let close = match self.onboarding_window.take() {
            Some(onboarding_window) => {
                self.onboarding = None;
                window::close(onboarding_window)
            }
            None => Task::none(),
        };

        Task::batch([open.discard(), close])
    }

    /// Open another workspace window with an independent layout that
    /// still observes the same app-root Counter and Clock stores.
    fn open_additional_workspace(&mut self) -> Task<Message> {
        let settings = workspace_window_settings();
        let size = settings.size;
        let (workspace_window, open) = window::open(settings);
        self.workspaces.insert(
            workspace_window,
            build_secondary_workspace(&mut self.stores, self.theme_mode).with_window_size(size),
        );
        open.discard()
    }

    /// Open a workspace window hosting a single tab torn off from another.
    fn open_workspace_with_panel(&mut self, panel: Box<dyn Panel>) -> Task<Message> {
        let settings = workspace_window_settings();
        let size = settings.size;
        let (workspace_window, open) = window::open(settings);
        self.workspaces.insert(
            workspace_window,
            Workspace::single_pane(PaneState::new(vec![panel]), self.theme_mode)
                .with_window_size(size),
        );
        open.discard()
    }

    /// Restore the workspace from a persisted layout snapshot, or build
    /// the seeded default when nothing valid is stored.
    fn restore_or_build_workspace(&mut self) -> Workspace {
        match self.layout_store.load() {
            Some(snapshot) => {
                workspace::restore(&snapshot, &self.registry, &mut self.stores, self.theme_mode)
            }
            None => build_workspace(&mut self.stores, self.theme_mode),
        }
    }

    fn tab_drag_source(&self) -> Option<window::Id> {
        self.workspaces
            .iter()
            .find_map(|(id, workspace)| workspace.is_tab_drag_active().then_some(*id))
    }

    fn route_tab_drag_cursor(
        &mut self,
        source: window::Id,
        event_window: window::Id,
        cursor: iced::Point,
    ) {
        for workspace in self.workspaces.values_mut() {
            workspace.clear_cross_window_drop_preview();
        }

        let drag = self
            .workspaces
            .get(&source)
            .and_then(|workspace| workspace.drag_state.as_ref().cloned());
        let Some(drag) = drag else {
            return;
        };

        let (target_window, target) = if let Some(workspace) = self.workspaces.get(&event_window)
            && workspace.contains_client_point(cursor)
        {
            (
                Some(event_window),
                workspace.resolve_drop_at(cursor, Some(&drag)),
            )
        } else {
            (None, DropTarget::None)
        };

        if let Some(workspace) = self.workspaces.get_mut(&source) {
            workspace.update_drag_target(target, cursor, target_window, event_window);
        }

        if let (Some(target_window), false) = (target_window, matches!(target, DropTarget::None))
            && target_window != source
        {
            let preview = CrossWindowDropPreview {
                drag: drag.clone(),
                target,
                cursor,
            };
            if let Some(workspace) = self.workspaces.get_mut(&target_window) {
                workspace.set_cross_window_drop_preview(preview);
            }
        }
    }

    fn resolve_drag_drop_target(&self, drag: &DragState) -> (Option<window::Id>, DropTarget) {
        let Some(event_window) = drag.cursor_window else {
            return (drag.target_window, drag.target);
        };

        let Some(workspace) = self.workspaces.get(&event_window) else {
            return (None, DropTarget::None);
        };

        if !workspace.contains_client_point(drag.cursor) {
            return (None, DropTarget::None);
        }

        let target = workspace.resolve_drop_at(drag.cursor, Some(drag));
        (Some(event_window), target)
    }

    fn handle_tab_drag_dropped(&mut self, source: window::Id) -> Task<Message> {
        let drag = match self
            .workspaces
            .get_mut(&source)
            .and_then(|w| w.drag_state.take())
        {
            Some(drag) => drag,
            None => return Task::none(),
        };

        for workspace in self.workspaces.values_mut() {
            workspace.clear_cross_window_drop_preview();
        }

        if matches!(drag.target, DropTarget::None) && !drag.pointer_moved {
            if let Some(workspace) = self.workspaces.get_mut(&source) {
                workspace.finish_local_tab_drag(drag, &mut self.stores);
            }
            return Task::none();
        }

        let (target_window, target) = self.resolve_drag_drop_target(&drag);
        let mut drag = drag;
        drag.target = target;
        drag.target_window = target_window;

        let target_window = drag.target_window.unwrap_or(source);

        if target_window != source && !matches!(drag.target, DropTarget::None) {
            let source_location = drag.source_location;
            let source_tab = drag.source_tab;
            let target = drag.target;

            let panel = match self.workspaces.get_mut(&source) {
                Some(workspace) => workspace.extract_dragged_panel(&drag),
                None => None,
            };
            let Some(panel) = panel else {
                return Task::none();
            };

            let failed = self
                .workspaces
                .get_mut(&target_window)
                .and_then(|workspace| workspace.apply_incoming_panel_drop(panel, target));

            if let (Some(workspace), Some(panel)) = (self.workspaces.get_mut(&source), failed) {
                workspace.restore_dragged_panel(source_location, source_tab, panel);
            } else if let Some(workspace) = self.workspaces.get_mut(&source) {
                workspace.cleanup_after_drag_source(source_location);
            }

            return Task::none();
        }

        if let Some(workspace) = self.workspaces.get_mut(&source) {
            workspace.finish_local_tab_drag(drag, &mut self.stores);
            if let Some(panel) = workspace.take_torn_off_panel() {
                return self.open_workspace_with_panel(panel);
            }
        }

        Task::none()
    }

    /// React to a window the user (or our own handoff) closed. When no
    /// windows remain, the daemon has nothing left to show, so exit.
    fn handle_window_closed(&mut self, id: window::Id) -> Task<Message> {
        if self.onboarding_window == Some(id) {
            self.onboarding_window = None;
            self.onboarding = None;
        }

        if let Some(workspace) = self.workspaces.remove(&id)
            && self.workspaces.is_empty()
        {
            let snapshot = workspace::capture(&workspace, &self.stores);
            let _ = self.layout_store.save(&snapshot);
        }

        if self.onboarding_window.is_none() && self.workspaces.is_empty() {
            iced::exit()
        } else {
            Task::none()
        }
    }

    fn view(&self, window: window::Id) -> Element<'_, Message> {
        if self.onboarding_window == Some(window)
            && let Some(onboarding) = &self.onboarding
        {
            return onboarding_view(onboarding).map(Message::Onboarding);
        }

        if let Some(workspace) = self.workspaces.get(&window) {
            return workspace::workspace_view::view(workspace, &self.stores)
                .map(move |message| Message::Workspace { window, message });
        }

        iced::widget::container(iced::widget::Space::new())
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut streams: Vec<Subscription<Message>> = Vec::new();

        if let Some(onboarding) = &self.onboarding {
            streams.push(onboarding.subscription().map(Message::Onboarding));
        }

        let workspace_window_ids: Vec<window::Id> = self.workspaces.keys().copied().collect();
        for window_id in workspace_window_ids {
            let workspace = &self.workspaces[&window_id];
            streams.push(
                workspace
                    .subscription()
                    .with(window_id)
                    .map(|(window, message)| Message::Workspace { window, message }),
            );
            streams.push(window::resize_events().with(window_id).filter_map(
                |(target_window, (id, size))| {
                    (id == target_window).then(|| Message::Workspace {
                        window: target_window,
                        message: WorkspaceMessage::WindowResized(size),
                    })
                },
            ));
        }

        if !self.workspaces.is_empty() {
            streams.push(event::listen_with(workspace_key_event));
        }

        if self.tab_drag_source().is_some() {
            for window_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
                streams.push(
                    tab_drag_subscription()
                        .with(window_id)
                        .map(move |(window, message)| Message::Workspace { window, message }),
                );
            }
        }

        if self.any_workspace_has_clock() {
            streams.push(
                iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::ClockTick),
            );
        }

        streams.push(window::close_events().map(Message::WindowClosed));

        Subscription::batch(streams)
    }

    fn any_workspace_has_clock(&self) -> bool {
        self.workspaces
            .values()
            .any(|workspace| workspace.has_clock_panel())
    }

    fn title(&self, _window: window::Id) -> String {
        String::from("OpenZone")
    }

    fn theme(&self, window: window::Id) -> Theme {
        let mode = if let Some(workspace) = self.workspaces.get(&window) {
            workspace.theme_mode
        } else if let Some(onboarding) = &self.onboarding {
            onboarding.theme_mode
        } else {
            self.theme_mode
        };

        match mode {
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::Light => Theme::Light,
        }
    }
}

/// Top-level message: feature messages tagged by origin, plus the
/// window-lifecycle and per-window keyboard events the daemon needs.
#[derive(Debug, Clone)]
enum Message {
    Onboarding(OnboardingMessage),
    Workspace {
        window: window::Id,
        message: WorkspaceMessage,
    },
    /// A key chord, tagged with the window that produced it so the
    /// reducer can route it only to that workspace window.
    Key {
        window: window::Id,
        chord: Chord,
    },
    /// Open another workspace window (title bar or `Cmd+Shift+N`).
    OpenWorkspace,
    /// One tick from the single app-level Clock subscription.
    ClockTick,
    WindowClosed(window::Id),
}

/// Convert a raw window event into a tagged key chord message. Must be a
/// plain `fn` (no captures) because `event::listen_with` takes a function
/// pointer; the window id is carried in the message instead of captured.
fn workspace_key_event(
    event: Event,
    _status: event::Status,
    window: window::Id,
) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard) => {
            if let Some(chord) = chord_from_keyboard_event(&keyboard) {
                if chord == Chord::ch('n', Mods::CMD.with_shift()) {
                    return Some(Message::OpenWorkspace);
                }
                return Some(Message::Key { window, chord });
            }
            None
        }
        _ => None,
    }
}

fn onboarding_window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(960.0, 680.0),
        ..window::Settings::default()
    }
}

fn workspace_window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(1100.0, 760.0),
        ..window::Settings::default()
    }
}

fn load_persistence() -> Arc<dyn OnboardingPersistence> {
    match FileOnboardingPersistence::from_project_dirs() {
        Ok(store) => Arc::new(store),
        Err(_) => Arc::new(InMemoryOnboardingPersistence::new()),
    }
}

fn load_layout_store() -> Arc<dyn LayoutStore> {
    match FileLayoutStore::from_project_dirs() {
        Ok(store) => Arc::new(store),
        Err(_) => Arc::new(NoopLayoutStore),
    }
}

struct NoopLayoutStore;

impl LayoutStore for NoopLayoutStore {
    fn load(&self) -> Option<workspace::LayoutSnapshot> {
        None
    }

    fn save(
        &self,
        _snapshot: &workspace::LayoutSnapshot,
    ) -> Result<(), workspace::LayoutStoreError> {
        Ok(())
    }
}

fn build_registry() -> PanelRegistry {
    let mut registry = PanelRegistry::new();
    registry
        .register(PanelKind::Counter, |snapshot, stores| {
            Box::new(CounterPanel::from_snapshot(snapshot, stores))
        })
        .register(PanelKind::Text, |snapshot, _stores| {
            Box::new(TextPanel::from_snapshot(snapshot))
        })
        .register(PanelKind::Clock, |snapshot, stores| {
            Box::new(ClockPanel::from_snapshot(snapshot, stores))
        });
    registry
}

/// Build the primary workspace layout: one center pane hosting the dummy
/// panels as tabs, with one dummy panel per edge dock.
fn build_workspace(stores: &mut AppStores, theme_mode: ThemeMode) -> Workspace {
    let center_tabs: Vec<Box<dyn Panel>> = vec![
        Box::new(CounterPanel::new(stores)),
        Box::new(TextPanel::new()),
        Box::new(ClockPanel::new()),
    ];

    let docks = Docks::new(
        PaneState::new(vec![Box::new(ClockPanel::new())]),
        PaneState::new(vec![Box::new(CounterPanel::new(stores))]),
        PaneState::new(vec![Box::new(TextPanel::new())]),
    );

    Workspace::with_docks(PaneState::new(center_tabs), docks, theme_mode)
}

/// Build an additional workspace window with its own layout but shared
/// store-backed Counter and Clock panels.
fn build_secondary_workspace(stores: &mut AppStores, theme_mode: ThemeMode) -> Workspace {
    let shared_counter = shared_counter_id(stores);
    let center_tabs: Vec<Box<dyn Panel>> = vec![
        Box::new(CounterPanel::with_id(shared_counter)),
        Box::new(ClockPanel::new()),
    ];

    Workspace::single_pane(PaneState::new(center_tabs), theme_mode)
}

/// Reuse the first live counter slot when opening another window so both
/// windows observe the same count after a store mutation.
///
/// Assumes slot 0 belongs to the primary workspace's center counter
/// (monotonically allocated from 0; [`build_workspace`] creates the
/// center counter before any dock counters). If no slot is live yet,
/// allocates a fresh one.
fn shared_counter_id(stores: &mut AppStores) -> CounterId {
    if stores.counter.count(0).is_some() {
        0
    } else {
        stores.counter.create()
    }
}

#[cfg(test)]
mod cross_window_tab_drop_tests {
    use super::*;
    use crate::workspace::workspace_location::PanelLocation;
    use crate::workspace::{DropTarget, TabStripTarget};

    fn two_workspace_app() -> (OpenZone, window::Id, window::Id) {
        let mut stores = AppStores::new();
        let window_a = window::Id::unique();
        let window_b = window::Id::unique();
        let workspace_a = Workspace::single_pane(
            PaneState::new(vec![
                Box::new(CounterPanel::new(&mut stores)),
                Box::new(TextPanel::new()),
            ]),
            ThemeMode::Dark,
        );
        let workspace_b = Workspace::single_pane(
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            ThemeMode::Dark,
        );
        let mut workspaces = HashMap::new();
        workspaces.insert(window_a, workspace_a);
        workspaces.insert(window_b, workspace_b);
        let app = OpenZone {
            onboarding: None,
            onboarding_window: None,
            workspaces,
            stores,
            persistence: Arc::new(InMemoryOnboardingPersistence::new()),
            registry: build_registry(),
            layout_store: Arc::new(NoopLayoutStore),
            theme_mode: ThemeMode::Dark,
        };
        (app, window_a, window_b)
    }

    fn center_location(workspace: &Workspace) -> PanelLocation {
        let pane = workspace.panes.iter().next().unwrap().0;
        PanelLocation::Center(*pane)
    }

    #[test]
    fn route_tab_drag_cursor_targets_other_window_tab_strip() {
        let (mut app, window_a, window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let pane_b = *app.workspaces[&window_b].panes.iter().next().unwrap().0;
        let grid = crate::workspace::workspace_drag::compute_grid_bounds(
            &app.workspaces[&window_b].docks,
            app.workspaces[&window_b].window_size,
        );
        let pane_bounds = crate::workspace::workspace_drag::compute_pane_bounds(
            &app.workspaces[&window_b].panes,
            grid,
        );
        let pb = &pane_bounds[0];
        let cursor = iced::Point::new(pb.tab_strip.x + 20.0, pb.tab_strip.y + 4.0);
        app.route_tab_drag_cursor(window_a, window_b, cursor);

        let drag = app.workspaces[&window_a].drag_state.as_ref().unwrap();
        assert_eq!(drag.target_window, Some(window_b));
        assert_eq!(
            drag.target,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pane_b),
                index: 1,
            })
        );
        assert!(
            app.workspaces[&window_b]
                .cross_window_drop_preview
                .is_some()
        );
    }

    #[test]
    fn handle_tab_drag_dropped_transfers_tab_to_other_window() {
        let (mut app, window_a, window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let pane_b = *app.workspaces[&window_b].panes.iter().next().unwrap().0;
        {
            let drag = app
                .workspaces
                .get_mut(&window_a)
                .unwrap()
                .drag_state
                .as_mut()
                .unwrap();
            drag.target = DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pane_b),
                index: 1,
            });
            drag.target_window = Some(window_b);
            drag.pointer_moved = true;
        }

        let _task = app.handle_tab_drag_dropped(window_a);

        let titles_a: Vec<_> = app.workspaces[&window_a]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_a, vec!["Text"]);

        let titles_b: Vec<_> = app.workspaces[&window_b]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_b, vec!["Clock", "Counter"]);
    }

    #[test]
    fn handle_tab_drag_dropped_click_without_move_clears_drag_state() {
        let (mut app, window_a, _window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let _task = app.handle_tab_drag_dropped(window_a);

        assert!(app.workspaces[&window_a].drag_state.is_none());
        let titles: Vec<_> = app.workspaces[&window_a]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles, vec!["Counter", "Text"]);
    }

    #[test]
    fn route_tab_drag_cursor_clears_target_when_cursor_leaves_window() {
        let (mut app, window_a, window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let pane_b = *app.workspaces[&window_b].panes.iter().next().unwrap().0;
        let grid = crate::workspace::workspace_drag::compute_grid_bounds(
            &app.workspaces[&window_b].docks,
            app.workspaces[&window_b].window_size,
        );
        let pane_bounds = crate::workspace::workspace_drag::compute_pane_bounds(
            &app.workspaces[&window_b].panes,
            grid,
        );
        let pb = &pane_bounds[0];
        let inside = iced::Point::new(pb.tab_strip.x + 20.0, pb.tab_strip.y + 4.0);
        app.route_tab_drag_cursor(window_a, window_b, inside);
        assert_eq!(
            app.workspaces[&window_a]
                .drag_state
                .as_ref()
                .unwrap()
                .target_window,
            Some(window_b)
        );

        let outside = iced::Point::new(
            app.workspaces[&window_b].window_size.width + 40.0,
            app.workspaces[&window_b].window_size.height + 40.0,
        );
        app.route_tab_drag_cursor(window_a, window_b, outside);

        let drag = app.workspaces[&window_a].drag_state.as_ref().unwrap();
        assert_eq!(drag.target_window, None);
        assert_eq!(drag.target, DropTarget::None);
        assert!(
            app.workspaces[&window_b]
                .cross_window_drop_preview
                .is_none()
        );
        let _ = pane_b;
    }

    #[test]
    fn handle_tab_drag_dropped_revalidates_stale_target_from_last_cursor() {
        let (mut app, window_a, window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let pane_b = *app.workspaces[&window_b].panes.iter().next().unwrap().0;
        let outside_b = {
            let size = app.workspaces[&window_b].window_size;
            iced::Point::new(size.width + 100.0, size.height + 100.0)
        };
        {
            let drag = app
                .workspaces
                .get_mut(&window_a)
                .unwrap()
                .drag_state
                .as_mut()
                .unwrap();
            drag.target = DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pane_b),
                index: 1,
            });
            drag.target_window = Some(window_b);
            drag.pointer_moved = true;
            drag.cursor_window = Some(window_b);
            drag.cursor = outside_b;
        }

        let _task = app.handle_tab_drag_dropped(window_a);

        let titles_a: Vec<_> = app.workspaces[&window_a]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_a, vec!["Text"]);
        let titles_b: Vec<_> = app.workspaces[&window_b]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_b, vec!["Clock"]);
    }

    #[test]
    fn handle_tab_drag_dropped_cross_window_dock() {
        let (mut app, window_a, window_b) = two_workspace_app();
        {
            let workspace_b = app.workspaces.get_mut(&window_b).unwrap();
            workspace_b.docks.left.tabs = PaneState::new(vec![Box::new(TextPanel::new())]);
            workspace_b.docks.left.open = true;
        }
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );

        let grid = crate::workspace::workspace_drag::compute_grid_bounds(
            &app.workspaces[&window_b].docks,
            app.workspaces[&window_b].window_size,
        );
        let (_, bodies) = crate::workspace::workspace_drag::compute_dock_regions(
            &app.workspaces[&window_b].docks,
            app.workspaces[&window_b].window_size,
        );
        let (_, left_body) = bodies
            .iter()
            .find(|(side, _)| *side == crate::workspace::DockSide::Left)
            .expect("left dock body");
        let cursor = iced::Point::new(
            left_body.x + 12.0,
            left_body.y + crate::workspace::workspace_layout_metrics::tab_strip_height() / 2.0,
        );
        app.route_tab_drag_cursor(window_a, window_b, cursor);

        let _task = app.handle_tab_drag_dropped(window_a);

        assert_eq!(
            app.workspaces[&window_b].docks.left.tabs.tabs[0].title(),
            "Counter"
        );
        assert_eq!(
            app.workspaces[&window_a]
                .panes
                .iter()
                .next()
                .unwrap()
                .1
                .tabs
                .iter()
                .map(|p| p.title())
                .collect::<Vec<_>>(),
            vec!["Text"]
        );
        let _ = grid;
    }

    #[test]
    fn handle_tab_drag_dropped_outside_queues_tear_off_panel() {
        let (mut app, window_a, _window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );
        let outside_a = {
            let size = app.workspaces[&window_a].window_size;
            iced::Point::new(size.width + 50.0, size.height + 50.0)
        };
        {
            let drag = app
                .workspaces
                .get_mut(&window_a)
                .unwrap()
                .drag_state
                .as_mut()
                .unwrap();
            drag.target = DropTarget::None;
            drag.target_window = None;
            drag.pointer_moved = true;
            drag.cursor_window = Some(window_a);
            drag.cursor = outside_a;
        }

        let _task = app.handle_tab_drag_dropped(window_a);

        let titles_a: Vec<_> = app.workspaces[&window_a]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_a, vec!["Text"]);
        assert_eq!(app.workspaces.len(), 3);
    }

    #[test]
    fn handle_tab_drag_dropped_outside_all_windows_still_tears_off() {
        let (mut app, window_a, _window_b) = two_workspace_app();
        let source_location = center_location(&app.workspaces[&window_a]);
        app.workspaces.get_mut(&window_a).unwrap().update(
            WorkspaceMessage::TabDragStarted {
                location: source_location,
                tab: 0,
            },
            &mut app.stores,
        );
        let outside_a = {
            let size = app.workspaces[&window_a].window_size;
            iced::Point::new(size.width + 50.0, size.height + 50.0)
        };
        {
            let drag = app
                .workspaces
                .get_mut(&window_a)
                .unwrap()
                .drag_state
                .as_mut()
                .unwrap();
            drag.target = DropTarget::None;
            drag.target_window = None;
            drag.pointer_moved = true;
            drag.cursor_window = Some(window_a);
            drag.cursor = outside_a;
        }

        let _task = app.handle_tab_drag_dropped(window_a);

        let titles_a: Vec<_> = app.workspaces[&window_a]
            .panes
            .iter()
            .next()
            .unwrap()
            .1
            .tabs
            .iter()
            .map(|p| p.title())
            .collect();
        assert_eq!(titles_a, vec!["Text"]);
        assert!(
            app.workspaces
                .get_mut(&window_a)
                .unwrap()
                .take_torn_off_panel()
                .is_none()
        );
    }
}
