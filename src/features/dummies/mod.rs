#![allow(dead_code)]

//! Dummy panels for the workspace build spine.
//!
//! These prove the [`Panel`](crate::workspace::Panel) port end to end
//! before any real feature exists:
//!
//! * [`counter`] — trivial interactive panel (button-driven state).
//! * [`text`] — text-input panel (widget-focus placeholder slice).
//! * [`clock`] — ticks via a panel-level subscription.
//!
//! They depend on the shell (`features -> workspace`), never the
//! reverse. The composition root registers their constructors.

pub mod clock;
pub mod counter;
pub mod text;

pub use clock::ClockPanel;
pub use counter::CounterPanel;
pub use text::TextPanel;
