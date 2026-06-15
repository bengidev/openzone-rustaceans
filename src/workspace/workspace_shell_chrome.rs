//! Pure shell-chrome decisions shared by the workspace view and smoke tests.
//!
//! Keeps layout/styling contracts testable without pixel-perfect rendering.

use iced::Color;

use crate::shared::design::{BorderToken, ForegroundToken, OpenZoneTheme, SpacingToken};
use crate::workspace::workspace_dock::{Dock, DockVisibility};
use crate::workspace::workspace_layout_metrics;
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_state::Workspace;

/// Whether a tab close affordance should render for the given interaction state.
pub fn tab_close_visible(active: bool, hovered: bool) -> bool {
    active || hovered
}

/// Border color for a focused vs unfocused pane or dock body.
pub fn pane_border_color(theme: OpenZoneTheme, focused: bool) -> Color {
    if focused {
        theme.foreground(ForegroundToken::Accent)
    } else {
        theme.border(BorderToken::Default)
    }
}

/// Border width for pane and dock body outlines.
pub fn pane_border_width(_focused: bool) -> f32 {
    1.0
}

/// Active tab underline thickness.
pub const ACTIVE_TAB_UNDERLINE: f32 = 1.0;

/// Command palette card width.
pub const PALETTE_MAX_WIDTH: f32 = 440.0;

/// Layout contract for the command palette overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteOverlaySpec {
    pub has_visual_backdrop: bool,
    pub top_inset: f32,
    pub max_width: f32,
}

impl PaletteOverlaySpec {
    pub fn current() -> Self {
        Self {
            has_visual_backdrop: false,
            top_inset: workspace_layout_metrics::title_bar_height() + SpacingToken::S1.value(),
            max_width: PALETTE_MAX_WIDTH,
        }
    }
}

/// Whether drag/drop preview rectangles should be filled.
pub fn drop_preview_uses_fill() -> bool {
    false
}

/// What a dock renders in the shell for a given visibility state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockRenderRole {
    None,
    Rail,
    Body,
}

pub fn dock_render_role(dock: &Dock) -> DockRenderRole {
    if dock.is_empty() || dock.is_hidden() {
        DockRenderRole::None
    } else if dock.is_open() {
        DockRenderRole::Body
    } else {
        DockRenderRole::Rail
    }
}

pub fn dock_control_enabled(workspace: &Workspace, side: DockSide) -> bool {
    let dock = workspace.docks.get(side);
    !dock.is_empty() || workspace.has_dock_factory(side) || dock.is_open()
}

pub fn dirty_tab_title(title: &str) -> String {
    format!("• {title}")
}

pub fn dirty_tab_titles(workspace: &Workspace) -> Vec<String> {
    let mut titles = Vec::new();

    for (_, pane_state) in workspace.panes.iter() {
        for panel in &pane_state.tabs {
            if panel.is_dirty() {
                titles.push(dirty_tab_title(&panel.title()));
            }
        }
    }

    for side in DockSide::ALL {
        let dock = workspace.docks.get(side);
        for panel in &dock.tabs.tabs {
            if panel.is_dirty() {
                titles.push(dirty_tab_title(&panel.title()));
            }
        }
    }

    titles
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSmokeSnapshot {
    pub dock_roles: [(DockSide, DockRenderRole); 3],
    pub dock_controls_enabled: [(DockSide, bool); 3],
    pub dirty_tab_titles: Vec<String>,
    pub palette_open: bool,
    pub palette_has_visual_backdrop: bool,
    pub close_confirmation_open: bool,
    pub focused_uses_accent_border: bool,
    pub drop_preview_uses_fill: bool,
}

pub fn shell_smoke_snapshot(workspace: &Workspace) -> ShellSmokeSnapshot {
    ShellSmokeSnapshot {
        dock_roles: [
            (
                DockSide::Left,
                dock_render_role(workspace.docks.get(DockSide::Left)),
            ),
            (
                DockSide::Right,
                dock_render_role(workspace.docks.get(DockSide::Right)),
            ),
            (
                DockSide::Bottom,
                dock_render_role(workspace.docks.get(DockSide::Bottom)),
            ),
        ],
        dock_controls_enabled: [
            (
                DockSide::Left,
                dock_control_enabled(workspace, DockSide::Left),
            ),
            (
                DockSide::Right,
                dock_control_enabled(workspace, DockSide::Right),
            ),
            (
                DockSide::Bottom,
                dock_control_enabled(workspace, DockSide::Bottom),
            ),
        ],
        dirty_tab_titles: dirty_tab_titles(workspace),
        palette_open: workspace.palette.open,
        palette_has_visual_backdrop: PaletteOverlaySpec::current().has_visual_backdrop,
        close_confirmation_open: workspace.close_confirmation.is_some(),
        focused_uses_accent_border: focused_pane_uses_accent_border(workspace.theme),
        drop_preview_uses_fill: drop_preview_uses_fill(),
    }
}

pub fn tab_chip_underline_color(theme: OpenZoneTheme, active: bool) -> Color {
    if active {
        theme.foreground(ForegroundToken::Accent)
    } else {
        Color::TRANSPARENT
    }
}

pub fn ghost_tab_fill(_theme: OpenZoneTheme) -> Color {
    Color::TRANSPARENT
}

pub fn drop_target_fill(_accent: Color) -> Color {
    Color::TRANSPARENT
}

pub fn dock_strip_control_labels() -> (&'static str, &'static str) {
    ("▾", "×")
}

pub fn dock_control_color(theme: OpenZoneTheme, visibility: DockVisibility) -> Color {
    match visibility {
        DockVisibility::Open => theme.foreground(ForegroundToken::Accent),
        DockVisibility::Collapsed => theme.foreground(ForegroundToken::Secondary),
        DockVisibility::Hidden => theme.foreground(ForegroundToken::Muted),
    }
}

/// Whether focused pane/dock chrome uses accent borders per the shell contract.
pub fn focused_pane_uses_accent_border(theme: OpenZoneTheme) -> bool {
    pane_border_color(theme, true) == theme.foreground(ForegroundToken::Accent)
        && pane_border_width(true) == 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{ScratchMessage, ScratchPanel};
    use crate::shared::design::ThemeMode;
    use crate::workspace::workspace_command::Command;
    use crate::workspace::workspace_dock::{DockVisibility, Docks};
    use crate::workspace::workspace_message::WorkspaceMessage;
    use crate::workspace::workspace_pane_state::PaneState;
    use crate::workspace::workspace_state::Workspace;
    use crate::workspace::workspace_stores::AppStores;
    use crate::workspace::{DockSurfaceFactory, erase};
    use iced::widget::text_editor;

    fn theme() -> OpenZoneTheme {
        OpenZoneTheme::dark()
    }

    #[test]
    fn focused_pane_accent_border_contract() {
        assert!(focused_pane_uses_accent_border(theme()));
    }

    #[test]
    fn focused_border_uses_accent_color() {
        let accent = theme().foreground(ForegroundToken::Accent);
        assert_eq!(pane_border_color(theme(), true), accent);
        assert_ne!(pane_border_color(theme(), false), accent);
        assert_eq!(pane_border_width(true), 1.0);
    }

    #[test]
    fn tab_close_visible_on_active_or_hover_only() {
        assert!(tab_close_visible(true, false));
        assert!(tab_close_visible(false, true));
        assert!(!tab_close_visible(false, false));
    }

    #[test]
    fn palette_has_no_visual_backdrop_and_sits_below_title_bar() {
        let spec = PaletteOverlaySpec::current();
        assert!(!spec.has_visual_backdrop);
        assert!(spec.top_inset > workspace_layout_metrics::title_bar_height());
        assert_eq!(spec.max_width, PALETTE_MAX_WIDTH);
    }

    #[test]
    fn drop_preview_is_outline_only() {
        assert!(!drop_preview_uses_fill());
        assert_eq!(
            drop_target_fill(theme().foreground(ForegroundToken::Accent)),
            Color::TRANSPARENT
        );
        assert_eq!(ghost_tab_fill(theme()), Color::TRANSPARENT);
    }

    #[test]
    fn hidden_dock_renders_no_rail_or_body() {
        let workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            ThemeMode::Dark,
        );
        let snapshot = shell_smoke_snapshot(&workspace);
        assert_eq!(
            snapshot.dock_roles,
            [
                (DockSide::Left, DockRenderRole::None),
                (DockSide::Right, DockRenderRole::None),
                (DockSide::Bottom, DockRenderRole::None),
            ]
        );
    }

    #[test]
    fn open_dock_renders_body_role() {
        let mut stores = AppStores::new();
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(
                crate::features::dummies::clock::ClockPanel::new(),
            )]),
            PaneState::empty(),
        );
        let mut workspace = Workspace::with_docks(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            docks,
            ThemeMode::Dark,
        );
        workspace.apply_command(Command::OpenDock(DockSide::Right), &mut stores);
        let snapshot = shell_smoke_snapshot(&workspace);
        assert_eq!(
            snapshot.dock_roles[1],
            (DockSide::Right, DockRenderRole::Body)
        );
    }

    #[test]
    fn collapsed_dock_renders_rail_role() {
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(
                crate::features::dummies::clock::ClockPanel::new(),
            )]),
            PaneState::empty(),
        );
        let mut workspace = Workspace::with_docks(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            docks,
            ThemeMode::Dark,
        );
        workspace.docks.right.visibility = DockVisibility::Collapsed;
        let snapshot = shell_smoke_snapshot(&workspace);
        assert_eq!(
            snapshot.dock_roles[1],
            (DockSide::Right, DockRenderRole::Rail)
        );
    }

    #[test]
    fn dock_control_disabled_without_content_or_factory() {
        let workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            ThemeMode::Dark,
        );
        assert!(!dock_control_enabled(&workspace, DockSide::Left));
    }

    #[test]
    fn dock_control_enabled_with_factory() {
        let mut stores = AppStores::new();
        let mut workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(
                crate::features::dummies::counter::CounterPanel::new(&mut stores),
            )]),
            ThemeMode::Dark,
        );
        let factory: DockSurfaceFactory = |_| Box::new(ScratchPanel::new());
        workspace.set_dock_factory(DockSide::Left, factory);
        assert!(dock_control_enabled(&workspace, DockSide::Left));
    }

    #[test]
    fn dirty_tab_titles_include_dot_prefix() {
        let mut stores = AppStores::new();
        let mut workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            ThemeMode::Dark,
        );
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
        assert_eq!(dirty_tab_titles(&workspace), vec!["• untitled".to_string()]);
    }

    #[test]
    fn shell_snapshot_tracks_palette_and_close_confirmation_state() {
        let workspace = Workspace::single_pane(
            PaneState::new(vec![Box::new(ScratchPanel::new())]),
            ThemeMode::Dark,
        );
        let snapshot = shell_smoke_snapshot(&workspace);
        assert!(!snapshot.palette_open);
        assert!(!snapshot.palette_has_visual_backdrop);
        assert!(!snapshot.close_confirmation_open);
        assert!(snapshot.focused_uses_accent_border);
    }
}
