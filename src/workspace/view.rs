#![allow(dead_code)]

//! Workspace view.
//!
//! Pure render over `&Workspace`. Composes the outer frame
//! (`column[title_bar, pane_grid, status_bar]`) and, per pane, a tab
//! strip above the active panel's content. All styling resolves through
//! `shared::design` tokens — no hardcoded colors or sizes.

use iced::widget::{button, column, container, pane_grid, row, text, PaneGrid};
use iced::{Background, Border, Color, Element, Length};

use crate::shared::design::{
    BackgroundToken, BorderToken, ForegroundToken, OpenZoneTheme, RadiusToken, SpacingToken,
    TypeRole,
};
use crate::workspace::location::PanelLocation;
use crate::workspace::message::WorkspaceMessage;
use crate::workspace::pane_state::PaneState;
use crate::workspace::state::Workspace;

/// Render the whole workspace shell.
pub fn view(workspace: &Workspace) -> Element<'_, WorkspaceMessage> {
    let theme = workspace.theme;

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

    let center = container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SpacingToken::S3.value())
        .style(move |_| surface_style(theme, BackgroundToken::Primary));

    column![title_bar(theme), center, status_bar(theme, workspace)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Top application chrome.
fn title_bar(theme: OpenZoneTheme) -> Element<'static, WorkspaceMessage> {
    let label = text("OpenZone Workspace")
        .size(TypeRole::LabelMd.size())
        .style(move |_| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        });

    container(label)
        .width(Length::Fill)
        .padding(SpacingToken::S3.value())
        .style(move |_| bar_style(theme, BackgroundToken::Elevated))
        .into()
}

/// Bottom status bar — reports the focused pane and its active panel.
fn status_bar(theme: OpenZoneTheme, workspace: &Workspace) -> Element<'_, WorkspaceMessage> {
    let active_title = match workspace.focused {
        PanelLocation::Center(pane) => workspace
            .panes
            .get(pane)
            .and_then(|pane_state| pane_state.active_panel())
            .map(|panel| panel.title())
            .unwrap_or_else(|| String::from("—")),
    };

    let label = text(format!("Focus: {active_title}"))
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

/// A single pane: tab strip stacked above the active panel's content.
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
            // Wrap the panel's erased messages with routing metadata.
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

    // Focused pane gets a strong border; others a default separator.
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

/// The clickable tab strip for one pane.
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

// -- styling helpers (token-driven) -----------------------------------------

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
