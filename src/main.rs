//! OpenZone Rustaceans — composition root.
//!
//! This binary composes internal modules, chooses infrastructure, and
//! launches the Iced application. It is the one place that names
//! concrete feature panels and wires them into the shell registry; the
//! workspace shell itself never depends on a concrete feature.

mod features;
mod shared;
mod workspace;

use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
use crate::shared::design::ThemeMode;
use crate::workspace::{Panel, PaneState, PanelKind, PanelRegistry};

fn main() -> iced::Result {
    // Register feature panel constructors. This table is the
    // composition seam the shell uses to rehydrate panels (persistence,
    // later dynamic open). The shell knows kinds, not concrete types.
    let mut registry = PanelRegistry::new();
    registry
        .register(PanelKind::Counter, |snapshot| {
            Box::new(CounterPanel::from_snapshot(snapshot))
        })
        .register(PanelKind::Text, |snapshot| {
            Box::new(TextPanel::from_snapshot(snapshot))
        })
        .register(PanelKind::Clock, |snapshot| {
            Box::new(ClockPanel::from_snapshot(snapshot))
        });

    // One center pane hosting the three dummy panels as tabs. Wrapped
    // in a factory because the shell's boot closure may run more than
    // once; each boot constructs fresh panels.
    let build_pane = || {
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new()),
            Box::new(TextPanel::new()),
            Box::new(ClockPanel::new()),
        ];
        PaneState::new(tabs)
    };

    workspace::run(build_pane, registry, ThemeMode::Dark)
}
