#![allow(dead_code)]

//! Workspace view.
//!
//! Pure render over `&Workspace`. Composes the outer frame
//! (`column[title_bar, docks + center, status_bar]`) and, per pane or
//! dock, a tab strip above the active panel's content. Collapsed docks
//! render as minimal rails. All styling resolves through
//! `shared::design` tokens — no hardcoded colors or sizes.

use iced::widget::{PaneGrid, button, column, container, mouse_area, pane_grid, row, space, text};
use iced::{Background, Border, Color, Element, Length};

use crate::shared::design::{
    BackgroundToken, BorderToken, ForegroundToken, OpenZoneTheme, RadiusToken, SpacingToken,
    ThemeMode, TypeRole,
};
use crate::workspace::dock::Dock;
use crate::workspace::location::{DockSide, PanelLocation};
use crate::workspace::message::WorkspaceMessage;
use crate::workspace::pane_state::PaneState;
use crate::workspace::state::Workspace;

const SIDE_DOCK_WIDTH: f32 = 280.0;
const BOTTOM_DOCK_HEIGHT: f32 = 200.0;
const DOCK_RAIL_THICKNESS: f32 = 28.0;

/// Render the whole workspace shell.
pub fn view(workspace: &Workspace) -> Element<'_, WorkspaceMessage> {
    let theme = workspace.theme;

    let center = center_pane_grid(workspace, theme);

    let main_row = row![
        dock_side(workspace, theme, DockSide::Left),
        center,
        dock_side(workspace, theme, DockSide::Right),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(SpacingToken::S2.value());

    let body = column![main_row, dock_bottom(workspace, theme)]
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(SpacingToken::S2.value());

    let framed = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SpacingToken::S3.value())
        .style(move |_| surface_style(theme, BackgroundToken::Primary));

    column![title_bar(theme), framed, status_bar(theme, workspace)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn center_pane_grid(workspace: &Workspace, theme: OpenZoneTheme) -> Element<'_, WorkspaceMessage> {
    let grid: PaneGrid<'_, WorkspaceMessage> =
        PaneGrid::new(&workspace.panes, |pane, pane_state, _is_maximized| {
            let location = PanelLocation::Center(pane);
            let focused = workspace.is_focused(location);
            pane_grid::Content::new(pane_body(theme, location, pane_state, focused))
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(SpacingToken::S2.value())
        .on_click(WorkspaceMessage::PaneClicked);

    container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn dock_side(
    workspace: &Workspace,
    theme: OpenZoneTheme,
    side: DockSide,
) -> Element<'_, WorkspaceMessage> {
    let dock = workspace.docks.get(side);
    let location = PanelLocation::Dock(side);
    let focused = workspace.is_focused(location);

    if dock.is_empty() {
        return space::horizontal().width(Length::Shrink).into();
    }

    if dock.open {
        let body = focus_on_click(pane_body(theme, location, &dock.tabs, focused), location);
        return container(body)
            .width(Length::Fixed(SIDE_DOCK_WIDTH))
            .height(Length::Fill)
            .style(move |_| {
                pane_frame_style(
                    theme,
                    if focused {
                        BorderToken::Strong
                    } else {
                        BorderToken::Default
                    },
                )
            })
            .into();
    }

    dock_rail(theme, side, location, focused, true)
}

fn dock_bottom(workspace: &Workspace, theme: OpenZoneTheme) -> Element<'_, WorkspaceMessage> {
    let dock = workspace.docks.get(DockSide::Bottom);
    let location = PanelLocation::Dock(DockSide::Bottom);
    let focused = workspace.is_focused(location);

    if dock.is_empty() {
        return space::vertical().height(Length::Shrink).into();
    }

    if dock.open {
        let body = focus_on_click(pane_body(theme, location, &dock.tabs, focused), location);
        return container(body)
            .width(Length::Fill)
            .height(Length::Fixed(BOTTOM_DOCK_HEIGHT))
            .style(move |_| {
                pane_frame_style(
                    theme,
                    if focused {
                        BorderToken::Strong
                    } else {
                        BorderToken::Default
                    },
                )
            })
            .into();
    }

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
        space::horizontal().width(Length::Fill),
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
    let (region, active_title) = match workspace.focused {
        PanelLocation::Center(pane) => (
            String::from("Center"),
            workspace
                .panes
                .get(pane)
                .and_then(|pane_state| pane_state.active_panel())
                .map(|panel| panel.title())
                .unwrap_or_else(|| String::from("—")),
        ),
        PanelLocation::Dock(side) => {
            let dock: &Dock = workspace.docks.get(side);
            (
                side.label().to_string(),
                dock.tabs
                    .active_panel()
                    .map(|panel| panel.title())
                    .unwrap_or_else(|| String::from("—")),
            )
        }
    };

    let label = text(format!("Focus: {region} / {active_title}"))
        .size(TypeRole::MonoSm.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Secondary)),
        });

    container(label)
        .width(Length::Fill)
        .padding(SpacingToken::S2.value())
        .style(move |_| bar_style(theme, BackgroundToken::Secondary))
        .into()
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
) -> Element<'a, WorkspaceMessage> {
    let strip = tab_strip(theme, location, pane_state);

    let content: Element<'a, WorkspaceMessage> = match pane_state.active_panel() {
        Some(panel) => {
            let tab = pane_state.active;
            panel.view().map(move |message| WorkspaceMessage::Panel {
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

    let border_token = if focused {
        BorderToken::Strong
    } else {
        BorderToken::Default
    };

    container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| pane_frame_style(theme, border_token))
        .into()
}

/// The clickable tab strip for one pane or dock.
fn tab_strip<'a>(
    theme: OpenZoneTheme,
    location: PanelLocation,
    pane_state: &'a PaneState,
) -> Element<'a, WorkspaceMessage> {
    let mut strip = row![].spacing(SpacingToken::S1.value());

    for (index, panel) in pane_state.tabs.iter().enumerate() {
        let active = index == pane_state.active;
        let label = text(panel.title())
            .size(TypeRole::LabelMd.size())
            .style(move |_| text::Style {
                color: Some(if active {
                    theme.foreground(ForegroundToken::Accent)
                } else {
                    theme.foreground(ForegroundToken::Secondary)
                }),
            });

        let tab = button(label)
            .padding([
                SpacingToken::S1.value() as u16,
                SpacingToken::S3.value() as u16,
            ])
            .on_press(WorkspaceMessage::TabSelected {
                location,
                tab: index,
            })
            .style(move |_, _| tab_button_style(theme, active));

        strip = strip.push(tab);
    }

    container(strip)
        .width(Length::Fill)
        .padding(SpacingToken::S1.value())
        .style(move |_| bar_style(theme, BackgroundToken::Tertiary))
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
            radius: RadiusToken::Xs.value().into(),
        },
        ..container::Style::default()
    }
}

fn pane_frame_style(theme: OpenZoneTheme, border_token: BorderToken) -> container::Style {
    container::Style {
        background: Some(Background::Color(
            theme.background(BackgroundToken::Secondary),
        )),
        border: Border {
            color: theme.border(border_token),
            width: if border_token == BorderToken::Strong {
                2.0
            } else {
                1.0
            },
            radius: RadiusToken::Md.value().into(),
        },
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
            radius: RadiusToken::Sm.value().into(),
        },
        ..button::Style::default()
    }
}
