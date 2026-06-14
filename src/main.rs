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

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iced::event::{self, Event};
use iced::{Element, Size, Subscription, Task, Theme, window};

use crate::features::ScratchPanel;
#[cfg(test)]
use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
use crate::features::onboarding::onboarding_file_persistence::FileOnboardingPersistence;
use crate::features::onboarding::onboarding_memory_persistence::InMemoryOnboardingPersistence;
use crate::features::onboarding::{
    OnboardingMessage, OnboardingOutcome, OnboardingPersistence, OnboardingState, mark_completed,
    view as onboarding_view,
};
use crate::shared::design::ThemeMode;
#[cfg(test)]
use crate::workspace::DockVisibility;
use crate::workspace::workspace_command_palette::default_command_items;
use crate::workspace::workspace_state::tab_drag_subscription;
use crate::workspace::{
    AppStores, Chord, CrossWindowDropPreview, DirtyPanel, DockSide, DragState, DropTarget,
    FileLayoutStore, LayoutStore, Mods, PaneState, Panel, PanelKind, PanelLocation, PanelRegistry,
    Workspace, WorkspaceMessage, chord_from_keyboard_event,
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
                primary_window: None,
                window_ordinals: HashMap::new(),
                next_window_ordinal: 0,
                pending_close_windows: Vec::new(),
                close_eval_scheduled: false,
                app_close_prompt: None,
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

/// A dirty tab scoped to a workspace OS window (for app-quit aggregation).
#[derive(Debug, Clone)]
struct GlobalDirtyPanel {
    window: window::Id,
    window_label: String,
    location: PanelLocation,
    tab: usize,
    title: String,
}

/// App-level close confirmation before destroying workspace windows.
#[derive(Debug, Clone)]
enum AppClosePrompt {
    Window {
        target_window: window::Id,
        panels: Vec<DirtyPanel>,
    },
    AppQuit {
        target_window: window::Id,
        panels: Vec<GlobalDirtyPanel>,
    },
}

impl AppClosePrompt {
    fn target_window(&self) -> window::Id {
        match self {
            Self::Window { target_window, .. } | Self::AppQuit { target_window, .. } => {
                *target_window
            }
        }
    }
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
    /// First workspace window opened from onboarding; its layout is persisted.
    primary_window: Option<window::Id>,
    /// Stable labels for workspace windows (`OpenZone`, `OpenZone (2)`, ...).
    window_ordinals: HashMap<window::Id, usize>,
    next_window_ordinal: usize,
    /// OS windows queued for close after the user confirms discarding dirty tabs.
    pending_close_windows: Vec<window::Id>,
    close_eval_scheduled: bool,
    /// Cross-window dirty close confirmation overlay.
    app_close_prompt: Option<AppClosePrompt>,
}

impl OpenZone {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Onboarding(message) => self.update_onboarding(message),
            Message::Workspace { window, message } => {
                if self.app_close_prompt.is_some()
                    && self
                        .app_close_prompt
                        .as_ref()
                        .is_some_and(|prompt| prompt.target_window() == window)
                {
                    return Task::none();
                }
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
                    if let Some(cmd) = workspace.pending_app_command.take() {
                        use crate::workspace::workspace_command_palette::CommandId;
                        if matches!(cmd, CommandId::NewWindow) {
                            return self.open_additional_workspace();
                        }
                    }
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
            Message::WindowCloseRequested(id) => self.on_window_close_requested(id),
            Message::EvaluatePendingCloses => self.evaluate_pending_closes(),
            Message::ConfirmAppCloseDiscard => self.confirm_app_close_discard(),
            Message::ConfirmAppCloseCancel => {
                self.app_close_prompt = None;
                self.pending_close_windows.clear();
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
        self.primary_window = Some(workspace_window);
        self.register_window_ordinal(workspace_window);

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
        self.register_window_ordinal(workspace_window);
        open.discard()
    }

    /// Open a workspace window hosting a single tab torn off from another.
    fn open_workspace_with_panel(&mut self, panel: Box<dyn Panel>) -> Task<Message> {
        let settings = workspace_window_settings();
        let size = settings.size;
        let (workspace_window, open) = window::open(settings);
        let mut workspace = Workspace::single_pane(PaneState::new(vec![panel]), self.theme_mode)
            .with_window_size(size);
        workspace.set_scratch_factory(|| Box::new(ScratchPanel::new()));
        workspace.set_dock_factory(DockSide::Left, build_activity_surface);
        workspace.set_dock_factory(DockSide::Right, build_conversation_surface);
        workspace.set_dock_factory(DockSide::Bottom, build_output_surface);
        workspace.ensure_scratch_fallback();
        workspace.all_commands = default_command_items();
        self.workspaces.insert(workspace_window, workspace);
        self.register_window_ordinal(workspace_window);
        open.discard()
    }

    /// Restore the workspace from a persisted layout snapshot, or build
    /// the seeded default when nothing valid is stored.
    fn restore_or_build_workspace(&mut self) -> Workspace {
        match self.layout_store.load() {
            Some(snapshot) => {
                let mut workspace = workspace::restore(
                    &snapshot,
                    &self.registry,
                    &mut self.stores,
                    self.theme_mode,
                );
                workspace.set_scratch_factory(|| Box::new(ScratchPanel::new()));
                workspace.set_dock_factory(DockSide::Left, build_activity_surface);
                workspace.set_dock_factory(DockSide::Right, build_conversation_surface);
                workspace.set_dock_factory(DockSide::Bottom, build_output_surface);
                workspace.ensure_scratch_fallback();
                workspace.populate_empty_docks(&mut self.stores);
                workspace.all_commands = default_command_items();
                workspace
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

        if self.primary_window == Some(id) {
            self.primary_window = None;
        }
        self.workspaces.remove(&id);
        self.window_ordinals.remove(&id);

        if self.onboarding_window.is_none() && self.workspaces.is_empty() {
            iced::exit()
        } else {
            Task::none()
        }
    }

    fn register_window_ordinal(&mut self, id: window::Id) {
        self.next_window_ordinal += 1;
        self.window_ordinals.insert(id, self.next_window_ordinal);
    }

    fn window_label(&self, id: window::Id) -> String {
        match self.window_ordinals.get(&id) {
            Some(1) => String::from("OpenZone"),
            Some(n) => format!("OpenZone ({n})"),
            None => String::from("OpenZone"),
        }
    }

    fn on_window_close_requested(&mut self, id: window::Id) -> Task<Message> {
        if self.app_close_prompt.is_some() {
            return Task::none();
        }

        if self.onboarding_window == Some(id) {
            return window::close(id);
        }

        if !self.pending_close_windows.contains(&id) {
            self.pending_close_windows.push(id);
        }

        self.schedule_pending_close_evaluation()
    }

    fn schedule_pending_close_evaluation(&mut self) -> Task<Message> {
        if self.close_eval_scheduled {
            return Task::none();
        }
        self.close_eval_scheduled = true;
        Task::perform(std::future::ready(()), |_| Message::EvaluatePendingCloses)
    }

    fn evaluate_pending_closes(&mut self) -> Task<Message> {
        self.close_eval_scheduled = false;
        let pending: Vec<window::Id> = self.pending_close_windows.drain(..).collect();
        if pending.is_empty() {
            return Task::none();
        }

        let workspace_ids: HashSet<window::Id> = self.workspaces.keys().copied().collect();
        let pending_set: HashSet<window::Id> = pending.iter().copied().collect();
        let is_app_quit = !workspace_ids.is_empty() && pending_set == workspace_ids;

        if is_app_quit {
            return self.prompt_app_quit();
        }

        let mut batch = Task::none();
        let mut dirty_windows = Vec::new();

        for id in pending {
            let dirty = self
                .workspaces
                .get(&id)
                .map(Workspace::collect_dirty_panels)
                .unwrap_or_default();
            if dirty.is_empty() {
                batch = Task::batch([batch, self.force_close_workspace(id)]);
            } else {
                dirty_windows.push((id, dirty));
            }
        }

        if dirty_windows.is_empty() {
            return batch;
        }

        // Prompt one dirty window at a time so earlier prompts are not overwritten.
        for (id, _) in dirty_windows.iter().skip(1) {
            if !self.pending_close_windows.contains(id) {
                self.pending_close_windows.push(*id);
            }
        }

        let (id, dirty) = dirty_windows.remove(0);
        if let Some(workspace) = self.workspaces.get_mut(&id) {
            workspace.palette.dismiss();
        }
        self.app_close_prompt = Some(AppClosePrompt::Window {
            target_window: id,
            panels: dirty,
        });
        Task::batch([batch, Task::none()])
    }

    fn prompt_app_quit(&mut self) -> Task<Message> {
        let mut all_dirty = Vec::new();
        for (id, workspace) in &self.workspaces {
            let label = self.window_label(*id);
            for panel in workspace.collect_dirty_panels() {
                all_dirty.push(GlobalDirtyPanel {
                    window: *id,
                    window_label: label.clone(),
                    location: panel.location,
                    tab: panel.tab,
                    title: panel.title,
                });
            }
        }

        if all_dirty.is_empty() {
            let windows: Vec<window::Id> = self.workspaces.keys().copied().collect();
            let mut batch = Task::none();
            for id in windows {
                batch = Task::batch([batch, self.force_close_workspace(id)]);
            }
            return batch;
        }

        let target_window = self
            .workspaces
            .keys()
            .next()
            .copied()
            .expect("app quit implies at least one workspace");
        if let Some(workspace) = self.workspaces.get_mut(&target_window) {
            workspace.palette.dismiss();
        }
        self.app_close_prompt = Some(AppClosePrompt::AppQuit {
            target_window,
            panels: all_dirty,
        });
        Task::none()
    }

    fn confirm_app_close_discard(&mut self) -> Task<Message> {
        let Some(prompt) = self.app_close_prompt.take() else {
            return Task::none();
        };

        let mut batch = match prompt {
            AppClosePrompt::Window {
                target_window,
                panels,
            } => {
                if let Some(workspace) = self.workspaces.get_mut(&target_window) {
                    workspace.discard_dirty_panels(panels, &mut self.stores);
                }
                self.force_close_workspace(target_window)
            }
            AppClosePrompt::AppQuit { panels, .. } => {
                let mut by_window: HashMap<window::Id, Vec<DirtyPanel>> = HashMap::new();
                for panel in panels {
                    by_window.entry(panel.window).or_default().push(DirtyPanel {
                        location: panel.location,
                        tab: panel.tab,
                        title: panel.title,
                        message: std::borrow::Cow::Borrowed(""),
                    });
                }
                for (window_id, dirty) in by_window {
                    if let Some(workspace) = self.workspaces.get_mut(&window_id) {
                        workspace.discard_dirty_panels(dirty, &mut self.stores);
                    }
                }
                let windows: Vec<window::Id> = self.workspaces.keys().copied().collect();
                let mut quit_batch = Task::none();
                for id in windows {
                    quit_batch = Task::batch([quit_batch, self.force_close_workspace(id)]);
                }
                quit_batch
            }
        };

        if !self.pending_close_windows.is_empty() {
            batch = Task::batch([batch, self.schedule_pending_close_evaluation()]);
        }
        batch
    }
    fn save_primary_layout_if_needed(&mut self, id: window::Id) {
        if self.primary_window != Some(id) {
            return;
        }
        if let Some(workspace) = self.workspaces.get(&id) {
            let snapshot = workspace::capture(workspace, &self.stores);
            let _ = self.layout_store.save(&snapshot);
        }
    }

    fn force_close_workspace(&mut self, id: window::Id) -> Task<Message> {
        self.save_primary_layout_if_needed(id);
        window::close(id)
    }

    fn view(&self, window: window::Id) -> Element<'_, Message> {
        if self.onboarding_window == Some(window)
            && let Some(onboarding) = &self.onboarding
        {
            return onboarding_view(onboarding).map(Message::Onboarding);
        }

        if let Some(workspace) = self.workspaces.get(&window) {
            let base = workspace::workspace_view::view(workspace, &self.stores)
                .map(move |message| Message::Workspace { window, message });
            if let Some(prompt) = &self.app_close_prompt
                && prompt.target_window() == window
            {
                let (title, list_items) = match prompt {
                    AppClosePrompt::Window { panels, .. } => (
                        Workspace::batch_close_summary(panels).to_string(),
                        if panels.len() > 1 {
                            panels
                                .iter()
                                .map(|panel| format!("• {}", panel.title))
                                .collect()
                        } else {
                            Vec::new()
                        },
                    ),
                    AppClosePrompt::AppQuit { panels, .. } => (
                        Workspace::app_quit_close_summary(panels.len()).to_string(),
                        panels
                            .iter()
                            .map(|panel| format!("• {}: {}", panel.window_label, panel.title))
                            .collect(),
                    ),
                };
                return workspace::workspace_view::close_prompt_overlay(
                    base,
                    workspace.theme,
                    title,
                    list_items,
                    Message::ConfirmAppCloseCancel,
                    Message::ConfirmAppCloseDiscard,
                );
            }
            return base;
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

        streams.push(window::close_events().map(Message::WindowClosed));
        streams.push(window::close_requests().map(Message::WindowCloseRequested));

        Subscription::batch(streams)
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
    /// User requested closing an OS window (title-bar close or app quit).
    WindowCloseRequested(window::Id),
    /// Evaluate batched window close requests after the event loop tick.
    EvaluatePendingCloses,
    ConfirmAppCloseDiscard,
    ConfirmAppCloseCancel,
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
        exit_on_close_request: false,
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
    registry.register(PanelKind::Scratch, |snapshot, _stores| {
        Box::new(ScratchPanel::from_snapshot(snapshot))
    });
    registry
}

/// Build the primary workspace layout: one center pane hosting a
/// single ScratchPanel as the non-durable startup tab.
fn build_workspace(_stores: &mut AppStores, theme_mode: ThemeMode) -> Workspace {
    let center = PaneState::new(vec![Box::new(ScratchPanel::new())]);
    let mut workspace = Workspace::single_pane(center, theme_mode);
    workspace.set_scratch_factory(|| Box::new(ScratchPanel::new()));
    workspace.set_dock_factory(DockSide::Left, build_activity_surface);
    workspace.set_dock_factory(DockSide::Right, build_conversation_surface);
    workspace.set_dock_factory(DockSide::Bottom, build_output_surface);
    workspace.all_commands = default_command_items();
    workspace.ensure_scratch_fallback();
    workspace
}

/// Build an additional workspace window with dock surface factories
/// wired but no seeded dock content — docks open empty until the
/// user (or restore) triggers them.
fn build_secondary_workspace(_stores: &mut AppStores, theme_mode: ThemeMode) -> Workspace {
    let center = PaneState::new(vec![Box::new(ScratchPanel::new())]);
    let mut workspace = Workspace::single_pane(center, theme_mode);
    workspace.set_scratch_factory(|| Box::new(ScratchPanel::new()));
    workspace.set_dock_factory(DockSide::Left, build_activity_surface);
    workspace.set_dock_factory(DockSide::Right, build_conversation_surface);
    workspace.set_dock_factory(DockSide::Bottom, build_output_surface);
    workspace.all_commands = default_command_items();
    workspace.ensure_scratch_fallback();
    workspace
}

// ---- Default dock surface factories -----------------------------------------
// The composition root owns these factories; the shell calls them when
// opening an empty dock. Each factory is a function pointer matching
// `fn(&mut AppStores) -> Box<dyn Panel>`. Replace the ScratchPanel placeholder
// with the real Activity/Conversation/Output constructors when they land.

fn build_activity_surface(_stores: &mut AppStores) -> Box<dyn Panel> {
    Box::new(ScratchPanel::new())
}

fn build_conversation_surface(_stores: &mut AppStores) -> Box<dyn Panel> {
    Box::new(ScratchPanel::new())
}

fn build_output_surface(_stores: &mut AppStores) -> Box<dyn Panel> {
    Box::new(ScratchPanel::new())
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
            primary_window: None,
            window_ordinals: HashMap::new(),
            next_window_ordinal: 0,
            pending_close_windows: Vec::new(),
            close_eval_scheduled: false,
            app_close_prompt: None,
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
            workspace_b.docks.left.visibility = DockVisibility::Open;
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

#[cfg(test)]
struct CountingLayoutStore {
    saves: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl CountingLayoutStore {
    fn new() -> Self {
        Self {
            saves: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn save_count(&self) -> usize {
        self.saves.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
impl LayoutStore for CountingLayoutStore {
    fn load(&self) -> Option<workspace::LayoutSnapshot> {
        None
    }

    fn save(
        &self,
        _snapshot: &workspace::LayoutSnapshot,
    ) -> Result<(), workspace::LayoutStoreError> {
        self.saves.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod window_close_guard_tests {
    use super::*;
    use crate::features::ScratchMessage;
    use crate::workspace::erase;
    use iced::widget::text_editor;

    fn open_zone_with_dirty_scratch() -> (OpenZone, window::Id) {
        let window_id = window::Id::unique();
        let mut stores = AppStores::new();
        let mut workspace = build_workspace(&mut stores, ThemeMode::Dark);
        let location = workspace.focused;
        workspace.update(
            WorkspaceMessage::Panel {
                location,
                tab: 0,
                message: erase(ScratchMessage::Edit(text_editor::Action::Edit(
                    text_editor::Edit::Insert('x'),
                ))),
            },
            &mut stores,
        );
        assert_eq!(workspace.collect_dirty_panels().len(), 1);

        let app = OpenZone {
            onboarding: None,
            onboarding_window: None,
            workspaces: HashMap::from([(window_id, workspace)]),
            stores,
            persistence: Arc::new(InMemoryOnboardingPersistence::new()),
            registry: build_registry(),
            layout_store: Arc::new(NoopLayoutStore),
            theme_mode: ThemeMode::Dark,
            primary_window: Some(window_id),
            window_ordinals: HashMap::from([(window_id, 1)]),
            next_window_ordinal: 1,
            pending_close_windows: Vec::new(),
            close_eval_scheduled: false,
            app_close_prompt: None,
        };
        (app, window_id)
    }

    #[test]
    fn close_requested_with_dirty_opens_app_quit_prompt() {
        let (mut app, window_id) = open_zone_with_dirty_scratch();
        let _ = app.on_window_close_requested(window_id);
        let _ = app.evaluate_pending_closes();

        assert!(matches!(
            app.app_close_prompt,
            Some(AppClosePrompt::AppQuit { panels, .. }) if panels.len() == 1
        ));
    }

    #[test]
    fn cancel_app_close_prompt_clears_state() {
        let (mut app, window_id) = open_zone_with_dirty_scratch();
        let _ = app.on_window_close_requested(window_id);
        let _ = app.evaluate_pending_closes();
        let _ = app.update(Message::ConfirmAppCloseCancel);

        assert!(app.app_close_prompt.is_none());
        assert!(app.pending_close_windows.is_empty());
        assert_eq!(app.workspaces[&window_id].collect_dirty_panels().len(), 1);
    }

    #[test]
    fn discard_app_close_prompt_removes_dirty_tabs() {
        let (mut app, window_id) = open_zone_with_dirty_scratch();
        let _ = app.on_window_close_requested(window_id);
        let _ = app.evaluate_pending_closes();
        let _ = app.confirm_app_close_discard();

        assert!(app.app_close_prompt.is_none());
        assert!(app.workspaces[&window_id].collect_dirty_panels().is_empty());
    }

    #[test]
    fn partial_multi_window_dirty_close_prompts_sequentially() {
        let (mut app, window_a) = open_zone_with_dirty_scratch();
        let window_b = window::Id::unique();
        let mut stores = AppStores::new();
        let mut workspace_b = build_secondary_workspace(&mut stores, ThemeMode::Dark);
        let location = workspace_b.focused;
        workspace_b.update(
            WorkspaceMessage::Panel {
                location,
                tab: 0,
                message: erase(ScratchMessage::Edit(text_editor::Action::Edit(
                    text_editor::Edit::Insert('y'),
                ))),
            },
            &mut stores,
        );
        app.workspaces.insert(window_b, workspace_b);
        app.stores = stores;
        app.register_window_ordinal(window_b);

        let _ = app.on_window_close_requested(window_a);
        let _ = app.on_window_close_requested(window_b);
        let _ = app.evaluate_pending_closes();

        assert!(matches!(
            app.app_close_prompt,
            Some(AppClosePrompt::Window {
                target_window,
                ref panels,
                ..
            }) if target_window == window_a && panels.len() == 1
        ));
        assert_eq!(app.pending_close_windows, vec![window_b]);

        let _ = app.confirm_app_close_discard();

        assert!(matches!(
            app.app_close_prompt,
            Some(AppClosePrompt::Window {
                target_window,
                ref panels,
                ..
            }) if target_window == window_b && panels.len() == 1
        ));
        assert!(app.pending_close_windows.is_empty());
        assert!(app.workspaces.contains_key(&window_a));
        assert!(app.workspaces[&window_b].collect_dirty_panels().len() == 1);
    }

    #[test]
    fn primary_layout_saved_only_on_primary_close() {
        let primary_id = window::Id::unique();
        let secondary_id = window::Id::unique();
        let layout_store = Arc::new(CountingLayoutStore::new());
        let mut stores = AppStores::new();
        let primary_workspace = build_workspace(&mut stores, ThemeMode::Dark);
        let secondary_workspace = build_secondary_workspace(&mut stores, ThemeMode::Dark);

        let mut app = OpenZone {
            onboarding: None,
            onboarding_window: None,
            workspaces: HashMap::from([
                (primary_id, primary_workspace),
                (secondary_id, secondary_workspace),
            ]),
            stores,
            persistence: Arc::new(InMemoryOnboardingPersistence::new()),
            registry: build_registry(),
            layout_store: layout_store.clone(),
            theme_mode: ThemeMode::Dark,
            primary_window: Some(primary_id),
            window_ordinals: HashMap::from([(primary_id, 1), (secondary_id, 2)]),
            next_window_ordinal: 2,
            pending_close_windows: Vec::new(),
            close_eval_scheduled: false,
            app_close_prompt: None,
        };

        let _ = app.force_close_workspace(secondary_id);
        assert_eq!(layout_store.save_count(), 0);

        let _ = app.force_close_workspace(primary_id);
        assert_eq!(layout_store.save_count(), 1);
    }

    #[test]
    fn simultaneous_close_requests_aggregate_as_app_quit() {
        let (mut app, window_a) = open_zone_with_dirty_scratch();
        let window_b = window::Id::unique();
        let mut stores_b = AppStores::new();
        let workspace_b = build_secondary_workspace(&mut stores_b, ThemeMode::Dark);
        app.workspaces.insert(window_b, workspace_b);
        app.register_window_ordinal(window_b);

        let _ = app.on_window_close_requested(window_a);
        let _ = app.on_window_close_requested(window_b);
        let _ = app.evaluate_pending_closes();

        assert!(matches!(
            app.app_close_prompt,
            Some(AppClosePrompt::AppQuit { .. })
        ));
    }
}
