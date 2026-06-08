#![allow(dead_code)]

//! The `Panel` port — the stable, final contract every workspace panel
//! implements.
//!
//! `dyn Panel` cannot carry an associated message type and remain
//! object-safe, so every panel **erases** its concrete message to a
//! dynamic type at the trait boundary. The shell routes all panel
//! messages through this single erased path.
//!
//! # View-over-handle
//!
//! Panels do not own domain data. Counter and Clock panels read their
//! state from app-root stores ([`AppStores`]) handed in by the shell;
//! the panel itself only carries the *handle* needed to address its
//! slice of the store (a [`crate::workspace::stores::CounterId`] for
//! Counter, nothing for Clock since the clock store is global). Their
//! [`Panel::view`] and [`Panel::snapshot`] both take `&AppStores` so
//! every render and persistence point sees the canonical store value.
//!
//! # Intent lifting
//!
//! Panel messages are *intents* (e.g. `CounterMessage::Increment`)
//! erased at the trait boundary. The workspace reducer is the single
//! writer: it routes every intent through [`Panel::update`], which
//! receives `&mut AppStores` and folds the intent into a store
//! mutation. Panel-local UI state (text input buffer, scroll offset)
//! still lands on `self` from the same call. Either way, the only
//! `&mut` access to stores comes from the app-root `update` path —
//! there is no interior mutability lock anywhere.

use std::any::Any;
use std::sync::Arc;

use iced::{Element, Subscription};
use serde::{Deserialize, Serialize};

use crate::workspace::command::Chord;
use crate::workspace::stores::AppStores;

/// A panel message erased to a sendable, cloneable dynamic type.
///
/// `Arc` (not `Box`) is deliberate: Iced widgets such as `button` and
/// `text_input` require the application message to be `Clone`, and
/// `Box<dyn Any>` is not `Clone`. An `Arc` clones by refcount and keeps
/// the erased payload shareable. `Send + Sync` is mandatory — Iced
/// `Task`s and `Subscription`s run on the tokio executor.
pub type ErasedMessage = Arc<dyn Any + Send + Sync>;

/// Identity of a panel type.
///
/// Drives the registry (`PanelKind -> constructor`) and persistence (a
/// snapshot is meaningless without the kind that knows how to rehydrate
/// it). This enum is the one place the shell enumerates known panel
/// kinds; panels themselves are trait objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelKind {
    Counter,
    Text,
    Clock,
}

/// The final panel contract.
///
/// Every lifecycle concern the shell needs lives here: how a panel
/// titles itself, renders against shared stores, folds intents through
/// app-root state, subscribes to *panel-local* external streams,
/// identifies its kind, releases its store handle on close, and
/// serializes a rehydration handle.
pub trait Panel {
    /// Human-readable tab/title-bar label.
    fn title(&self) -> String;

    /// Stable kind identity for the registry and persistence.
    fn kind(&self) -> PanelKind;

    /// Render the panel as a view over `stores`. Concrete messages are
    /// erased at this boundary; the resulting [`ErasedMessage`] is the
    /// panel's *intent*, lifted by the workspace into a store mutation
    /// in [`Panel::update`].
    fn view<'a>(&'a self, stores: &'a AppStores) -> Element<'a, ErasedMessage>;

    /// Fold an erased intent. Implementations downcast to their concrete
    /// message type and choose whether the mutation lands on `self` (UI
    /// state — text-input buffer, scroll position) or on `stores`
    /// (domain state — counter values, etc.). A failed downcast fires a
    /// `debug_assert!` so misrouting is loud in development but does not
    /// crash release builds.
    fn update(&mut self, message: ErasedMessage, stores: &mut AppStores);

    /// Optional ongoing *panel-local* external stream (PTYs, token
    /// streams, panel-only timers). Defaults to none. The workspace
    /// batches every panel's subscription; Iced starts/stops them as
    /// panels appear/drop.
    ///
    /// Cross-panel streams that fan out to many panels (e.g. the global
    /// 1-Hz clock tick that drives every Clock panel) live at the
    /// workspace layer instead, gated on whether any addressing panel
    /// still exists. Per-panel subscriptions here are the right call
    /// only when the data source belongs to *this* panel instance.
    fn subscription(&self) -> Subscription<ErasedMessage> {
        Subscription::none()
    }

    /// Whether this panel consumes `chord` itself (panel-first key
    /// capture). When `true`, the workspace does **not** resolve the
    /// chord against its keymap — the focused panel swallowed it (e.g. a
    /// text input absorbing a character). When `false`, the unhandled
    /// chord bubbles up to the workspace command layer.
    ///
    /// Defaults to `false`: most panels are display-only and let every
    /// chord reach the workspace. Interactive panels (text inputs)
    /// override this to claim the keys they type into.
    fn captures_chord(&self, _chord: Chord) -> bool {
        false
    }

    /// A handle-only snapshot for layout persistence. Reads from
    /// `stores` so a Counter panel persists the canonical store count
    /// rather than a stale local copy. Stores a rehydration handle
    /// (e.g. a counter value, a file path), never the panel's full
    /// content.
    fn snapshot(&self, stores: &AppStores) -> serde_json::Value;

    /// Release any store slot this panel holds. Called by the workspace
    /// reducer when the tab carrying this panel is closed, so an
    /// addressed [`crate::workspace::stores::CounterId`] never lingers
    /// past the lifetime of its addressing panel. Default: no-op (panels
    /// without a per-instance handle, like Clock or Text, do nothing).
    fn on_close(&mut self, _stores: &mut AppStores) {}
}

/// Erase a concrete panel message to [`ErasedMessage`].
///
/// Use as the map function at a panel's view/subscription boundary:
/// `Element::from(content).map(erase::<MyMessage>)`.
pub fn erase<M>(message: M) -> ErasedMessage
where
    M: Any + Send + Sync,
{
    Arc::new(message)
}

/// Recover a concrete message from an [`ErasedMessage`].
///
/// Returns `None` when the payload is not of type `M`; callers pair this
/// with a `debug_assert!` to surface misrouted messages in development.
pub fn downcast<M>(message: ErasedMessage) -> Option<Arc<M>>
where
    M: Any + Send + Sync,
{
    message.downcast::<M>().ok()
}
