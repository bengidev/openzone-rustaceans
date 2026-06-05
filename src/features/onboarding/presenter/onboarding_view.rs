//! Onboarding view — single-page layout with main-feature highlight cards.
//!
//! 1. Identity lock-up (top)
//! 2. Context marker
//! 3. Primary message + galaxy orb demonstration
//! 4. Main-feature cards (chat, terminal, text editor, rust)
//! 5. Actions (enter + skip)

use iced::Element;
use iced::Font;
use iced::Length;
use iced::Theme;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::Canvas;
use iced::widget::text::Wrapping;
use iced::widget::{MouseArea, Row, Space, button, column, container, row, scrollable, text};

use crate::shared::design::OpenZoneTheme;
use crate::shared::design::tokens::{
    AccentToken, ActionToken, BackgroundToken, BorderToken, ForegroundToken, RadiusToken,
    SpacingToken,
};

use crate::features::onboarding::application::onboarding_dynamics::dynamics_for_progress;
use crate::features::onboarding::application::onboarding_messages::OnboardingMessage;
use crate::features::onboarding::application::onboarding_state::{FEATURE_COUNT, OnboardingState};
use crate::features::onboarding::presenter::feature_card_icon::{FeatureCardIcon, FeatureKind};
use crate::features::onboarding::presenter::galaxy_orb::GalaxyOrbProgram;

const PAGE_MAX_WIDTH: f32 = 960.0;
const FEATURE_CARD_WIDTH: f32 = 200.0;
const FEATURE_CARD_HEIGHT: f32 = 224.0;
const FEATURE_ICON_HEIGHT: f32 = 88.0;
const FEATURE_CARD_GAP: f32 = 12.0;
const CARD_INNER_WIDTH: f32 = FEATURE_CARD_WIDTH - 22.0;
const ORB_HEIGHT: f32 = 200.0;

/// Render the onboarding view.
pub fn view(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let scroll_body = column![
        identity_row(state),
        Space::new().height(Length::Fixed(SpacingToken::S5.value())),
        context_marker(state),
        Space::new().height(Length::Fixed(SpacingToken::S4.value())),
        hero_block(state),
        Space::new().height(Length::Fixed(SpacingToken::S6.value())),
        main_feature_cards(state),
    ]
    .max_width(PAGE_MAX_WIDTH)
    .spacing(0)
    .width(Length::Fill);

    let page = column![
        scrollable(scroll_body)
            .width(Length::Fill)
            .height(Length::Fill),
        Space::new().height(Length::Fixed(SpacingToken::S5.value())),
        action_row(state),
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .max_width(PAGE_MAX_WIDTH);

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([SpacingToken::S6.value(), SpacingToken::S7.value()])
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top)
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(
                theme.background(BackgroundToken::Primary),
            )),
            ..Default::default()
        })
        .into()
}

fn identity_row(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let pip = pip(theme.foreground(ForegroundToken::Accent), 8.0);

    let brand = column![
        text("OpenZone")
            .size(13)
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

    let lhs = row![pip, Space::new().width(Length::Fixed(10.0)), brand].align_y(Vertical::Center);

    let skip = button(text("Skip").size(11).style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Secondary)),
    }))
    .padding([6, 12])
    .on_press(OnboardingMessage::Skipped)
    .style(move |_t: &Theme, status| chip_style(theme, status));

    row![lhs, Space::new().width(Length::Fill), skip]
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
}

fn context_marker(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let dot = pip(theme.foreground(ForegroundToken::Accent), 6.0);

    container(
        row![
            dot,
            Space::new().width(Length::Fixed(8.0)),
            text("WKSP-00")
                .size(10)
                .style(move |_t: &Theme| text::Style {
                    color: Some(theme.foreground(ForegroundToken::Accent)),
                }),
            Space::new().width(Length::Fixed(12.0)),
            text("command surface")
                .size(10)
                .style(move |_t: &Theme| text::Style {
                    color: Some(theme.foreground(ForegroundToken::Secondary)),
                }),
        ]
        .align_y(Vertical::Center),
    )
    .padding([6, 12])
    .style(move |_t: &Theme| container::Style {
        background: Some(iced::Background::Color(theme.accent(AccentToken::Soft))),
        border: iced::Border {
            radius: RadiusToken::Pill.value().into(),
            width: 1.0,
            color: with_alpha(theme.foreground(ForegroundToken::Accent), 0.32),
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

    let badge = orb_state_badge(state);

    let headline = text("Your local AI command workspace.")
        .size(32)
        .style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        });

    let subhead = text(
        "OpenZone combines chat, terminal, editing, and Rust-native performance \
         in one permissioned desktop environment.",
    )
    .size(14)
    .style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Secondary)),
    });

    column![
        container(interactive_orb)
            .width(Length::Fill)
            .height(Length::Fixed(ORB_HEIGHT)),
        Space::new().height(Length::Fixed(10.0)),
        badge,
        Space::new().height(Length::Fixed(48.0)),
        headline,
        Space::new().height(Length::Fixed(8.0)),
        subhead,
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

fn orb_state_badge(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;
    let progress = state.hold_progress;

    let label = if progress < 0.05 {
        "GALAXY ORB / READY".to_string()
    } else if progress > 0.95 {
        "GALAXY ORB / FINAL FORM".to_string()
    } else {
        let (speed, _) = dynamics_for_progress(progress);
        format!("GALAXY ORB / {}x", speed.round() as u32)
    };

    let dot = pip(theme.foreground(ForegroundToken::Accent), 6.0);

    container(
        row![
            dot,
            Space::new().width(Length::Fixed(8.0)),
            text(label).size(10).style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Accent)),
            }),
        ]
        .align_y(Vertical::Center),
    )
    .padding([6, 10])
    .style(move |_t: &Theme| container::Style {
        background: Some(iced::Background::Color(with_alpha(
            theme.foreground(ForegroundToken::Accent),
            0.10,
        ))),
        border: iced::Border {
            radius: RadiusToken::Xs.value().into(),
            width: 1.0,
            color: with_alpha(theme.foreground(ForegroundToken::Accent), 0.32),
        },
        ..Default::default()
    })
    .into()
}

fn main_feature_cards(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let section_label = row![
        text("MAIN FEATURES")
            .size(9)
            .style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Muted)),
            }),
        Space::new().width(Length::Fill),
        text(format!("{:02} MODULES", FEATURE_COUNT))
            .size(9)
            .style(move |_t: &Theme| text::Style {
                color: Some(theme.foreground(ForegroundToken::Muted)),
            }),
    ]
    .align_y(Vertical::Center)
    .width(Length::Fill);

    let mut cards: Row<'_, OnboardingMessage> =
        Row::new().spacing(FEATURE_CARD_GAP).width(Length::Shrink);
    for index in 0..FEATURE_COUNT {
        cards = cards.push(feature_card(state, index));
    }

    let cards_row = row![
        Space::new().width(Length::Fill),
        cards,
        Space::new().width(Length::Fill),
    ]
    .width(Length::Fill);

    column![
        section_label,
        Space::new().height(Length::Fixed(8.0)),
        cards_row,
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

fn feature_card(state: &OnboardingState, index: usize) -> Element<'_, OnboardingMessage> {
    let (code, title, body, kind) = feature_meta(index);
    let theme = state.theme;
    let active = state.selected_feature == index;
    let glow = state.feature_glow[index];

    let accent = theme.foreground(ForegroundToken::Accent);
    let muted = theme.foreground(ForegroundToken::Muted);
    let primary = theme.foreground(ForegroundToken::Primary);
    let secondary = theme.foreground(ForegroundToken::Secondary);

    let index_tag = container(text(code).size(9).style(move |_t: &Theme| text::Style {
        color: Some(if active {
            theme.action(ActionToken::StrongText)
        } else {
            secondary
        }),
    }))
    .padding([4, 6])
    .style(move |_t: &Theme| container::Style {
        background: Some(iced::Background::Color(if active {
            accent
        } else {
            theme.background(BackgroundToken::Tertiary)
        })),
        border: iced::Border {
            radius: RadiusToken::Xs.value().into(),
            width: 1.0,
            color: if active {
                accent
            } else {
                theme.border(BorderToken::Default)
            },
        },
        ..Default::default()
    });

    let icon = Canvas::new(FeatureCardIcon::new(
        kind,
        glow,
        state.now,
        state.started_at,
        theme,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(FEATURE_ICON_HEIGHT));

    let title_text = text(title).size(11).style(move |_t: &Theme| text::Style {
        color: Some(if active { primary } else { secondary }),
    });

    let body_text = text(body)
        .size(9)
        .line_height(iced::widget::text::LineHeight::Relative(1.35))
        .width(Length::Fixed(CARD_INNER_WIDTH))
        .wrapping(Wrapping::Word)
        .style(move |_t: &Theme| text::Style {
            color: Some(if active { secondary } else { muted }),
        });

    let footer = if active {
        row![
            container(Space::new())
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0))
                .style(move |_t: &Theme| container::Style {
                    background: Some(iced::Background::Color(accent)),
                    border: iced::Border {
                        radius: RadiusToken::Xs.value().into(),
                        width: 0.0,
                        color: iced::Color::TRANSPARENT,
                    },
                    ..Default::default()
                }),
            Space::new().width(Length::Fixed(5.0)),
            text("SELECTED")
                .size(9)
                .style(move |_t: &Theme| text::Style {
                    color: Some(accent),
                }),
        ]
        .align_y(Vertical::Center)
    } else {
        row![
            text("+")
                .size(11)
                .style(move |_t: &Theme| text::Style { color: Some(muted) }),
        ]
        .align_y(Vertical::Center)
    };

    let accent_wash = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(2.0 + glow * 2.0))
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(with_alpha(
                accent,
                0.12 + glow * 0.28,
            ))),
            ..Default::default()
        });

    let card_body = column![
        index_tag,
        Space::new().height(Length::Fixed(8.0)),
        icon,
        Space::new().height(Length::Fixed(10.0)),
        title_text,
        Space::new().height(Length::Fixed(4.0)),
        body_text,
        Space::new().height(Length::Fill),
        footer,
        Space::new().height(Length::Fixed(5.0)),
        accent_wash,
    ]
    .spacing(0)
    .height(Length::Fill);

    let card = button(
        container(card_body)
            .padding([11, 11])
            .width(Length::Fixed(FEATURE_CARD_WIDTH))
            .height(Length::Fixed(FEATURE_CARD_HEIGHT))
            .style(move |_t: &Theme| container::Style {
                background: Some(iced::Background::Color(blend_surface(
                    theme.background(BackgroundToken::Secondary),
                    theme.background(BackgroundToken::GalaxyTint),
                    glow,
                ))),
                border: iced::Border {
                    radius: RadiusToken::Sm.value().into(),
                    width: if active { 1.5 } else { 1.0 + glow * 0.35 },
                    color: with_alpha(
                        if active {
                            accent
                        } else {
                            theme.border(BorderToken::Default)
                        },
                        0.45 + glow * 0.55,
                    ),
                },
                ..Default::default()
            }),
    )
    .width(Length::Fixed(FEATURE_CARD_WIDTH))
    .padding(0)
    .on_press(OnboardingMessage::FeatureSelected(index))
    .style(move |_t: &Theme, status| feature_card_button_style(theme, active, glow, status));

    MouseArea::new(card)
        .on_enter(OnboardingMessage::FeatureHovered(Some(index)))
        .on_exit(OnboardingMessage::FeatureHovered(None))
        .interaction(iced::mouse::Interaction::Pointer)
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
                .style(move |_t: &Theme| {
                    text::Style {
                        color: Some(theme.action(ActionToken::StrongText)),
                    }
                }),
        ]
        .align_y(Vertical::Center),
    )
    .padding([14, 28])
    .on_press(OnboardingMessage::EnterPressed)
    .style(move |_t: &Theme, status| primary_action_style(theme, status));

    row![Space::new().width(Length::Fill), primary,]
        .align_y(Vertical::Center)
        .width(Length::Fill)
        .into()
}

fn feature_meta(index: usize) -> (&'static str, &'static str, &'static str, FeatureKind) {
    match index {
        0 => (
            "01.0",
            "Chat",
            "Ask local models with context kept on your machine.",
            FeatureKind::Chat,
        ),
        1 => (
            "02.0",
            "Terminal",
            "Run shell commands with permissioned workspace access.",
            FeatureKind::Terminal,
        ),
        2 => (
            "03.0",
            "Text Editor",
            "Review agent edits before anything writes to disk.",
            FeatureKind::TextEditor,
        ),
        3 => (
            "04.0",
            "Rust",
            "Native speed and memory safety for the whole workspace.",
            FeatureKind::Rust,
        ),
        _ => ("—", "—", "—", FeatureKind::Chat),
    }
}

fn pip(color: iced::Color, diameter: f32) -> Element<'static, OnboardingMessage> {
    container(Space::new())
        .width(Length::Fixed(diameter))
        .height(Length::Fixed(diameter))
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            border: iced::Border {
                radius: (diameter * 0.5).into(),
                width: 0.0,
                color: iced::Color::TRANSPARENT,
            },
            ..Default::default()
        })
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

fn feature_card_button_style(
    theme: OpenZoneTheme,
    active: bool,
    glow: f32,
    status: button::Status,
) -> button::Style {
    let accent = theme.foreground(ForegroundToken::Accent);
    let base = button::Style {
        background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
        text_color: theme.foreground(ForegroundToken::Primary),
        border: iced::Border {
            radius: RadiusToken::Sm.value().into(),
            width: 0.0,
            color: iced::Color::TRANSPARENT,
        },
        ..Default::default()
    };

    match status {
        button::Status::Hovered if !active => button::Style {
            background: Some(iced::Background::Color(with_alpha(
                accent,
                0.05 + glow * 0.03,
            ))),
            border: iced::Border {
                radius: RadiusToken::Sm.value().into(),
                width: 1.0,
                color: with_alpha(accent, 0.28 + glow * 0.2),
            },
            ..base
        },
        button::Status::Pressed => button::Style {
            background: Some(iced::Background::Color(with_alpha(accent, 0.10))),
            ..base
        },
        _ => base,
    }
}

fn blend_surface(from: iced::Color, to: iced::Color, amount: f32) -> iced::Color {
    let amount = amount.clamp(0.0, 1.0);
    iced::Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

fn with_alpha(color: iced::Color, alpha: f32) -> iced::Color {
    iced::Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}
