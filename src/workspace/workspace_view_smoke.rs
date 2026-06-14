//! View-layer smoke probes — read shell visibility from [`Workspace`] state
//! without rendering Iced widgets.
//!
//! These helpers mirror the decisions in [`crate::workspace::workspace_view`]
//! so unit tests can assert chrome behavior from reducer state alone.

use crate::workspace::workspace_dock::{Dock, DockVisibility, Docks};
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_pane_state::PaneState;
use crate::workspace::workspace_panel::{Panel, StatusSink};
use crate::workspace::workspace_state::Workspace;

/// Captured shell visibility derived from workspace state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellProbe {
    pub docks: DockShellProbe,
    pub overlays: OverlayProbe,
    pub tab_chrome: TabChromeProbe,
    pub status_bar: StatusBarProbe,
    pub dock_strip: DockStripProbe,
}

/// Per-dock layout visibility matching [`workspace_view`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockShellProbe {
    pub left: DockFaceProbe,
    pub right: DockFaceProbe,
    pub bottom: DockFaceProbe,
}

/// Whether a dock renders its body, rail, or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockFaceProbe {
    pub visibility: DockVisibility,
    pub layout_visible: bool,
    pub body_visible: bool,
    pub rail_visible: bool,
}

/// Transient overlays stacked above the base shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayProbe {
    pub drop_overlay: bool,
    pub palette_open: bool,
    pub palette_has_backdrop: bool,
    pub close_confirmation: bool,
}

/// Tab-strip chrome flags mirrored from the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabChromeProbe {
    pub drag_source_hidden: Option<(PanelLocation, usize)>,
    pub hovered_tab: Option<(PanelLocation, usize)>,
}

/// Status-bar focus segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarProbe {
    pub focus_segment: String,
}

/// Dock tab-strip layout controls (one per dock side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockStripProbe {
    pub activity: DockStripControl,
    pub conversation: DockStripControl,
    pub output: DockStripControl,
}

/// Collapse and hide controls rendered in an open dock's tab strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockStripControl {
    pub collapse_visible: bool,
    pub hide_visible: bool,
    pub collapse_enabled: bool,
    pub hide_enabled: bool,
    pub collapse_action: CollapseControlAction,
    pub hide_action: HideControlAction,
}

/// Press action exposed by the collapse control when enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapseControlAction {
    Collapse,
    Open,
    Disabled,
}

/// Press action exposed by the hide control when enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideControlAction {
    Hide,
    Disabled,
}

/// Build a [`ShellProbe`] from the current workspace state.
pub fn probe_shell(workspace: &Workspace) -> ShellProbe {
    ShellProbe {
        docks: probe_docks(&workspace.docks),
        overlays: OverlayProbe {
            drop_overlay: workspace.drag_state.is_some()
                || workspace.cross_window_drop_preview.is_some(),
            palette_open: workspace.palette.open,
            palette_has_backdrop: false,
            close_confirmation: workspace.close_confirmation.is_some(),
        },
        tab_chrome: TabChromeProbe {
            drag_source_hidden: workspace
                .drag_state
                .as_ref()
                .map(|drag| (drag.source_location, drag.source_tab)),
            hovered_tab: workspace.hovered_tab,
        },
        status_bar: probe_status_bar(workspace),
        dock_strip: probe_dock_strip(workspace),
    }
}

/// Tab label text, including the dirty marker used in tab strips.
pub fn tab_display_title(panel: &dyn Panel) -> String {
    if panel.is_dirty() {
        format!("• {}", panel.title())
    } else {
        panel.title().to_string()
    }
}

/// Whether the close button is shown on a tab (active or hovered).
pub fn tab_close_visible(workspace: &Workspace, location: PanelLocation, tab: usize) -> bool {
    let Some(pane_state) = pane_state_at(workspace, location) else {
        return false;
    };
    if tab >= pane_state.tabs.len() {
        return false;
    }

    let active = tab == pane_state.active;
    let hovered = workspace.hovered_tab == Some((location, tab));
    active || hovered
}

/// Visible tab titles at `location`, omitting the tab currently being dragged.
pub fn tab_titles_at(workspace: &Workspace, location: PanelLocation) -> Vec<String> {
    let Some(pane_state) = pane_state_at(workspace, location) else {
        return Vec::new();
    };

    let drag_source_tab = workspace
        .drag_state
        .as_ref()
        .and_then(|drag| (drag.source_location == location).then_some(drag.source_tab));

    pane_state
        .tabs
        .iter()
        .enumerate()
        .filter(|(index, _)| drag_source_tab != Some(*index))
        .map(|(_, panel)| tab_display_title(panel.as_ref()))
        .collect()
}

fn probe_docks(docks: &Docks) -> DockShellProbe {
    DockShellProbe {
        left: probe_dock_face(docks.get(DockSide::Left)),
        right: probe_dock_face(docks.get(DockSide::Right)),
        bottom: probe_dock_face(docks.get(DockSide::Bottom)),
    }
}

fn probe_dock_face(dock: &Dock) -> DockFaceProbe {
    let hidden_or_empty = dock.is_empty() || dock.is_hidden();
    DockFaceProbe {
        visibility: dock.visibility,
        layout_visible: !hidden_or_empty,
        body_visible: dock.is_open() && !hidden_or_empty,
        rail_visible: dock.is_collapsed() && !hidden_or_empty,
    }
}

fn probe_status_bar(workspace: &Workspace) -> StatusBarProbe {
    StatusBarProbe {
        focus_segment: probe_focus_segment(workspace),
    }
}

fn probe_dock_strip(workspace: &Workspace) -> DockStripProbe {
    DockStripProbe {
        activity: probe_dock_control(workspace, DockSide::Left),
        conversation: probe_dock_control(workspace, DockSide::Right),
        output: probe_dock_control(workspace, DockSide::Bottom),
    }
}

fn probe_focus_segment(workspace: &Workspace) -> String {
    let segment_first = match workspace.focused {
        PanelLocation::Center(pane) => {
            let active_title = workspace
                .panes
                .get(pane)
                .and_then(|pane_state| pane_state.active_panel())
                .map(|panel| panel.title())
                .unwrap_or_else(|| std::borrow::Cow::Borrowed("—"));
            format!("Focus: Center / {active_title}")
        }
        PanelLocation::Dock(side) => {
            let dock = workspace.docks.get(side);
            let active_title = dock
                .tabs
                .active_panel()
                .map(|panel| panel.title())
                .unwrap_or_else(|| std::borrow::Cow::Borrowed("—"));
            format!("Focus: {} / {active_title}", side.label())
        }
    };

    let mut segments = vec![std::borrow::Cow::Owned(segment_first)];

    let active_panel = match workspace.focused {
        PanelLocation::Center(pane) => workspace
            .panes
            .get(pane)
            .and_then(|pane_state| pane_state.tabs.get(pane_state.active)),
        PanelLocation::Dock(side) => {
            let dock = workspace.docks.get(side);
            dock.tabs.tabs.get(dock.tabs.active)
        }
    };

    if let Some(panel) = active_panel {
        let mut sink = StatusSink::new(&mut segments);
        panel.status_contribution(&mut sink);
    }

    segments
        .iter()
        .map(|segment| segment.as_ref())
        .collect::<Vec<_>>()
        .join("   ")
}

fn probe_dock_control(workspace: &Workspace, side: DockSide) -> DockStripControl {
    let dock = workspace.docks.get(side);

    if dock.visibility != DockVisibility::Open {
        return DockStripControl {
            collapse_visible: false,
            hide_visible: false,
            collapse_enabled: false,
            hide_enabled: false,
            collapse_action: CollapseControlAction::Disabled,
            hide_action: HideControlAction::Disabled,
        };
    }

    DockStripControl {
        collapse_visible: true,
        hide_visible: true,
        collapse_enabled: true,
        hide_enabled: true,
        collapse_action: CollapseControlAction::Collapse,
        hide_action: HideControlAction::Hide,
    }
}

fn pane_state_at(workspace: &Workspace, location: PanelLocation) -> Option<&PaneState> {
    match location {
        PanelLocation::Center(pane) => workspace.panes.get(pane),
        PanelLocation::Dock(side) => Some(&workspace.docks.get(side).tabs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::ScratchMessage;
    use crate::features::ScratchPanel;
    use crate::features::dummies::{ClockPanel, CounterPanel, TextPanel};
    use crate::shared::design::ThemeMode;
    use crate::workspace::workspace_command::Command;
    use crate::workspace::workspace_dock::DockVisibility;
    use crate::workspace::workspace_message::WorkspaceMessage;
    use crate::workspace::workspace_panel::erase;
    use crate::workspace::workspace_state::Workspace;
    use crate::workspace::workspace_stores::AppStores;
    use iced::widget::text_editor;

    fn three_tab_workspace() -> (Workspace, AppStores) {
        let mut stores = AppStores::new();
        let tabs: Vec<Box<dyn Panel>> = vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
        ];
        let workspace = Workspace::single_pane(PaneState::new(tabs), ThemeMode::Dark);
        (workspace, stores)
    }

    fn workspace_with_right_dock() -> (Workspace, AppStores) {
        let mut stores = AppStores::new();
        let center = PaneState::new(vec![
            Box::new(CounterPanel::new(&mut stores)),
            Box::new(TextPanel::new()),
        ]);
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(ClockPanel::new())]),
            PaneState::empty(),
        );
        let workspace = Workspace::with_docks(center, docks, ThemeMode::Dark);
        (workspace, stores)
    }

    fn only_center_location(workspace: &Workspace) -> PanelLocation {
        let pane = *workspace.panes.iter().next().unwrap().0;
        PanelLocation::Center(pane)
    }

    #[test]
    fn default_workspace_has_hidden_docks_and_no_overlays() {
        let (workspace, _stores) = three_tab_workspace();
        let probe = probe_shell(&workspace);

        assert_eq!(probe.docks.left.visibility, DockVisibility::Hidden);
        assert!(!probe.docks.left.layout_visible);
        assert!(!probe.overlays.drop_overlay);
        assert!(!probe.overlays.palette_open);
        assert!(!probe.overlays.palette_has_backdrop);
        assert!(!probe.overlays.close_confirmation);
        assert!(probe.tab_chrome.drag_source_hidden.is_none());
        assert!(probe.tab_chrome.hovered_tab.is_none());
        assert_eq!(probe.status_bar.focus_segment, "Focus: Center / Counter");
        assert!(!probe.dock_strip.conversation.collapse_visible);
        assert!(!probe.dock_strip.conversation.hide_visible);
    }

    #[test]
    fn open_dock_renders_body_not_rail() {
        let (mut workspace, mut stores) = workspace_with_right_dock();
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);

        let probe = probe_shell(&workspace);

        assert!(probe.docks.right.body_visible);
        assert!(!probe.docks.right.rail_visible);
        assert_eq!(probe.docks.right.visibility, DockVisibility::Open);

        let controls = &probe.dock_strip.conversation;
        assert!(controls.collapse_visible);
        assert!(controls.hide_visible);
        assert!(controls.collapse_enabled);
        assert!(controls.hide_enabled);
        assert_eq!(controls.collapse_action, CollapseControlAction::Collapse);
        assert_eq!(controls.hide_action, HideControlAction::Hide);
    }

    #[test]
    fn collapsed_dock_renders_rail_not_body() {
        let (mut workspace, _stores) = workspace_with_right_dock();
        workspace
            .docks
            .set_visibility(DockSide::Right, DockVisibility::Collapsed);

        let probe = probe_shell(&workspace);

        assert!(!probe.docks.right.body_visible);
        assert!(probe.docks.right.rail_visible);

        let controls = &probe.dock_strip.conversation;
        assert!(!controls.collapse_visible);
        assert!(!controls.hide_visible);
        assert_eq!(controls.collapse_action, CollapseControlAction::Disabled);
        assert_eq!(controls.hide_action, HideControlAction::Disabled);
    }

    #[test]
    fn tab_display_title_matches_dirty_dot_logic() {
        let panel = TextPanel::new();
        assert_eq!(tab_display_title(&panel), "Text");

        let mut dirty = ScratchPanel::new();
        dirty.update(
            erase(ScratchMessage::Edit(text_editor::Action::Edit(
                text_editor::Edit::Insert('x'),
            ))),
            &mut AppStores::new(),
        );
        assert_eq!(tab_display_title(&dirty), "• untitled");
    }

    #[test]
    fn tab_close_visible_for_active_tab() {
        let (workspace, _stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        assert!(tab_close_visible(&workspace, location, 0));
        assert!(!tab_close_visible(&workspace, location, 1));
    }

    #[test]
    fn tab_close_visible_for_hovered_tab() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabSelected { location, tab: 1 },
            &mut stores,
        );
        workspace.update(
            WorkspaceMessage::TabHoverChanged {
                location: Some((location, 0)),
            },
            &mut stores,
        );

        assert!(tab_close_visible(&workspace, location, 0));
        assert!(tab_close_visible(&workspace, location, 1));
    }

    #[test]
    fn palette_open_is_reflected_in_overlay_probe() {
        let (mut workspace, mut stores) = three_tab_workspace();
        workspace.all_commands =
            crate::workspace::workspace_command_palette::default_command_items();
        workspace.update(WorkspaceMessage::TogglePalette, &mut stores);

        let probe = probe_shell(&workspace);

        assert!(probe.overlays.palette_open);
        assert!(!probe.overlays.palette_has_backdrop);
        assert!(!probe.overlays.close_confirmation);
    }

    #[test]
    fn close_confirmation_is_reflected_in_overlay_probe() {
        let mut stores = AppStores::new();
        let mut workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            ThemeMode::Dark,
        );
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::Panel {
                location,
                tab: 0,
                message: erase(ScratchMessage::Edit(text_editor::Action::Edit(
                    text_editor::Edit::Insert('a'),
                ))),
            },
            &mut stores,
        );
        workspace.update(
            WorkspaceMessage::TabCloseRequested { location, tab: 0 },
            &mut stores,
        );

        let probe = probe_shell(&workspace);

        assert!(probe.overlays.close_confirmation);
        assert!(!probe.overlays.palette_open);
        assert!(!probe.overlays.palette_has_backdrop);
    }

    #[test]
    fn drag_source_is_hidden_from_tab_titles() {
        let (mut workspace, _stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut AppStores::new(),
        );

        let probe = probe_shell(&workspace);
        assert_eq!(probe.tab_chrome.drag_source_hidden, Some((location, 0)));
        assert_eq!(
            tab_titles_at(&workspace, location),
            vec!["Text".to_string()]
        );
    }

    #[test]
    fn hovered_tab_is_reflected_in_tab_chrome_probe() {
        let (mut workspace, mut stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabHoverChanged {
                location: Some((location, 1)),
            },
            &mut stores,
        );

        let probe = probe_shell(&workspace);
        assert_eq!(probe.tab_chrome.hovered_tab, Some((location, 1)));
    }

    #[test]
    fn empty_hidden_dock_strip_controls_are_not_visible() {
        let (workspace, _stores) = three_tab_workspace();
        let probe = probe_shell(&workspace);

        let controls = &probe.dock_strip.conversation;
        assert!(!controls.collapse_visible);
        assert!(!controls.hide_visible);
        assert!(!controls.collapse_enabled);
        assert!(!controls.hide_enabled);
        assert_eq!(controls.collapse_action, CollapseControlAction::Disabled);
        assert_eq!(controls.hide_action, HideControlAction::Disabled);
    }

    #[test]
    fn drop_overlay_active_while_dragging() {
        let (mut workspace, _stores) = three_tab_workspace();
        let location = only_center_location(&workspace);

        workspace.update(
            WorkspaceMessage::TabDragStarted { location, tab: 0 },
            &mut AppStores::new(),
        );

        let probe = probe_shell(&workspace);
        assert!(probe.overlays.drop_overlay);
    }
}
