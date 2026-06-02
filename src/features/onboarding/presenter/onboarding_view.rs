//! Onboarding view — implements all 7 visual design sections.
//!
//! 1. Identity lock-up (top-left)
//! 2. Context marker (state code + label)
//! 3. Primary message (title + body)
//! 4. Demonstration (galaxy orb, top-center)
//! 5. Support detail (feature tiles)
//! 6. Progress (slide indicators)
//! 7. Actions (primary CTA + secondary skip)

use iced::Element;
use iced::Length;
use iced::Theme;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas::Canvas;
use iced::widget::{MouseArea, Row, Space, button, column, container, row, text};

use crate::shared::design::OpenZoneTheme;
use crate::shared::design::tokens::{
    AccentToken, ActionToken, BackgroundToken, BorderToken, ForegroundToken, RadiusToken,
    SpacingToken,
};

use crate::features::onboarding::application::onboarding_dynamics::dynamics_for_progress;
use crate::features::onboarding::application::onboarding_messages::OnboardingMessage;
use crate::features::onboarding::application::onboarding_state::{OnboardingState, SLIDE_COUNT};
use crate::features::onboarding::presenter::galaxy_orb::GalaxyOrbProgram;

/// Render the onboarding view.
pub fn view(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let main = column![
        identity_row(state),
        Space::new().height(Length::Fixed(SpacingToken::S6.value())),
        context_marker(state),
        Space::new().height(Length::Fixed(SpacingToken::S5.value())),
        hero_block(state),
        Space::new().height(Length::Fixed(SpacingToken::S6.value())),
        feature_tiles(state),
        Space::new().height(Length::Fixed(SpacingToken::S7.value())),
        progress_row(state),
        Space::new().height(Length::Fixed(SpacingToken::S6.value())),
        action_row(state),
    ]
    .max_width(780)
    .spacing(0)
    .width(Length::Fill);

    let centered = container(main)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([SpacingToken::S7.value(), SpacingToken::S7.value()])
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top)
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(
                theme.background(BackgroundToken::Primary),
            )),
            ..Default::default()
        });

    centered.into()
}

// ---------------------------------------------------------------------------
// 1. Identity lock-up
// ---------------------------------------------------------------------------

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

    // Skip control (top-right).
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

// ---------------------------------------------------------------------------
// 2. Context marker
// ---------------------------------------------------------------------------

fn context_marker(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let (code, label) = slide_meta(state.current_slide);

    let dot = pip(theme.foreground(ForegroundToken::Accent), 6.0);

    let code_text = text(code).size(10).style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Accent)),
    });

    let label_text = text(label).size(10).style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Secondary)),
    });

    let chip = container(
        row![
            dot,
            Space::new().width(Length::Fixed(8.0)),
            code_text,
            Space::new().width(Length::Fixed(12.0)),
            label_text,
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
    });

    chip.into()
}

// ---------------------------------------------------------------------------
// 3 + 4. Primary message + demonstration (galaxy orb)
// ---------------------------------------------------------------------------

fn hero_block(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    // The galaxy orb sits at the top center — preserved centerpiece.
    let orb = Canvas::new(GalaxyOrbProgram::with_dynamics(
        theme,
        state.started_at,
        state.now,
        state.displayed_speed,
        state.displayed_zoom,
    ))
    .width(Length::Fill)
    .height(Length::Fixed(220.0));

    let interactive_orb = MouseArea::new(orb)
        .on_press(OnboardingMessage::OrbPressed)
        .on_release(OnboardingMessage::OrbReleased)
        .interaction(iced::mouse::Interaction::Pointer);

    let orb_container = container(interactive_orb)
        .width(Length::Fill)
        .height(Length::Fixed(220.0));

    let badge = orb_state_badge(state);

    let (title, body) = slide_copy(state.current_slide);

    let headline = text(title).size(34).style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Primary)),
    });

    let subhead = text(body).size(14).style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Secondary)),
    });

    column![
        orb_container,
        Space::new().height(Length::Fixed(12.0)),
        badge,
        Space::new().height(Length::Fixed(20.0)),
        headline,
        Space::new().height(Length::Fixed(10.0)),
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

// ---------------------------------------------------------------------------
// 5. Support detail (feature tiles)
// ---------------------------------------------------------------------------

fn feature_tiles(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let tiles = slide_tiles(state.current_slide);

    let mut row_widget: Row<'_, OnboardingMessage> = Row::new().spacing(10).width(Length::Fill);
    for (code, title, body) in tiles {
        row_widget = row_widget.push(feature_tile(theme, code, title, body));
    }
    row_widget.into()
}

fn feature_tile<'a>(
    theme: OpenZoneTheme,
    code: &'a str,
    title: &'a str,
    body: &'a str,
) -> Element<'a, OnboardingMessage> {
    let cell = column![
        text(code).size(9).style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Accent)),
        }),
        Space::new().height(Length::Fixed(6.0)),
        text(title).size(12).style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Primary)),
        }),
        Space::new().height(Length::Fixed(6.0)),
        text(body).size(11).style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Muted)),
        }),
    ]
    .spacing(0);

    container(cell)
        .padding(14)
        .width(Length::FillPortion(1))
        .style(move |_t: &Theme| container::Style {
            background: Some(iced::Background::Color(
                theme.background(BackgroundToken::Secondary),
            )),
            border: iced::Border {
                radius: RadiusToken::Sm.value().into(),
                width: 1.0,
                color: theme.border(BorderToken::Default),
            },
            ..Default::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// 6. Progress indicators
// ---------------------------------------------------------------------------

fn progress_row(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;

    let mut dots: Row<'_, OnboardingMessage> = Row::new().spacing(6);
    for i in 0..SLIDE_COUNT {
        let is_current = i == state.current_slide;
        let color = if is_current {
            theme.foreground(ForegroundToken::Accent)
        } else {
            theme.foreground(ForegroundToken::Muted)
        };
        let (w, h) = if is_current { (20.0, 6.0) } else { (6.0, 6.0) };
        let dot = container(Space::new())
            .width(Length::Fixed(w))
            .height(Length::Fixed(h))
            .style(move |_t: &Theme| container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::Border {
                    radius: (h * 0.5).into(),
                    width: 0.0,
                    color: iced::Color::TRANSPARENT,
                },
                ..Default::default()
            });
        dots = dots.push(dot);
    }

    let counter = text(format!(
        "{:02} / {:02}",
        state.current_slide + 1,
        SLIDE_COUNT
    ))
    .size(10)
    .style(move |_t: &Theme| text::Style {
        color: Some(theme.foreground(ForegroundToken::Muted)),
    });

    row![
        dots,
        Space::new().width(Length::Fixed(16.0)),
        counter,
        Space::new().width(Length::Fill),
    ]
    .align_y(Vertical::Center)
    .into()
}

// ---------------------------------------------------------------------------
// 7. Action row
// ---------------------------------------------------------------------------

fn action_row(state: &OnboardingState) -> Element<'_, OnboardingMessage> {
    let theme = state.theme;
    let is_final = state.is_final_slide();

    let primary_label = if is_final {
        "Enter OpenZone"
    } else {
        "Continue"
    };

    let primary = button(
        row![text(primary_label).size(13).style(move |_t: &Theme| {
            text::Style {
                color: Some(theme.action(ActionToken::StrongText)),
            }
        }),]
        .align_y(Vertical::Center),
    )
    .padding([14, 28])
    .on_press(OnboardingMessage::EnterPressed)
    .style(move |_t: &Theme, status| primary_action_style(theme, status));

    let back_label = if state.current_slide > 0 {
        Some("Back")
    } else {
        None
    };

    let mut row_widget: Row<'_, OnboardingMessage> =
        Row::new().spacing(10).align_y(Vertical::Center);

    if let Some(label) = back_label {
        let back = button(text(label).size(12).style(move |_t: &Theme| text::Style {
            color: Some(theme.foreground(ForegroundToken::Secondary)),
        }))
        .padding([10, 16])
        .on_press(OnboardingMessage::PreviousSlide)
        .style(move |_t: &Theme, status| chip_style(theme, status));
        row_widget = row_widget.push(back);
    }

    row_widget = row_widget.push(Space::new().width(Length::Fill));
    row_widget = row_widget.push(primary);

    row_widget.into()
}

// ---------------------------------------------------------------------------
// Slide content
// ---------------------------------------------------------------------------

fn slide_meta(slide: usize) -> (&'static str, &'static str) {
    match slide {
        0 => ("SEC-01", "encrypted pairing"),
        1 => ("AI-02", "model workspace"),
        2 => ("RUN-03", "queue control"),
        3 => ("THK-04", "reasoning dial"),
        _ => ("—", "—"),
    }
}

fn slide_copy(slide: usize) -> (&'static str, &'static str) {
    match slide {
        0 => (
            "Pair trusted devices and keep chat context local.",
            "OpenZone encrypts your session context on-device. \
             Approve paired machines, audit what is sent to models, \
             and revoke access at any time.",
        ),
        1 => (
            "Steer models across providers from one workspace.",
            "Route prompts to the provider that fits the task. \
             Sessions stay local, secrets stay in your keychain, \
             and every response is reviewable.",
        ),
        2 => (
            "Queue the next prompt while the current turn runs.",
            "Long-running tasks, multi-step work, and agent loops \
             queue behind the active turn. Cancel, reorder, or \
             inspect any queued item before it ships.",
        ),
        3 => (
            "Set thinking before the model commits compute.",
            "Dial reasoning effort up for architecture and down for \
             quick lookups. Reviewable automation — you approve \
             before the agent writes to disk.",
        ),
        _ => ("", ""),
    }
}

fn slide_tiles(slide: usize) -> [(&'static str, &'static str, &'static str); 3] {
    match slide {
        0 => [
            ("// 01", "LOCAL", "Context stays on your machine."),
            ("// 02", "PAIRED", "Approve trusted devices."),
            ("// 03", "AUDITABLE", "Inspect every model call."),
        ],
        1 => [
            ("// 01", "ROUTED", "Providers picked per task."),
            ("// 02", "KEYCHAIN", "Secrets never leave the device."),
            ("// 03", "REVIEWABLE", "Every response, every prompt."),
        ],
        2 => [
            ("// 01", "QUEUED", "Tasks wait their turn."),
            ("// 02", "CANCELLABLE", "Kill any queued item."),
            ("// 03", "REPLAYABLE", "Rerun with full audit."),
        ],
        3 => [
            ("// 01", "DIALED", "Thinking effort, your call."),
            ("// 02", "GATED", "Approval before writes."),
            ("// 03", "REVERSIBLE", "Undo agent actions."),
        ],
        _ => [("", "", ""); 3],
    }
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

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

/// Graphite/near-black primary action per design spec: "Graphite for
/// commitment — primary actions use near-black when the action means
/// continue, enter, run, approve, or commit."
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
