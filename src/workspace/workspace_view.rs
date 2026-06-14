#![allow(dead_code)]

//! Workspace view.
//!
//! Pure render over `&Workspace`. Composes the outer frame
//! (`column[title_bar, docks + center, status_bar]`) and, per pane or
//! dock, a tab strip above the active panel's content. Collapsed docks
//! render as minimal rails. All styling resolves through
//! `shared::design` tokens — no hardcoded colors or sizes.

use crate::shared::design::StatusToken;
use iced::widget::Space;

use crate::shared::design::{
    BackgroundToken, BorderToken, ForegroundToken, OpenZoneTheme, RadiusToken, SpacingToken,
    ThemeMode, TypeRole,
};
use crate::workspace::workspace_command::Command;
use crate::workspace::workspace_dock::{Dock, DockVisibility};
use crate::workspace::workspace_drag as drag;
use crate::workspace::workspace_layout_metrics::{self as layout_metrics, DOCK_RAIL_THICKNESS};
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_message::WorkspaceMessage;
use crate::workspace::workspace_pane_state::PaneState;
use crate::workspace::workspace_panel::{
    CloseRequest, ErasedMessage, Panel, PanelKind, StatusSink,
};
use crate::workspace::workspace_shell_chrome as shell_chrome;
use crate::workspace::workspace_state::{CloseConfirmation, Workspace};
use crate::workspace::workspace_stores::AppStores;
use iced::alignment::Horizontal;
use iced::widget::canvas::{Frame, Geometry, Program, Stroke};
use iced::widget::{
    Canvas, PaneGrid, button, canvas, column, container, mouse_area, pane_grid, row, scrollable,
    space, stack, text, text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Size, mouse,
};

/// Render the whole workspace shell as a view over `stores`.
///
/// Threading [`AppStores`] through the view tree is what lets Counter
/// and Clock panels render their canonical store value (view-over-
/// handle); panels addressing a store slice receive the same `&stores`
/// reference at every level so the render is consistent within a frame.
pub fn view<'a>(workspace: &'a Workspace, stores: &'a AppStores) -> Element<'a, WorkspaceMessage> {
    let theme = workspace.theme;

    let center = center_pane_grid(workspace, stores, theme);

    let main_row = row![
        dock_side(workspace, stores, theme, DockSide::Left),
        center,
        dock_side(workspace, stores, theme, DockSide::Right),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(SpacingToken::Hairline.value());

    let body = column![main_row, dock_bottom(workspace, stores, theme)]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(SpacingToken::Hairline.value());

    let framed = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SpacingToken::Hairline.value())
        .style(move |_| surface_style(theme, BackgroundToken::Primary));

    let shell = column![title_bar(theme), framed, status_bar(theme, workspace)]
        .width(Length::Fill)
        .height(Length::Fill);

    let decorated =
        if workspace.drag_state.is_some() || workspace.cross_window_drop_preview.is_some() {
            Element::from(
                stack![shell, drop_overlay(workspace, theme)]
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
        } else {
            shell.into()
        };

    // Palette overlay: top-centered dropdown below the title bar.
    let decorated = if workspace.palette.open {
        let overlay_elem = palette_overlay(theme, workspace);
        Element::from(
            stack![decorated, overlay_elem]
                .width(Length::Fill)
                .height(Length::Fill),
        )
    } else {
        decorated
    };

    if let Some(confirm) = &workspace.close_confirmation {
        let (title, list_items, discard_message) = match confirm {
            CloseConfirmation::Tab {
                message,
                location,
                tab,
            } => (
                message.to_string(),
                Vec::new(),
                WorkspaceMessage::ConfirmCloseDiscard {
                    location: *location,
                    tab: *tab,
                },
            ),
            CloseConfirmation::Batch { panels, summary } => {
                let list_items = if panels.len() > 1 {
                    panels
                        .iter()
                        .map(|panel| format!("• {}", panel.title))
                        .collect()
                } else {
                    Vec::new()
                };
                (
                    summary.to_string(),
                    list_items,
                    WorkspaceMessage::ConfirmCloseDiscard {
                        location: workspace.focused,
                        tab: 0,
                    },
                )
            }
        };

        return close_prompt_overlay(
            decorated,
            theme,
            title,
            list_items,
            WorkspaceMessage::ConfirmCloseCancel,
            discard_message,
        );
    }

    decorated
}

fn center_pane_grid<'a>(
    workspace: &'a Workspace,
    stores: &'a AppStores,
    theme: OpenZoneTheme,
) -> Element<'a, WorkspaceMessage> {
    let grid: PaneGrid<'a, WorkspaceMessage> =
        PaneGrid::new(&workspace.panes, |pane, pane_state, _is_maximized| {
            let location = PanelLocation::Center(pane);
            let focused = workspace.is_focused(location);
            pane_grid::Content::new(pane_body(
                theme, location, pane_state, focused, stores, workspace,
            ))
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(SpacingToken::S2.value())
        .on_click(WorkspaceMessage::PaneClicked)
        .on_drag(WorkspaceMessage::PaneDragged);

    container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn dock_side<'a>(
    workspace: &'a Workspace,
    stores: &'a AppStores,
    theme: OpenZoneTheme,
    side: DockSide,
) -> Element<'a, WorkspaceMessage> {
    let dock = workspace.docks.get(side);
    let location = PanelLocation::Dock(side);
    let focused = workspace.is_focused(location);

    if dock.is_empty() || dock.is_hidden() {
        return space::horizontal().width(Length::Shrink).into();
    }

    if dock.is_open() {
        let body = focus_on_click(
            pane_body(theme, location, &dock.tabs, focused, stores, workspace),
            location,
        );
        return container(body)
            .width(Length::Fixed(dock.extent))
            .height(Length::Fill)
            .style(move |_| pane_frame_style(theme, focused))
            .into();
    }

    // Collapsed: render rail
    dock_rail(theme, side, location, focused, true)
}

fn dock_bottom<'a>(
    workspace: &'a Workspace,
    stores: &'a AppStores,
    theme: OpenZoneTheme,
) -> Element<'a, WorkspaceMessage> {
    let dock = workspace.docks.get(DockSide::Bottom);
    let location = PanelLocation::Dock(DockSide::Bottom);
    let focused = workspace.is_focused(location);

    if dock.is_empty() || dock.is_hidden() {
        return space::vertical().height(Length::Shrink).into();
    }

    if dock.is_open() {
        let body = focus_on_click(
            pane_body(theme, location, &dock.tabs, focused, stores, workspace),
            location,
        );
        return container(body)
            .width(Length::Fill)
            .height(Length::Fixed(dock.extent))
            .style(move |_| pane_frame_style(theme, focused))
            .into();
    }

    // Collapsed: render rail
    dock_rail(theme, DockSide::Bottom, location, focused, false)
}

fn dock_rail(
    theme: OpenZoneTheme,
    side: DockSide,
    location: PanelLocation,
    focused: bool,
    vertical: bool,
) -> Element<'static, WorkspaceMessage> {
    let label = text(side.label())
        .size(TypeRole::MonoSm.size())
        .style(move |_| text::Style {
            color: Some(if focused {
                theme.foreground(ForegroundToken::Accent)
            } else {
                theme.foreground(ForegroundToken::Secondary)
            }),
        });

    let rail = button(label)
        .padding(SpacingToken::S1.value() as u16)
        .on_press(WorkspaceMessage::DockFocused(location))
        .style(move |_, _| tab_button_style(theme, focused));

    let sized = if vertical {
        container(rail)
            .width(Length::Fixed(DOCK_RAIL_THICKNESS))
            .height(Length::Fill)
    } else {
        container(rail)
            .width(Length::Fill)
            .height(Length::Fixed(DOCK_RAIL_THICKNESS))
    };

    sized
        .style(move |_| bar_style(theme, BackgroundToken::Tertiary))
        .into()
}

/// Top application chrome.
fn title_bar(theme: OpenZoneTheme) -> Element<'static, WorkspaceMessage> {
    let label = text("OpenZone Workspace")
        .size(TypeRole::LabelMd.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        });

    let toggle_label = match theme.mode {
        ThemeMode::Dark => "Light",
        ThemeMode::Light => "Dark",
    };

    let command_center = button(
        text("Search commands")
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            }),
    )
    .padding([
        SpacingToken::S1.value() as u16,
        SpacingToken::S3.value() as u16,
    ])
    .on_press(WorkspaceMessage::TogglePalette)
    .style(move |_, _| tab_button_style(theme, false));

    let new_window = button(
        text("New Window")
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            }),
    )
    .padding([
        SpacingToken::S1.value() as u16,
        SpacingToken::S3.value() as u16,
    ])
    .on_press(WorkspaceMessage::NewWindow)
    .style(move |_, _| tab_button_style(theme, false));

    let theme_toggle = button(
        text(toggle_label)
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            }),
    )
    .padding([
        SpacingToken::S1.value() as u16,
        SpacingToken::S3.value() as u16,
    ])
    .on_press(WorkspaceMessage::ToggleTheme)
    .style(move |_, _| tab_button_style(theme, false));

    let bar = row![
        label,
        command_center,
        space::horizontal().width(Length::Fill),
        new_window,
        theme_toggle,
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    container(bar)
        .width(Length::Fill)
        .padding(SpacingToken::S3.value())
        .style(move |_| bar_style(theme, BackgroundToken::Elevated))
        .into()
}

/// Bottom status bar — reports the focused location and active panel.
fn status_bar(theme: OpenZoneTheme, workspace: &Workspace) -> Element<'_, WorkspaceMessage> {
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
            let dock: &Dock = workspace.docks.get(side);
            let active_title = dock
                .tabs
                .active_panel()
                .map(|panel| panel.title())
                .unwrap_or_else(|| std::borrow::Cow::Borrowed("—"));
            let label_side = side.label();
            format!("Focus: {label_side} / {active_title}")
        }
    };

    let mut segments = vec![std::borrow::Cow::Owned(segment_first)];

    // Get contributions from the active panel of the focused location
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

    let joined = segments
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join("   ");

    let label = text(joined)
        .size(TypeRole::MonoSm.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Secondary)),
        });

    let dock_controls = row![
        dock_control_button(theme, DockSide::Left, workspace, "Activity"),
        dock_control_button(theme, DockSide::Right, workspace, "Conversation"),
        dock_control_button(theme, DockSide::Bottom, workspace, "Output"),
    ];

    // Layout: left segment fills, right segment shrinks
    let bar = row![container(label).width(Length::Fill), dock_controls,]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .padding(SpacingToken::S2.value());

    container(bar)
        .width(Length::Fill)
        .style(move |_| bar_style(theme, BackgroundToken::Secondary))
        .into()
}

fn dock_control_button<'a>(
    theme: OpenZoneTheme,
    side: DockSide,
    workspace: &Workspace,
    label: &'static str,
) -> Element<'a, WorkspaceMessage> {
    let dock = workspace.docks.get(side);
    use crate::workspace::workspace_command::Command;

    let _is_empty = dock.is_empty();
    let visibility = dock.visibility;

    let color = shell_chrome::dock_control_color(theme, visibility);

    // Label with state indicator
    let display_label = match visibility {
        DockVisibility::Open => format!("▾ {label}"),
        DockVisibility::Collapsed => format!("▸ {label}"),
        DockVisibility::Hidden => label.to_string(),
    };

    let btn = button(
        text(display_label)
            .size(TypeRole::MonoSm.size())
            .style(move |_| text::Style { color: Some(color) }),
    )
    .padding([
        SpacingToken::S1.value() as u16,
        SpacingToken::S2.value() as u16,
    ])
    .style(move |_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: if visibility == DockVisibility::Open {
                theme.foreground(ForegroundToken::Accent)
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: RadiusToken::Sm.value().into(),
        },
        ..button::Style::default()
    });

    let has_content = shell_chrome::dock_control_enabled(workspace, side);
    if !has_content && !dock.is_open() {
        btn.into()
    } else if visibility == DockVisibility::Open {
        btn.on_press(WorkspaceMessage::Command(Command::HideDock(side)))
            .into()
    } else {
        btn.on_press(WorkspaceMessage::Command(Command::OpenDock(side)))
            .into()
    }
}

/// Wrap a dock body so a click anywhere in it moves focus to that dock.
///
/// `pane_grid` already focuses center panes on body-click via its own
/// `on_click`, but docks are plain containers. A `MouseArea` lets inner
/// widgets (tab buttons, panel controls) handle the press first and only
/// then publishes the focus message, so nothing inside is shadowed.
fn focus_on_click<'a>(
    body: Element<'a, WorkspaceMessage>,
    location: PanelLocation,
) -> Element<'a, WorkspaceMessage> {
    mouse_area(body)
        .on_press(WorkspaceMessage::DockFocused(location))
        .into()
}

/// A single pane or dock body: tab strip stacked above active content.
fn pane_body<'a>(
    theme: OpenZoneTheme,
    location: PanelLocation,
    pane_state: &'a PaneState,
    focused: bool,
    stores: &'a AppStores,
    workspace: &'a Workspace,
) -> Element<'a, WorkspaceMessage> {
    let strip = tab_strip(theme, location, pane_state, workspace);

    let content: Element<'a, WorkspaceMessage> = match pane_state.active_panel() {
        Some(panel) => {
            let tab = pane_state.active;
            panel
                .view(stores)
                .map(move |message| WorkspaceMessage::Panel {
                    location,
                    tab,
                    message,
                })
        }
        None => text("empty pane")
            .size(TypeRole::BodyMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Muted)),
            })
            .into(),
    };

    let body = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SpacingToken::S4.value());

    let inner = column![strip, body]
        .width(Length::Fill)
        .height(Length::Fill);

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| pane_frame_style(theme, focused))
        .into()
}

/// The clickable tab strip for one pane or dock.
fn tab_strip<'a>(
    theme: OpenZoneTheme,
    location: PanelLocation,
    pane_state: &'a PaneState,
    workspace: &'a Workspace,
) -> Element<'a, WorkspaceMessage> {
    let drag_source_tab = workspace
        .drag_state
        .as_ref()
        .and_then(|drag| (drag.source_location == location).then_some(drag.source_tab));

    let mut tab_elements: Vec<Element<'a, WorkspaceMessage>> = Vec::new();
    for (index, panel) in pane_state.tabs.iter().enumerate() {
        if drag_source_tab == Some(index) {
            continue;
        }

        let active = index == pane_state.active;
        let hovered = workspace.hovered_tab == Some((location, index));
        tab_elements.push(tab_chip(
            theme,
            location,
            index,
            panel.as_ref(),
            active,
            hovered,
        ));
    }

    let mut tabs_row = row![].spacing(SpacingToken::S1.value());
    for tab in tab_elements {
        tabs_row = tabs_row.push(tab);
    }

    let scrollable_tabs = scrollable(
        container(tabs_row)
            .width(Length::Shrink)
            .padding(SpacingToken::S1.value()),
    )
    .width(Length::Fill)
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::default(),
    ));

    let strip_body: Element<'a, WorkspaceMessage> = if let PanelLocation::Dock(side) = location {
        row![
            scrollable_tabs,
            Space::new().width(Length::Fixed(SpacingToken::S2.value())),
            dock_strip_controls(theme, side, workspace),
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center)
        .spacing(SpacingToken::S1.value())
        .into()
    } else {
        scrollable_tabs.into()
    };

    container(strip_body)
        .width(Length::Fill)
        .style(move |_| bar_style(theme, BackgroundToken::Tertiary))
        .into()
}

fn tab_chip<'a>(
    theme: OpenZoneTheme,
    location: PanelLocation,
    index: usize,
    panel: &'a dyn Panel,
    active: bool,
    hovered: bool,
) -> Element<'a, WorkspaceMessage> {
    let display_title = if panel.is_dirty() {
        shell_chrome::dirty_tab_title(&panel.title())
    } else {
        panel.title().to_string()
    };

    let label = text(display_title)
        .size(TypeRole::LabelMd.size())
        .style(move |_: &iced::Theme| text::Style {
            color: Some(if active {
                theme.foreground(ForegroundToken::Accent)
            } else {
                theme.foreground(ForegroundToken::Secondary)
            }),
        });

    let mut title_row = row![label].spacing(SpacingToken::S1.value());
    if shell_chrome::tab_close_visible(active, hovered) {
        let close = button(text("×").size(TypeRole::LabelMd.size()).style(
            move |_: &iced::Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Muted)),
            },
        ))
        .padding(SpacingToken::S1.value() as u16)
        .on_press(WorkspaceMessage::TabCloseRequested {
            location,
            tab: index,
        })
        .style(move |_, _| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..button::Style::default()
        });
        title_row = title_row.push(close);
    }

    let underline =
        container(Space::new().height(Length::Fixed(shell_chrome::ACTIVE_TAB_UNDERLINE)))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(shell_chrome::tab_chip_underline_color(
                    theme, active,
                ))),
                ..container::Style::default()
            });

    let chip_inner = column![title_row, underline].spacing(0.0);
    let chip_body = container(chip_inner)
        .padding([
            SpacingToken::S1.value() as u16,
            SpacingToken::S3.value() as u16,
        ])
        .style(move |_| tab_chip_style(theme, active));

    mouse_area(chip_body)
        .on_press(WorkspaceMessage::TabDragStarted {
            location,
            tab: index,
        })
        .on_enter(WorkspaceMessage::TabHoverEntered {
            location,
            tab: index,
        })
        .on_exit(WorkspaceMessage::TabHoverExited {
            location,
            tab: index,
        })
        .interaction(mouse::Interaction::Grab)
        .into()
}

fn dock_strip_controls<'a>(
    theme: OpenZoneTheme,
    side: DockSide,
    workspace: &'a Workspace,
) -> Element<'a, WorkspaceMessage> {
    let dock = workspace.docks.get(side);
    let (collapse_label, hide_label) = shell_chrome::dock_strip_control_labels();
    let color = shell_chrome::dock_control_color(theme, dock.visibility);

    let collapse = button(
        text(collapse_label)
            .size(TypeRole::MonoSm.size())
            .style(move |_: &iced::Theme| text::Style { color: Some(color) }),
    )
    .padding(SpacingToken::S1.value() as u16)
    .on_press(WorkspaceMessage::Command(Command::CollapseDock(side)))
    .style(move |_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..button::Style::default()
    });

    let hide = button(
        text(hide_label)
            .size(TypeRole::MonoSm.size())
            .style(move |_: &iced::Theme| text::Style { color: Some(color) }),
    )
    .padding(SpacingToken::S1.value() as u16)
    .on_press(WorkspaceMessage::Command(Command::HideDock(side)))
    .style(move |_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..button::Style::default()
    });

    row![collapse, hide]
        .spacing(SpacingToken::S1.value())
        .padding([
            SpacingToken::S1.value() as u16,
            SpacingToken::S2.value() as u16,
        ])
        .into()
}

fn surface_style(theme: OpenZoneTheme, token: BackgroundToken) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme.background(token))),
        ..container::Style::default()
    }
}

fn bar_style(theme: OpenZoneTheme, token: BackgroundToken) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme.background(token))),
        border: Border {
            color: theme.border(BorderToken::Default),
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn pane_frame_style(theme: OpenZoneTheme, focused: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(
            theme.background(BackgroundToken::Secondary),
        )),
        border: Border {
            color: shell_chrome::pane_border_color(theme, focused),
            width: shell_chrome::pane_border_width(focused),
            radius: RadiusToken::Xs.value().into(),
        },
        ..container::Style::default()
    }
}

fn drop_overlay<'a>(
    workspace: &'a Workspace,
    theme: OpenZoneTheme,
) -> Element<'a, WorkspaceMessage> {
    let (preview, ghost) = if let Some(preview_state) = workspace.cross_window_drop_preview.as_ref()
    {
        let grid = drag::compute_grid_bounds(&workspace.docks, workspace.window_size);
        let pane_bounds = drag::compute_pane_bounds(&workspace.panes, grid);
        let (rails, bodies) = drag::compute_dock_regions(&workspace.docks, workspace.window_size);
        let preview = drag::preview_bounds(
            preview_state.target,
            &pane_bounds,
            &rails,
            &bodies,
            &workspace.docks,
            Some(&preview_state.drag),
        );
        (preview, None)
    } else if let Some(drag) = workspace.drag_state.as_ref() {
        let grid = drag::compute_grid_bounds(&workspace.docks, workspace.window_size);
        let pane_bounds = drag::compute_pane_bounds(&workspace.panes, grid);
        let (rails, bodies) = drag::compute_dock_regions(&workspace.docks, workspace.window_size);
        let preview = drag::preview_bounds(
            drag.target,
            &pane_bounds,
            &rails,
            &bodies,
            &workspace.docks,
            Some(drag),
        );
        let ghost = drag.pointer_moved.then(|| {
            let title = workspace
                .tab_title(drag.source_location, drag.source_tab)
                .unwrap_or_else(|| String::from("Tab"));
            let tab_w = layout_metrics::estimated_tab_width();
            let tab_h = layout_metrics::tab_strip_height();
            GhostTab {
                position: Point::new(drag.cursor.x - tab_w / 2.0, drag.cursor.y - tab_h / 2.0),
                size: Size::new(tab_w, tab_h),
                title,
            }
        });
        (preview, ghost)
    } else {
        return space::horizontal().width(Length::Shrink).into();
    };

    let accent = theme.foreground(ForegroundToken::Accent);
    let elevated = theme.background(BackgroundToken::Elevated);
    Canvas::new(DropOverlay {
        preview,
        ghost,
        accent,
        elevated,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[derive(Debug, Clone)]
struct GhostTab {
    position: Point,
    size: Size,
    title: String,
}

#[derive(Debug, Clone)]
struct DropOverlay {
    preview: Option<Rectangle>,
    ghost: Option<GhostTab>,
    accent: Color,
    elevated: Color,
}

impl<Message> Program<Message> for DropOverlay {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        if let Some(rect) = self.preview
            && rect.width > 0.0
            && rect.height > 0.0
        {
            frame.stroke_rectangle(
                Point::new(rect.x, rect.y),
                Size::new(rect.width, rect.height),
                Stroke::default().with_width(2.0).with_color(self.accent),
            );
        }

        if let Some(ghost) = &self.ghost {
            frame.stroke_rectangle(
                ghost.position,
                ghost.size,
                Stroke::default().with_width(1.0).with_color(self.accent),
            );
        }

        vec![frame.into_geometry()]
    }
}

fn tab_chip_style(theme: OpenZoneTheme, active: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        text_color: Some(if active {
            theme.foreground(ForegroundToken::Accent)
        } else {
            theme.foreground(ForegroundToken::Secondary)
        }),
        ..container::Style::default()
    }
}

fn tab_button_style(theme: OpenZoneTheme, active: bool) -> button::Style {
    let background = if active {
        theme.background(BackgroundToken::Elevated)
    } else {
        Color::TRANSPARENT
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active {
            theme.foreground(ForegroundToken::Accent)
        } else {
            theme.foreground(ForegroundToken::Secondary)
        },
        border: Border {
            color: theme.border(BorderToken::Subtle),
            width: if active { 1.0 } else { 0.0 },
            radius: RadiusToken::Xs.value().into(),
        },
        ..button::Style::default()
    }
}

/// Render the command palette as a top-centered dropdown below the title bar.
fn palette_overlay<'a>(
    theme: OpenZoneTheme,
    workspace: &'a Workspace,
) -> Element<'a, WorkspaceMessage> {
    let palette = &workspace.palette;

    let palette_spec = shell_chrome::PaletteOverlaySpec::current();
    let max_width = palette_spec.max_width;

    let items: Vec<Element<'_, WorkspaceMessage>> = palette
        .filtered
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == palette.selected;
            let label = item.label.to_string();
            let btn = button(
                text(label)
                    .size(TypeRole::LabelMd.size())
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([
                SpacingToken::S1.value() as u16,
                SpacingToken::S2.value() as u16,
            ])
            .on_press(WorkspaceMessage::PaletteItemClicked(i))
            .style(move |_, _| {
                let bg = if is_selected {
                    theme.background(BackgroundToken::Elevated)
                } else {
                    Color::TRANSPARENT
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: theme.foreground(ForegroundToken::Primary),
                    border: Border::default(),
                    ..button::Style::default()
                }
            });
            Element::from(btn)
        })
        .collect();

    let list = column(items).width(Length::Fill);

    let result_count = if palette.filtered.is_empty() {
        text("No matching commands")
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            })
    } else {
        text(format!(
            "{}/{}",
            palette.selected + 1,
            palette.filtered.len()
        ))
        .size(TypeRole::LabelMd.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Secondary)),
        })
    };

    let dropdown = column![
        // Search text input
        text_input("Type to filter commands...", &palette.query)
            .on_input(WorkspaceMessage::PaletteQueryChanged)
            .padding([
                SpacingToken::S1.value() as u16,
                SpacingToken::S1.value() as u16,
            ])
            .width(Length::Fill),
        // Divider
        container(Space::new().height(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(theme.border(BorderToken::Subtle),)),
                ..container::Style::default()
            }),
        list,
        container(result_count)
            .width(Length::Fill)
            .align_x(Horizontal::Right)
            .padding([
                SpacingToken::S1.value() as u16,
                SpacingToken::S1.value() as u16,
            ]),
    ]
    .spacing(SpacingToken::Hairline.value())
    .width(Length::Fixed(max_width));

    let dropdown_card = container(dropdown).style(move |_| container::Style {
        background: Some(Background::Color(
            theme.background(BackgroundToken::Primary),
        )),
        border: Border {
            color: theme.border(BorderToken::Strong),
            width: 1.0,
            radius: RadiusToken::Md.value().into(),
        },
        ..container::Style::default()
    });

    container(dropdown_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Top)
        .padding(Padding {
            top: palette_spec.top_inset,
            ..Padding::ZERO
        })
        .into()
}

const CLOSE_PROMPT_LIST_MAX_HEIGHT: f32 = 160.0;

/// Shared modal chrome for dirty-tab confirmations (workspace tab close and app-level close).
pub fn close_prompt_overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    theme: OpenZoneTheme,
    title: String,
    list_items: Vec<String>,
    on_cancel: Message,
    on_discard: Message,
) -> Element<'a, Message> {
    let overlay = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.4))),
            ..container::Style::default()
        });

    let title_text = text(title)
        .size(TypeRole::LabelMd.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        });

    let mut body = column![title_text.align_x(Horizontal::Center)]
        .spacing(SpacingToken::S4.value())
        .align_x(Horizontal::Center);

    if !list_items.is_empty() {
        let rows: Vec<Element<'a, Message>> = list_items
            .into_iter()
            .map(|item| {
                text(item)
                    .size(TypeRole::LabelMd.size())
                    .style(move |_| text::Style {
                        color: Some(theme.foreground(ForegroundToken::Secondary)),
                    })
                    .into()
            })
            .collect();
        let list = column(rows)
            .spacing(SpacingToken::S1.value())
            .width(Length::Fill);
        let list = scrollable(list)
            .height(Length::Fixed(CLOSE_PROMPT_LIST_MAX_HEIGHT))
            .width(Length::Fill);
        body = body.push(list);
    }

    let cancel_btn = button(
        text("Cancel")
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            }),
    )
    .padding([
        SpacingToken::S2.value() as u16,
        SpacingToken::S3.value() as u16,
    ])
    .on_press(on_cancel)
    .style(move |_, _| button::Style {
        background: Some(Background::Color(
            theme.background(BackgroundToken::Tertiary),
        )),
        border: Border {
            color: theme.border(BorderToken::Default),
            width: 1.0,
            radius: RadiusToken::Sm.value().into(),
        },
        ..button::Style::default()
    });

    let discard_btn = button(
        text("Discard")
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(theme.status(StatusToken::Danger)),
            }),
    )
    .padding([
        SpacingToken::S2.value() as u16,
        SpacingToken::S3.value() as u16,
    ])
    .on_press(on_discard)
    .style(move |_, _| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        border: Border {
            color: theme.status(StatusToken::Danger),
            width: 1.0,
            radius: RadiusToken::Sm.value().into(),
        },
        ..button::Style::default()
    });

    body = body.push(
        row![
            cancel_btn,
            Space::new().width(Length::Fixed(SpacingToken::S2.value())),
            discard_btn
        ]
        .align_y(Alignment::Center),
    );

    let modal_box = container(body)
        .padding(SpacingToken::S5.value())
        .width(Length::Shrink)
        .style(move |_| container::Style {
            background: Some(Background::Color(
                theme.background(BackgroundToken::Primary),
            )),
            border: Border {
                color: theme.border(BorderToken::Strong),
                width: 1.0,
                radius: RadiusToken::Md.value().into(),
            },
            ..container::Style::default()
        });

    let modal_centered = container(modal_box)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    stack![base, overlay, modal_centered]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
