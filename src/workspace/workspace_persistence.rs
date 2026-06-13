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
//! [`crate::workspace::workspace_layout_store`].

use iced::widget::pane_grid::{self, Axis, Configuration, Node};
use serde::{Deserialize, Serialize};

use crate::shared::design::ThemeMode;
use crate::workspace::workspace_dock::{Dock, DockVisibility, Docks};
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_pane_state::PaneState;
use crate::workspace::workspace_panel::{Panel, PanelKind};
use crate::workspace::workspace_registry::PanelRegistry;
use crate::workspace::workspace_state::Workspace;
use crate::workspace::workspace_stores::AppStores;

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

/// One edge dock's persisted state: its tab stack plus tri-state visibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockSnapshot {
    pub tabs: PaneSnapshot,
    pub visibility: DockVisibility,
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
///
/// Each panel's [`Panel::snapshot`] reads from `stores` so a Counter
/// panel persists the canonical store count rather than a stale local
/// copy. The capture itself stays pure — only handles cross the
/// boundary, never panel content or store interiors.
pub fn capture(workspace: &Workspace, stores: &AppStores) -> LayoutSnapshot {
    let layout = workspace.panes.layout();
    let center = capture_node(layout, &workspace.panes, stores);

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
        left: capture_dock(&workspace.docks.left, stores),
        right: capture_dock(&workspace.docks.right, stores),
        bottom: capture_dock(&workspace.docks.bottom, stores),
        focus,
    }
}

fn capture_node(
    node: &Node,
    panes: &pane_grid::State<PaneState>,
    stores: &AppStores,
) -> CenterNode {
    match node {
        Node::Split {
            axis, ratio, a, b, ..
        } => CenterNode::Split {
            axis: SplitAxis::from_axis(*axis),
            ratio: *ratio,
            a: Box::new(capture_node(a, panes, stores)),
            b: Box::new(capture_node(b, panes, stores)),
        },
        Node::Pane(pane) => {
            // The layout tree and the pane map are kept in lockstep by
            // `pane_grid::State`, so a leaf always has backing state.
            let pane_state = panes
                .get(*pane)
                .expect("layout leaf must have backing pane state");
            CenterNode::Pane(capture_pane(pane_state, stores))
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

fn capture_pane(pane_state: &PaneState, stores: &AppStores) -> PaneSnapshot {
    let mut tabs = Vec::with_capacity(pane_state.tabs.len());
    // Track the surviving index of the originally-focused tab: count
    // how many durable tabs preceded `pane_state.active`. When the
    // focused tab itself was non-durable, the clamp falls back to the
    // last surviving panel before it (saturating sub 1), so active
    // never dangles.
    let mut new_active: usize = 0;
    for (i, panel) in pane_state.tabs.iter().enumerate() {
        if let Some(snapshot) = panel.snapshot(stores) {
            if i < pane_state.active {
                new_active += 1;
            } else if i == pane_state.active {
                // Focused tab survived: new_active already counts the
                // preceding survivors, which is exactly its new index.
            }
            tabs.push(PanelHandle {
                kind: panel.kind(),
                snapshot,
            });
        } else if i == pane_state.active {
            // Focused tab is non-durable; fall back to the last
            // surviving tab before it (or 0 if none survived).
            new_active = new_active.saturating_sub(1);
        }
    }
    // If every tab was non-durable, collapse active to 0.
    if tabs.is_empty() {
        new_active = 0;
    } else {
        new_active = new_active.min(tabs.len() - 1);
    }
    PaneSnapshot {
        tabs,
        active: new_active,
    }
}

fn capture_dock(dock: &Dock, stores: &AppStores) -> DockSnapshot {
    DockSnapshot {
        tabs: capture_pane(&dock.tabs, stores),
        visibility: dock.visibility,
    }
}

// ---- restore (snapshot -> live Workspace) --------------------------------

/// Rebuild a [`Workspace`] from a snapshot. Each panel is rehydrated
/// through the registry from its handle (which may allocate a fresh
/// store slot — e.g. a [`crate::workspace::workspace_stores::CounterId`] seeded
/// at the persisted count); an unknown kind (no registered constructor)
/// is dropped, and the active index is clamped to the surviving tabs
/// so it never dangles.
pub fn restore(
    snapshot: &LayoutSnapshot,
    registry: &PanelRegistry,
    stores: &mut AppStores,
    theme_mode: ThemeMode,
) -> Workspace {
    let config = restore_config(&snapshot.center, registry, stores);
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
        left: restore_dock(&snapshot.left, registry, stores),
        right: restore_dock(&snapshot.right, registry, stores),
        bottom: restore_dock(&snapshot.bottom, registry, stores),
    };

    Workspace::from_parts(panes, docks, focused, theme_mode)
}

fn restore_config(
    node: &CenterNode,
    registry: &PanelRegistry,
    stores: &mut AppStores,
) -> Configuration<PaneState> {
    match node {
        CenterNode::Split { axis, ratio, a, b } => Configuration::Split {
            axis: axis.to_axis(),
            ratio: *ratio,
            a: Box::new(restore_config(a, registry, stores)),
            b: Box::new(restore_config(b, registry, stores)),
        },
        CenterNode::Pane(snapshot) => Configuration::Pane(restore_pane(snapshot, registry, stores)),
    }
}

fn restore_pane(
    snapshot: &PaneSnapshot,
    registry: &PanelRegistry,
    stores: &mut AppStores,
) -> PaneState {
    let tabs: Vec<Box<dyn Panel>> = snapshot
        .tabs
        .iter()
        .filter_map(|handle| registry.build(handle.kind, handle.snapshot.clone(), stores))
        .collect();

    let mut pane_state = PaneState::new(tabs);
    if snapshot.active < pane_state.tabs.len() {
        pane_state.active = snapshot.active;
    }
    pane_state
}

fn restore_dock(snapshot: &DockSnapshot, registry: &PanelRegistry, stores: &mut AppStores) -> Dock {
    Dock {
        tabs: restore_pane(&snapshot.tabs, registry, stores),
        visibility: snapshot.visibility,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::ScratchPanel;
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
    use crate::workspace::workspace_command::Command;
    use crate::workspace::workspace_location::DockSide;
    use crate::workspace::workspace_stores::AppStores;

    fn test_registry() -> PanelRegistry {
        let mut registry = PanelRegistry::new();
        registry
            .register(PanelKind::Counter, |s, stores| {
                Box::new(CounterPanel::from_snapshot(s, stores))
            })
            .register(PanelKind::Text, |s, _stores| {
                Box::new(TextPanel::from_snapshot(s))
            })
            .register(PanelKind::Clock, |s, stores| {
                Box::new(ClockPanel::from_snapshot(s, stores))
            });
        registry
    }

    fn seeded_workspace() -> (Workspace, AppStores) {
        let mut stores = AppStores::new();
        let center = PaneState::new(vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
            Box::new(ClockPanel::new()),
        ]);
        let docks = Docks::new(
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::new(vec![Box::new(CounterPanel::new(&mut stores))]),
            PaneState::new(vec![Box::new(TextPanel::new())]),
        );
        let workspace = Workspace::with_docks(center, docks, ThemeMode::Dark);
        (workspace, stores)
    }

    #[test]
    fn per_panel_handle_round_trips_through_registry() {
        let registry = test_registry();
        let mut stores = AppStores::new();

        let counter = CounterPanel::new(&mut stores);
        // Drive the canonical store value, then snapshot through it.
        stores.counter.increment(counter.id());
        let handle = PanelHandle {
            kind: counter.kind(),
            snapshot: counter.snapshot(&stores).expect("durable"),
        };

        let mut rebuild_stores = AppStores::new();
        let rebuilt = registry
            .build(handle.kind, handle.snapshot.clone(), &mut rebuild_stores)
            .expect("counter kind is registered");
        assert_eq!(rebuilt.kind(), PanelKind::Counter);
        assert_eq!(rebuilt.snapshot(&rebuild_stores), Some(handle.snapshot));
    }

    #[test]
    fn json_round_trip_preserves_structure() {
        let (workspace, stores) = seeded_workspace();
        let snapshot = capture(&workspace, &stores);

        let json = serde_json::to_string(&snapshot).expect("serialize");
        let decoded: LayoutSnapshot = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn capture_restore_capture_is_structurally_equal() {
        let (mut workspace, mut stores) = seeded_workspace();
        // Build a non-trivial layout: a split, a focus move, an open dock.
        workspace.apply_command(Command::SplitFocused, &mut stores);
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        let first = capture(&workspace, &stores);
        let mut restored_stores = AppStores::new();
        let restored = restore(
            &first,
            &test_registry(),
            &mut restored_stores,
            ThemeMode::Dark,
        );
        let second = capture(&restored, &restored_stores);

        assert_eq!(first, second);
    }

    #[test]
    fn restore_rehydrates_panel_content() {
        let (workspace, mut stores) = seeded_workspace();
        // Drive the first center tab (a Counter) up to 3 through the
        // store — that's the source of truth that capture reads from.
        if let PanelLocation::Center(pane) = workspace.focused {
            // Re-borrow stores after we drop the immutable borrow on workspace.
            let _ = pane;
        }
        // Mutate via store directly: the panel is a view, so mutating the
        // store has the same effect as routing three intents.
        let counter_id = {
            let _pane_state = workspace.panes.iter().next().unwrap().1;
            // The Counter is at index 0 of the center pane; recover its
            // id from the live store by reading what its snapshot
            // currently addresses.
            // The CounterPanel test exposed `id()`, but here we don't
            // have the concrete type. Instead, drive every live counter:
            // there's only one counter in the center tab.
            0u64
        };
        let _ = counter_id;
        // The seeded workspace allocated counter id 0 in the center and
        // counter id 1 in the right dock. Drive id 0 (center tab).
        for _ in 0..3 {
            stores.counter.increment(0);
        }

        let snapshot = capture(&workspace, &stores);
        let mut restored_stores = AppStores::new();
        let restored = restore(
            &snapshot,
            &test_registry(),
            &mut restored_stores,
            ThemeMode::Dark,
        );

        let pane = match restored.focused {
            PanelLocation::Center(pane) => pane,
            _ => panic!("expected center focus"),
        };
        let count = restored.panes.get(pane).unwrap().tabs[0]
            .snapshot(&restored_stores)
            .expect("durable")
            .get("count")
            .and_then(|v| v.as_i64())
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn restore_preserves_dock_open_state_and_focus() {
        let (mut workspace, mut stores) = seeded_workspace();
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        let snapshot = capture(&workspace, &stores);
        let mut restored_stores = AppStores::new();
        let restored = restore(
            &snapshot,
            &test_registry(),
            &mut restored_stores,
            ThemeMode::Dark,
        );

        assert!(restored.docks.right.is_open());
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
        let mut stores = AppStores::new();
        let pane = restore_pane(&snapshot, &test_registry(), &mut stores);
        assert_eq!(pane.active, 0);
    }

    #[test]
    fn scratch_filtered_active_clamped_to_surviving_durable() {
        let stores = AppStores::new();
        // [Scratch, Durable1, Durable2], active=1 (Durable1 focused)
        let panes = pane_grid::State::new(PaneState::new(vec![
            Box::new(ScratchPanel::new()),
            Box::new(TextPanel::new()),
            Box::new(TextPanel::new()),
        ]));
        let mut panes = panes.0;
        let (_pane, pane_state) = panes.iter_mut().next().unwrap();
        pane_state.active = 1;
        let snap = capture_pane(pane_state, &stores);
        // Scratch dropped; active=1 should map to surviving index 0
        assert_eq!(snap.tabs.len(), 2);
        assert_eq!(snap.active, 0);
        assert_eq!(snap.tabs[0].kind, PanelKind::Text);
    }
}
