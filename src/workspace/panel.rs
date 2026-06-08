#![allow(dead_code)]

//! The `Panel` port — the stable, final contract every workspace panel
//! implements.
//!
//! `dyn Panel` cannot carry an associated message type and remain
//! object-safe, so every panel **erases** its concrete message to a
//! dynamic type at the trait boundary. The shell routes all panel
//! messages through this single erased path.

use std::any::Any;
use std::sync::Arc;

use iced::{Element, Subscription};

use crate::workspace::command::Chord;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Counter,
    Text,
    Clock,
}

/// The final panel contract.
///
/// Every lifecycle concern the shell needs lives here: how a panel
/// titles itself, renders, folds messages, subscribes to external
/// streams, identifies its kind, and serializes a rehydration handle.
pub trait Panel {
    /// Human-readable tab/title-bar label.
    fn title(&self) -> String;

    /// Stable kind identity for the registry and persistence.
    fn kind(&self) -> PanelKind;

    /// Render the panel. Concrete messages are erased at this boundary.
    fn view(&self) -> Element<'_, ErasedMessage>;

    /// Fold an erased message back into panel state. Implementations
    /// downcast to their concrete message type and `debug_assert!` on a
    /// failed downcast so misrouting is loud in development but does not
    /// crash release builds.
    fn update(&mut self, message: ErasedMessage);

    /// Optional ongoing external stream (timers, PTYs, token streams).
    /// Defaults to none. The workspace batches every panel's
    /// subscription; Iced starts/stops them as panels appear/drop.
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

    /// A handle-only snapshot for layout persistence. Stores a
    /// rehydration handle (e.g. a counter value, a file path), never the
    /// panel's full content.
    fn snapshot(&self) -> serde_json::Value;
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
