//! Animated scene backdrop — subtle dot grid only.

use std::time::Instant;

use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::mouse;
use iced::widget::canvas::{Frame, Geometry, Program};

use crate::shared::design::OpenZoneTheme;
use crate::shared::design::design_tokens::ForegroundToken;

#[derive(Debug, Clone, Copy)]
pub struct SceneBackdrop {
    theme: OpenZoneTheme,
    started_at: Instant,
    now: Instant,
}

impl SceneBackdrop {
    pub fn new(theme: OpenZoneTheme, started_at: Instant, now: Instant) -> Self {
        Self {
            theme,
            started_at,
            now,
        }
    }

    fn elapsed(&self) -> f32 {
        self.now
            .saturating_duration_since(self.started_at)
            .as_secs_f32()
    }
}

impl<Message> Program<Message> for SceneBackdrop {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let t = self.elapsed();

        let dot_color = with_alpha(self.theme.foreground(ForegroundToken::Muted), 0.12);

        draw_dot_grid(&mut frame, bounds.size(), dot_color, t);

        vec![frame.into_geometry()]
    }
}

fn draw_dot_grid(frame: &mut Frame, size: Size, color: Color, t: f32) {
    let spacing = 28.0;
    let drift = (t * 0.08).sin() * 2.0;
    let cols = (size.width / spacing).ceil() as i32 + 1;
    let rows = (size.height / spacing).ceil() as i32 + 1;

    for row in 0..rows {
        for col in 0..cols {
            let x = col as f32 * spacing + drift;
            let y = row as f32 * spacing - drift * 0.5;
            let edge = edge_fade(x, y, size);
            let alpha = color.a * edge;
            if alpha < 0.01 {
                continue;
            }
            let dot = 1.2;
            frame.fill_rectangle(
                Point::new(x - dot * 0.5, y - dot * 0.5),
                Size::new(dot, dot),
                with_alpha(color, alpha),
            );
        }
    }
}

fn edge_fade(x: f32, y: f32, size: Size) -> f32 {
    let nx = (x / size.width - 0.5).abs() * 2.0;
    let ny = (y / size.height - 0.5).abs() * 2.0;
    let edge = nx.max(ny);
    (1.0 - (edge - 0.55).max(0.0) * 2.2).clamp(0.0, 1.0)
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}
