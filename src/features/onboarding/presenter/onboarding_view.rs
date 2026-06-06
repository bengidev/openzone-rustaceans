//! Onboarding view — immersive monochrome landing.
//!
//! Layered scene with animated backdrop, large interactive galaxy orb,
//! and centered hero copy.

use iced::Element;
use iced::Font;
use iced::Length;
use iced::Theme;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::Canvas;
use iced::widget::text::Wrapping;
use iced::widget::{MouseArea, Space, Stack, button, column, container, row, text};

use crate::shared::design::OpenZoneTheme;
use crate::shared::design::ThemeMode;
use crate::shared::design::tokens::{
    ActionToken, BackgroundToken, BorderToken, ForegroundToken, RadiusToken, SpacingToken, TypeRole,
};

use crate::features::onboarding::application::onboarding_messages::OnboardingMessage;
use crate::features::onboarding::application::onboarding_state::OnboardingState;
use crate::features::onboarding::presenter::galaxy_orb::GalaxyOrbProgram;
use crate::features::onboarding::presenter::scene_backdrop::SceneBackdrop;

const HERO_MAX_WIDTH: f32 = 600.0;
const ORB_HEIGHT: f32 = 360.0;
const EDGE_INSET_H: f32 = 16.0;
const EDGE_INSET_V: f32 = 20.0;

/// Render the onboarding view.
pub fn view(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let backdrop = Canvas::new(SceneBackdrop::new(theme, state.started_at, state.now))
        .width(Length::Fill)
        .height(Length::Fill);

    let main = container(
        column![
            header_row(state),
            Space::new().height(Length::Fixed(SpacingToken::S4.value())),
            row![
                Space::new().width(Length::Fill),
                container(hero_block(state))
                    .width(Length::Fixed(HERO_MAX_WIDTH))
                    .align_x(Horizontal::Center),
                Space::new().width(Length::Fill),
            ]
            .width(Length::Fill),
            Space::new().height(Length::Fill),
            action_row(state),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([EDGE_INSET_V, EDGE_INSET_H])
    .align_y(Vertical::Top);

    let scene = Stack::new()
        .width(Length::Fill)
        .height(Length::Fill)
        .push(main)
        .push_under(backdrop);

    container(scene)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(
                theme.background(BackgroundToken::Primary),
            )),
            ..Default::default()
        })
        .into()
}

fn header_row(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let brand = column![
        text("OpenZone")
            .size(TypeRole::LabelMd.size())
            .style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Primary)),
            }),
        text("LOCAL AI WORKSPACE")
            .size(9)
            .style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Muted)),
            }),
    ]
    .spacing(2);

    let controls = theme_toggle_button(state);

    row![brand, Space::new().width(Length::Fill), controls,]
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
}

fn theme_toggle_button(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;
    let label = match state.theme_mode {
        ThemeMode::Dark => "Light",
        ThemeMode::Light => "Dark",
    };

    button(
        row![
            theme_mode_icon(state.theme_mode, theme),
            Space::new().width(Length::Fixed(6.0)),
            text(label).size(10).style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Secondary)),
            }),
        ]
        .align_y(Vertical::Center),
    )
    .padding([6, 10])
    .on_press(OnboardingMessage::ToggleTheme)
    .style(move |_t: &Theme, status| chip_style(theme, status))
    .into()
}

fn theme_mode_icon(mode: ThemeMode, theme: OpenZoneTheme) -> Element<'static, OnboardingMessage> {
    let stroke = theme.foreground(ForegroundToken::Accent);
    let fill = with_alpha(stroke, 0.18);

    let (symbol, bg) = match mode {
        ThemeMode::Dark => ("◐", fill),
        ThemeMode::Light => ("◑", with_alpha(stroke, 0.28)),
    };

    container(text(symbol).size(11).style(move |_t: &Theme| text::Style {
        color: Some(stroke),
    }))
    .padding([2, 4])
    .style(move |_t: &Theme| container::Style {
        background: Some(iced::Background::Color(bg)),
        border: iced::Border {
            radius: RadiusToken::Xs.value().into(),
            width: 1.0,
            color: with_alpha(stroke, 0.35),
        },
        ..Default::default()
    })
    .into()
}

fn hero_block(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let orb = Canvas::new(GalaxyOrbProgram::with_dynamics(
        theme,
        state.started_at,
        state.now,
        state.displayed_speed,
        state.displayed_zoom,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(ORB_HEIGHT));

    let interactive_orb = MouseArea::new(orb)
        .on_press(OnboardingMessage::OrbPressed)
        .on_release(OnboardingMessage::OrbReleased)
        .interaction(iced::mouse::Interaction::Pointer);

    let headline = text("Your local AI command workspace")
        .size(TypeRole::DisplayMd.size())
        .style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        });

    let subhead = text(
        "OpenZone combines chat, terminal, editing, and Rust-native performance in one permissioned desktop environment. To leave the crowded cloud, polluted by leaks and unconsciousness, to return to a workspace that stays on your machine.",
    )
    .font(Font::MONOSPACE)
    .size(TypeRole::MonoSm.size())
    .line_height(iced::widget::text::LineHeight::Relative(
        TypeRole::MonoSm.line_height(),
    ))
    .width(Length::Fill)
    .wrapping(Wrapping::Word)
    .style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Secondary)),
    });

    column![
        container(interactive_orb)
            .width(Length::Fill)
            .height(Length::Fixed(ORB_HEIGHT)),
        Space::new().height(Length::Fixed(28.0)),
        headline,
        Space::new().height(Length::Fixed(10.0)),
        subhead,
    ]
    .spacing(0)
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .into()
}

fn action_row(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let primary = button(
        row![
            text("Enter OpenZone")
                .size(13)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::DEFAULT
                })
                .style(move |_t: &Theme| text::Style {
                    color: Some(theme.action(ActionToken::StrongText)),
                }),
        ]
        .align_y(Vertical::Center),
    )
    .padding([14, 28])
    .on_press(OnboardingMessage::EnterPressed)
    .style(move |_t: &Theme, status| primary_action_style(theme, status));

    row![
        Space::new().width(Length::Fill),
        primary,
        Space::new().width(Length::Fill),
    ]
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}

fn primary_action_style(theme: OpenZoneTheme, status: button::Status) -> button::Style {
    let base = button::Style {
        background: Some(iced::Background::Color(theme.action(ActionToken::Strong))),
        text_color: theme.action(ActionToken::StrongText),
        border: iced::Border {
            radius: RadiusToken::Sm.value().into(),
            width: 0.0,
            color: iced::Color::TRANSPARENT,
        },
        ..Default::default()
    };
    match status {
        button::Status::Hovered => button::Style {
            background: Some(iced::Background::Color(with_alpha(
                theme.action(ActionToken::Strong),
                0.88,
            ))),
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(with_alpha(
                theme.action(ActionToken::Strong),
                0.72,
            ))),
            ..base
        },
        _ => base,
    }
}

fn chip_style(theme: OpenZoneTheme, status: button::Status) -> button::Style {
    let text_color = theme.foreground(ForegroundToken::Secondary);
    let base = button::Style {
        background: Some(iced::Background::Color(
            theme.background(BackgroundToken::Tertiary),
        )),
        text_color,
        border: iced::Border {
            radius: RadiusToken::Xs.value().into(),
            width: 1.0,
            color: theme.border(BorderToken::Default),
        },
        ..Default::default()
    };
    match status {
        button::Status::Hovered => button::Style {
            border: iced::Border {
                radius: RadiusToken::Xs.value().into(),
                width: 1.0,
                color: theme.border(BorderToken::Strong),
            },
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(
                theme.background(BackgroundToken::Secondary),
            )),
            ..base
        },
        _ => base,
    }
}

fn with_alpha(color: iced::Color, alpha: f32) -> iced::Color {
    iced::Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}
