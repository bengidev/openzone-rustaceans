#![allow(dead_code)]

//! Workspace shell — the cross-cutting UI composition layer.
//!
//! This is **not** a domain feature: it has no business rules, so it has
//! no `domain/application/infrastructure` layering. It owns the layout
//! engine ([`state::Workspace`]), the [`panel::Panel`] port that
//! features implement, and the [`registry::PanelRegistry`] composition
//! seam.
//!
//! Dependency rule: `features/<panel> -> workspace` (to implement the
//! `Panel` trait), never `workspace -> a concrete feature`. The shell
//! addresses panels only through the trait and `PanelKind`.
//!
//! See `src/workspace/CONTEXT.md` for the bounded-context contract.

#![allow(unused_imports)]

pub mod command;
pub mod dock;
pub mod drag;
pub mod layout_metrics;
pub mod layout_store;
pub mod location;
pub mod message;
pub mod pane_state;
pub mod panel;
pub mod persistence;
pub mod registry;
pub mod state;
pub mod stores;
pub mod view;

pub use command::{Chord, Command, KeyRef, Keymap, Mods, chord_from_keyboard_event};
pub use dock::{Dock, Docks};
pub use drag::{Direction, DragState, DropTarget, SplitPaneTarget, TabStripTarget};
use iced::{Subscription, Task, Theme, window};
pub use layout_store::{FileLayoutStore, LayoutStore, LayoutStoreError};
pub use location::{DockSide, PanelLocation};
pub use message::WorkspaceMessage;
pub use pane_state::PaneState;
pub use panel::{ErasedMessage, Panel, PanelKind, downcast, erase};
pub use persistence::{LayoutSnapshot, capture, restore};
pub use registry::{PanelConstructor, PanelRegistry};
pub use state::{CrossWindowDropPreview, Workspace};
pub use stores::AppStores;
#[cfg(test)]
pub use stores::{ClockStore, CounterId, CounterStore};

use crate::shared::design::ThemeMode;

/// Launch the workspace shell as a single-window Iced application.
///
/// The composition root passes a `build_pane` factory and a
/// `build_docks` factory (the shell's boot closure may run more than
/// once, so the initial layout must be reconstructible) and the
/// registry. The registry is retained for later slices (persistence
/// rehydrate, dynamic panel open).
///
/// This single-window entry point owns its own [`AppStores`] for
/// parity with the daemon entry point, which keeps `AppStores` as a
/// sibling field of the workspace on the app root.
pub fn run<F, D>(
    build_pane: F,
    build_docks: D,
    _registry: PanelRegistry,
    theme_mode: ThemeMode,
) -> iced::Result
where
    F: Fn() -> PaneState + 'static,
    D: Fn() -> Docks + 'static,
{
    iced::application(
        move || {
            let workspace = Workspace::with_docks(build_pane(), build_docks(), theme_mode);
            (
                WorkspaceApp {
                    workspace,
                    stores: AppStores::new(),
                },
                Task::none(),
            )
        },
        WorkspaceApp::update,
        WorkspaceApp::view,
    )
    .title(WorkspaceApp::title)
    .subscription(WorkspaceApp::subscription)
    .theme(WorkspaceApp::theme)
    .window_size(iced::Size::new(1100.0, 760.0))
    .run()
}

/// Iced application wrapper for the workspace shell.
///
/// Holds [`AppStores`] alongside the [`Workspace`] so the reducer can
/// split-borrow them — the same pattern the multi-window daemon uses
/// at app root.
struct WorkspaceApp {
    workspace: Workspace,
    stores: AppStores,
}

impl WorkspaceApp {
    fn update(&mut self, message: WorkspaceMessage) -> Task<WorkspaceMessage> {
        self.workspace.update(message, &mut self.stores);
        Task::none()
    }

    fn view(&self) -> iced::Element<'_, WorkspaceMessage> {
        view::view(&self.workspace, &self.stores)
    }

    fn subscription(&self) -> Subscription<WorkspaceMessage> {
        let mut streams = vec![
            self.workspace.subscription(),
            iced::keyboard::listen()
                .filter_map(|event| chord_from_keyboard_event(&event).map(WorkspaceMessage::Key)),
            window::resize_events().map(|(_, size)| WorkspaceMessage::WindowResized(size)),
        ];
        if self.workspace.is_tab_drag_active() {
            streams.push(crate::workspace::state::tab_drag_subscription());
        }
        Subscription::batch(streams)
    }

    fn title(&self) -> String {
        String::from("OpenZone")
    }

    fn theme(&self) -> Theme {
        match self.workspace.theme_mode {
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::Light => Theme::Light,
        }
    }
}
