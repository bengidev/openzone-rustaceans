#![allow(dead_code)]

//! Handle-only layout persistence.
//!
//! The workspace serializes a *handle* of its layout — the split-tree
//! structure, per-pane tab stacks, dock open state, focus, and each
//! panel's rehydration handle (its [`PanelKind`] + `snapshot()`), never
//! a panel's full content. On relaunch the [`PanelRegistry`] rebuilds
//! each panel from its handle, so the shell restores structure without
//! ever naming a concrete feature type.
//!
//! This module is pure: it converts between the live [`Workspace`] and a
//! serializable [`LayoutSnapshot`]. Filesystem IO lives in
//! [`crate::workspace::layout_store`].

use iced::widget::pane_grid::{self, Axis, Configuration, Node};
use serde::{Deserialize, Serialize};

use crate::shared::design::ThemeMode;
use crate::workspace::dock::{Dock, Docks};
use crate::workspace::location::{DockSide, PanelLocation};
use crate::workspace::pane_state::PaneState;
use crate::workspace::panel::{Panel, PanelKind};
use crate::workspace::registry::PanelRegistry;
use crate::workspace::state::Workspace;

/// A panel reduced to its rehydration handle: which kind, plus the
/// handle-only JSON the panel's own `snapshot()` produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelHandle {
    pub kind: PanelKind,
    pub snapshot: serde_json::Value,
}

/// Split direction, mirroring [`pane_grid::Axis`] without leaking the
/// runtime type into the serialized form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

impl SplitAxis {
    fn from_axis(axis: Axis) -> Self {
        match axis {
            Axis::Horizontal => SplitAxis::Horizontal,
            Axis::Vertical => SplitAxis::Vertical,
        }
    }

    fn to_axis(self) -> Axis {
        match self {
            SplitAxis::Horizontal => Axis::Horizontal,
            SplitAxis::Vertical => Axis::Vertical,
        }
    }
}

/// A tab-stack snapshot: the ordered panel handles plus the active index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub tabs: Vec<PanelHandle>,
    pub active: usize,
}

/// The center pane-grid tree, mirroring [`pane_grid::Node`]: an interior
/// `Split` node or a leaf `Pane` carrying its tab stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CenterNode {
    Split {
        axis: SplitAxis,
        ratio: f32,
        a: Box<CenterNode>,
        b: Box<CenterNode>,
    },
    Pane(PaneSnapshot),
}

/// One edge dock's persisted state: its tab stack plus open/collapsed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockSnapshot {
    pub tabs: PaneSnapshot,
    pub open: bool,
}

/// The persisted focus location. Center focus is stored as a *leaf
/// ordinal* (the Nth pane in depth-first, a-before-b order) rather than
/// a raw `Pane` id, because ids are runtime-assigned and not stable
/// across a rebuild. Dock focus stores the side directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusSnapshot {
    Center(usize),
    Dock(DockSide),
}

/// The full, serializable workspace layout handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    pub center: CenterNode,
    pub left: DockSnapshot,
    pub right: DockSnapshot,
    pub bottom: DockSnapshot,
    pub focus: FocusSnapshot,
}

// ---- capture (live Workspace -> snapshot) --------------------------------

/// Capture the workspace's current layout as a handle-only snapshot.
pub fn capture(workspace: &Workspace) -> LayoutSnapshot {
    let layout = workspace.panes.layout();
    let center = capture_node(layout, &workspace.panes);

    let mut leaves = Vec::new();
    collect_leaves(layout, &mut leaves);

    let focus = match workspace.focused {
        PanelLocation::Center(pane) => {
            let ordinal = leaves.iter().position(|p| *p == pane).unwrap_or(0);
            FocusSnapshot::Center(ordinal)
        }
        PanelLocation::Dock(side) => FocusSnapshot::Dock(side),
    };

    LayoutSnapshot {
        center,
        left: capture_dock(&workspace.docks.left),
        right: capture_dock(&workspace.docks.right),
        bottom: capture_dock(&workspace.docks.bottom),
        focus,
    }
}

fn capture_node(node: &Node, panes: &pane_grid::State<PaneState>) -> CenterNode {
    match node {
        Node::Split {
            axis, ratio, a, b, ..
        } => CenterNode::Split {
            axis: SplitAxis::from_axis(*axis),
            ratio: *ratio,
            a: Box::new(capture_node(a, panes)),
            b: Box::new(capture_node(b, panes)),
        },
        Node::Pane(pane) => {
            // The layout tree and the pane map are kept in lockstep by
            // `pane_grid::State`, so a leaf always has backing state.
            let pane_state = panes
                .get(*pane)
                .expect("layout leaf must have backing pane state");
            CenterNode::Pane(capture_pane(pane_state))
        }
    }
}

/// Depth-first, a-before-b leaf collection. Defines the ordinal space
/// the focus snapshot indexes into; it matches the order in which
/// `pane_grid` assigns (monotonically increasing) leaf ids, so the Nth
/// leaf here is the Nth-smallest `Pane` id after a rebuild.
fn collect_leaves(node: &Node, out: &mut Vec<pane_grid::Pane>) {
    match node {
        Node::Split { a, b, .. } => {
            collect_leaves(a, out);
            collect_leaves(b, out);
        }
        Node::Pane(pane) => out.push(*pane),
    }
}

fn capture_pane(pane_state: &PaneState) -> PaneSnapshot {
    PaneSnapshot {
        tabs: pane_state
            .tabs
            .iter()
            .map(|panel| PanelHandle {
                kind: panel.kind(),
                snapshot: panel.snapshot(),
            })
            .collect(),
        active: pane_state.active,
    }
}

fn capture_dock(dock: &Dock) -> DockSnapshot {
    DockSnapshot {
        tabs: capture_pane(&dock.tabs),
        open: dock.open,
    }
}

// ---- restore (snapshot -> live Workspace) --------------------------------

/// Rebuild a [`Workspace`] from a snapshot. Each panel is rehydrated
/// through the registry from its handle; an unknown kind (no registered
/// constructor) is dropped, and the active index is clamped to the
/// surviving tabs so it never dangles.
pub fn restore(
    snapshot: &LayoutSnapshot,
    registry: &PanelRegistry,
    theme_mode: ThemeMode,
) -> Workspace {
    let config = restore_config(&snapshot.center, registry);
    let panes = pane_grid::State::with_configuration(config);

    // `State::iter` yields panes in ascending id order, which equals the
    // depth-first leaf order the focus ordinal was captured against.
    let ordered: Vec<pane_grid::Pane> = panes.iter().map(|(pane, _)| *pane).collect();

    let focused = match snapshot.focus {
        FocusSnapshot::Center(ordinal) => {
            let pane = ordered
                .get(ordinal)
                .or_else(|| ordered.first())
                .copied()
                .expect("a restored center grid always has at least one pane");
            PanelLocation::Center(pane)
        }
        FocusSnapshot::Dock(side) => PanelLocation::Dock(side),
    };

    let docks = Docks {
        left: restore_dock(&snapshot.left, registry),
        right: restore_dock(&snapshot.right, registry),
        bottom: restore_dock(&snapshot.bottom, registry),
    };

    Workspace::from_parts(panes, docks, focused, theme_mode)
}

fn restore_config(node: &CenterNode, registry: &PanelRegistry) -> Configuration<PaneState> {
    match node {
        CenterNode::Split { axis, ratio, a, b } => Configuration::Split {
            axis: axis.to_axis(),
            ratio: *ratio,
            a: Box::new(restore_config(a, registry)),
            b: Box::new(restore_config(b, registry)),
        },
        CenterNode::Pane(snapshot) => Configuration::Pane(restore_pane(snapshot, registry)),
    }
}

fn restore_pane(snapshot: &PaneSnapshot, registry: &PanelRegistry) -> PaneState {
    let tabs: Vec<Box<dyn Panel>> = snapshot
        .tabs
        .iter()
        .filter_map(|handle| registry.build(handle.kind, handle.snapshot.clone()))
        .collect();

    let mut pane_state = PaneState::new(tabs);
    if snapshot.active < pane_state.tabs.len() {
        pane_state.active = snapshot.active;
    }
    pane_state
}

fn restore_dock(snapshot: &DockSnapshot, registry: &PanelRegistry) -> Dock {
    Dock {
        tabs: restore_pane(&snapshot.tabs, registry),
        open: snapshot.open,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
    use crate::workspace::command::Command;
    use crate::workspace::location::DockSide;

    fn test_registry() -> PanelRegistry {
        let mut registry = PanelRegistry::new();
        registry
            .register(PanelKind::Counter, |s| {
                Box::new(CounterPanel::from_snapshot(s))
            })
            .register(PanelKind::Text, |s| Box::new(TextPanel::from_snapshot(s)))
            .register(PanelKind::Clock, |s| Box::new(ClockPanel::from_snapshot(s)));
        registry
    }

    fn seeded_workspace() -> Workspace {
        let center = PaneState::new(vec![
            Box::new(CounterPanel::new()),
            Box::new(TextPanel::new()),
            Box::new(ClockPanel::new()),
        ]);
        let docks = Docks::new(
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::new(vec![Box::new(CounterPanel::new())]),
            PaneState::new(vec![Box::new(TextPanel::new())]),
        );
        Workspace::with_docks(center, docks, ThemeMode::Dark)
    }

    #[test]
    fn per_panel_handle_round_trips_through_registry() {
        let registry = test_registry();

        let mut counter = CounterPanel::new();
        counter.update(crate::workspace::panel::erase(
            crate::features::dummies::counter::CounterMessage::Increment,
        ));
        let handle = PanelHandle {
            kind: counter.kind(),
            snapshot: counter.snapshot(),
        };

        let rebuilt = registry
            .build(handle.kind, handle.snapshot.clone())
            .expect("counter kind is registered");
        assert_eq!(rebuilt.kind(), PanelKind::Counter);
        assert_eq!(rebuilt.snapshot(), handle.snapshot);
    }

    #[test]
    fn json_round_trip_preserves_structure() {
        let workspace = seeded_workspace();
        let snapshot = capture(&workspace);

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: LayoutSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn capture_restore_capture_is_structurally_equal() {
        let mut workspace = seeded_workspace();
        // Build a non-trivial layout: a split, a focus move, an open dock.
        workspace.apply_command(Command::SplitFocused);
        workspace.apply_command(Command::ToggleDock(DockSide::Right));

        let first = capture(&workspace);
        let restored = restore(&first, &test_registry(), ThemeMode::Dark);
        let second = capture(&restored);

        assert_eq!(first, second);
    }

    #[test]
    fn restore_rehydrates_panel_content() {
        let mut workspace = seeded_workspace();
        // Drive the first center tab (a Counter) up to 3.
        if let PanelLocation::Center(pane) = workspace.focused {
            let counter = workspace.panes.get_mut(pane).unwrap().tabs[0].as_mut();
            for _ in 0..3 {
                counter.update(crate::workspace::panel::erase(
                    crate::features::dummies::counter::CounterMessage::Increment,
                ));
            }
        }

        let snapshot = capture(&workspace);
        let restored = restore(&snapshot, &test_registry(), ThemeMode::Dark);

        let pane = match restored.focused {
            PanelLocation::Center(pane) => pane,
            _ => panic!("expected center focus"),
        };
        let count = restored.panes.get(pane).unwrap().tabs[0]
            .snapshot()
            .get("count")
            .and_then(|v| v.as_i64())
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn restore_preserves_dock_open_state_and_focus() {
        let mut workspace = seeded_workspace();
        workspace.apply_command(Command::ToggleDock(DockSide::Right));

        let snapshot = capture(&workspace);
        let restored = restore(&snapshot, &test_registry(), ThemeMode::Dark);

        assert!(restored.docks.right.open);
        assert_eq!(restored.focused, PanelLocation::Dock(DockSide::Right));
    }

    #[test]
    fn restore_clamps_out_of_range_active_index() {
        let snapshot = PaneSnapshot {
            tabs: vec![PanelHandle {
                kind: PanelKind::Counter,
                snapshot: serde_json::json!({ "count": 0 }),
            }],
            active: 99,
        };
        let pane = restore_pane(&snapshot, &test_registry());
        assert_eq!(pane.active, 0);
    }
}
