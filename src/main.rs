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
//! Onboarding is never overridden in place.

mod features;
mod shared;
mod workspace;

use std::sync::Arc;

use iced::event::{self, Event};
use iced::{Element, Size, Subscription, Task, Theme, window};

use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
use crate::features::onboarding::infrastructure::file_persistence::FileOnboardingPersistence;
use crate::features::onboarding::infrastructure::memory_persistence::InMemoryOnboardingPersistence;
use crate::features::onboarding::{
    OnboardingMessage, OnboardingOutcome, OnboardingPersistence, OnboardingState, mark_completed,
    view as onboarding_view,
};
use crate::shared::design::ThemeMode;
use crate::workspace::{
    AppStores, Chord, Docks, FileLayoutStore, LayoutStore, PaneState, Panel, PanelKind,
    PanelRegistry, Workspace, WorkspaceMessage, chord_from_keyboard_event,
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
                workspace: None,
                workspace_window: None,
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
/// Holds at most one onboarding window and one workspace window, each
/// paired with the feature state it renders. A window id with no state
/// (or vice versa) never occurs: they are set and cleared together.
///
/// `stores` is the app-root [`AppStores`] — the single owner of Counter
/// and Clock state across the whole app. Workspace state lives in a
/// sibling `workspace` field so the two split-borrow cleanly when the
/// reducer runs.
struct OpenZone {
    onboarding: Option<OnboardingState>,
    onboarding_window: Option<window::Id>,
    workspace: Option<Workspace>,
    workspace_window: Option<window::Id>,
    /// App-root domain stores. Lives next to `workspace` so
    /// `OpenZone::update` can split-borrow the two when handing
    /// `&mut AppStores` to the workspace reducer.
    stores: AppStores,
    persistence: Arc<dyn OnboardingPersistence>,
    /// The composition seam the shell uses to rehydrate panels from
    /// persisted handles. It knows panel kinds, not concrete types.
    registry: PanelRegistry,
    /// Layout persistence: a saved snapshot is restored when entering the
    /// workspace and a fresh one is written when its window closes.
    layout_store: Arc<dyn LayoutStore>,
    theme_mode: ThemeMode,
}

impl OpenZone {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Onboarding(message) => self.update_onboarding(message),
            Message::Workspace(message) => {
                if let Some(workspace) = self.workspace.as_mut() {
                    // Sibling-field split borrow: `workspace` and
                    // `stores` are independent fields on `self`, so the
                    // reducer can mutate both without aliasing.
                    workspace.update(message, &mut self.stores);
                }
                Task::none()
            }
            Message::Key { window, chord } => {
                // Keyboard events fire for whichever window is focused;
                // only the workspace window routes chords into the shell.
                if self.workspace_window == Some(window)
                    && let Some(workspace) = self.workspace.as_mut()
                {
                    workspace.update(WorkspaceMessage::Key(chord), &mut self.stores);
                }
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

    /// Open the workspace in its own dedicated window and close the
    /// onboarding window. The workspace does not replace onboarding in
    /// place — it is a genuinely separate OS window. A previously saved
    /// layout is restored through the registry; absent or unreadable,
    /// the shell falls back to its seeded default layout.
    fn enter_workspace(&mut self) -> Task<Message> {
        let (workspace_window, open) = window::open(workspace_window_settings());
        self.workspace_window = Some(workspace_window);
        self.workspace = Some(self.restore_or_build_workspace());

        let close = match self.onboarding_window.take() {
            Some(onboarding_window) => {
                self.onboarding = None;
                window::close(onboarding_window)
            }
            None => Task::none(),
        };

        Task::batch([open.discard(), close])
    }

    /// Restore the workspace from a persisted layout snapshot, or build
    /// the seeded default when nothing valid is stored. Either path
    /// allocates panel store slots through `&mut self.stores`.
    fn restore_or_build_workspace(&mut self) -> Workspace {
        match self.layout_store.load() {
            Some(snapshot) => {
                workspace::restore(&snapshot, &self.registry, &mut self.stores, self.theme_mode)
            }
            None => build_workspace(&mut self.stores, self.theme_mode),
        }
    }

    /// React to a window the user (or our own handoff) closed. When no
    /// windows remain, the daemon has nothing left to show, so exit.
    fn handle_window_closed(&mut self, id: window::Id) -> Task<Message> {
        if self.onboarding_window == Some(id) {
            self.onboarding_window = None;
            self.onboarding = None;
        }
        if self.workspace_window == Some(id) {
            // Persist the final layout before tearing the workspace down,
            // so the next launch restores splits, tabs, docks, and focus.
            // Capture reads each panel's snapshot through the canonical
            // store value so the persisted handle reflects the latest
            // store state, not stale panel-side bookkeeping.
            if let Some(workspace) = self.workspace.as_ref() {
                let snapshot = workspace::capture(workspace, &self.stores);
                let _ = self.layout_store.save(&snapshot);
            }
            self.workspace_window = None;
            self.workspace = None;
        }

        if self.onboarding_window.is_none() && self.workspace_window.is_none() {
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

        if self.workspace_window == Some(window)
            && let Some(workspace) = &self.workspace
        {
            return workspace::view::view(workspace, &self.stores).map(Message::Workspace);
        }

        // A window with no backing state (e.g. mid-close) renders empty.
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

        if let Some(workspace) = &self.workspace {
            streams.push(workspace.subscription().map(Message::Workspace));
            streams.push(event::listen_with(workspace_key_event));
        }

        streams.push(window::close_events().map(Message::WindowClosed));

        Subscription::batch(streams)
    }

    fn title(&self, _window: window::Id) -> String {
        String::from("OpenZone")
    }

    fn theme(&self, window: window::Id) -> Theme {
        let mode = if self.workspace_window == Some(window) {
            self.workspace
                .as_ref()
                .map(|workspace| workspace.theme_mode)
                .unwrap_or(self.theme_mode)
        } else {
            self.onboarding
                .as_ref()
                .map(|onboarding| onboarding.theme_mode)
                .unwrap_or(self.theme_mode)
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
    Workspace(WorkspaceMessage),
    /// A key chord, tagged with the window that produced it so the
    /// reducer can route it only to the workspace window.
    Key {
        window: window::Id,
        chord: Chord,
    },
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
            chord_from_keyboard_event(&keyboard).map(|chord| Message::Key { window, chord })
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

/// Pick the production persistence backend, degrading to in-memory only
/// if the OS data directory cannot be resolved.
fn load_persistence() -> Arc<dyn OnboardingPersistence> {
    match FileOnboardingPersistence::from_project_dirs() {
        Ok(store) => Arc::new(store),
        Err(_) => Arc::new(InMemoryOnboardingPersistence::new()),
    }
}

/// Pick the layout-persistence backend. If the OS data directory cannot
/// be resolved, fall back to a no-op store so the shell still launches
/// (it just won't remember layout across runs).
fn load_layout_store() -> Arc<dyn LayoutStore> {
    match FileLayoutStore::from_project_dirs() {
        Ok(store) => Arc::new(store),
        Err(_) => Arc::new(NoopLayoutStore),
    }
}

/// A layout store that persists nothing. Used only when no data
/// directory is available; `load` always yields the seeded default.
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

/// Register feature panel constructors. This table is the composition
/// seam the shell uses to rehydrate panels; it knows kinds, not types.
/// Each constructor receives `&mut AppStores` so a store-backed panel
/// (Counter) can allocate its slot at rehydrate time.
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

/// Build the workspace layout: one center pane hosting the dummy panels
/// as tabs, with one dummy panel per edge dock so docks can be exercised.
/// Every constructed Counter allocates a fresh [`AppStores::counter`]
/// slot so each panel observes an independent count.
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
