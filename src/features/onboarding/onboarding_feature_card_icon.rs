//! Wireframe feature icons for onboarding highlight cards.

use std::time::Instant;

use iced::Color;
use iced::Point;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::mouse;
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke};

use super::onboarding_feature_card_dynamics::accent_pulse;
use crate::shared::design::OpenZoneTheme;
use crate::shared::design::design_tokens::ForegroundToken;

const DESIGN_SIZE: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureKind {
    Chat,
    Terminal,
    TextEditor,
    Rust,
}

pub struct FeatureCardIcon {
    kind: FeatureKind,
    glow: f32,
    now: Instant,
    started_at: Instant,
    theme: OpenZoneTheme,
}

impl FeatureCardIcon {
    pub fn new(
        kind: FeatureKind,
        glow: f32,
        now: Instant,
        started_at: Instant,
        theme: OpenZoneTheme,
    ) -> Self {
        Self {
            kind,
            glow: glow.clamp(0.0, 1.0),
            now,
            started_at,
            theme,
        }
    }
}

impl<Message> Program<Message> for FeatureCardIcon {
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

        let accent = self.theme.foreground(ForegroundToken::Accent);
        let muted = self.theme.foreground(ForegroundToken::Muted);
        let pulse = accent_pulse(self.now, self.started_at, 1.35);
        let emphasis = self.glow.clamp(0.0, 1.0);
        let stroke_color = blend_colors(muted, accent, emphasis);
        let node = blend_colors(with_alpha(muted, 0.85), accent, emphasis);

        let fit = (bounds.width.min(bounds.height) / DESIGN_SIZE) * 0.84;
        let content = DESIGN_SIZE * fit;
        let mapper = Mapper {
            offset: Point {
                x: (bounds.width - content) * 0.5,
                y: (bounds.height - content) * 0.5,
            },
            fit,
        };
        let line = (1.05 * fit).max(0.7);
        let node_size = (3.0 * fit).max(1.4);
        let stroke = Stroke::default().with_width(line).with_color(stroke_color);

        if emphasis > 0.02 {
            let radius = fit * (34.0 + pulse * 4.0 * emphasis);
            let alpha = 0.04 + emphasis * (0.06 + pulse * 0.04);
            draw_glow(
                &mut frame,
                mapper.map(Point::new(60.0, 58.0)),
                radius,
                accent,
                alpha,
            );
        }

        match self.kind {
            FeatureKind::Chat => draw_chat(&mut frame, &mapper, stroke, node, node_size),
            FeatureKind::Terminal => draw_terminal(
                &mut frame,
                &mapper,
                stroke,
                stroke_color,
                node,
                node_size,
                fit,
            ),
            FeatureKind::TextEditor => {
                draw_text_editor(&mut frame, &mapper, stroke, stroke_color, node, node_size)
            }
            FeatureKind::Rust => {
                draw_rust(&mut frame, &mapper, stroke, stroke_color, node, node_size)
            }
        }

        vec![frame.into_geometry()]
    }
}

struct Mapper {
    offset: Point,
    fit: f32,
}

impl Mapper {
    fn map(&self, point: Point) -> Point {
        Point {
            x: self.offset.x + point.x * self.fit,
            y: self.offset.y + point.y * self.fit,
        }
    }
}

fn draw_glow(frame: &mut Frame, center: Point, radius: f32, accent: Color, peak_alpha: f32) {
    for (scale, alpha_scale) in [(1.0, 0.55), (0.72, 0.75), (0.46, 1.0)] {
        let size = radius * scale * 2.0;
        frame.fill_rectangle(
            Point::new(center.x - size * 0.5, center.y - size * 0.5),
            Size::new(size, size),
            with_alpha(accent, peak_alpha * alpha_scale),
        );
    }
}

fn draw_chat(frame: &mut Frame, mapper: &Mapper, stroke: Stroke<'_>, node: Color, node_size: f32) {
    stroke_rect(
        frame,
        mapper,
        Point::new(28.0, 28.0),
        Size::new(44.0, 36.0),
        stroke,
    );
    stroke_rect(
        frame,
        mapper,
        Point::new(58.0, 46.0),
        Size::new(44.0, 36.0),
        stroke,
    );

    for point in [
        Point::new(28.0, 28.0),
        Point::new(72.0, 28.0),
        Point::new(72.0, 64.0),
        Point::new(28.0, 64.0),
        Point::new(58.0, 46.0),
        Point::new(102.0, 46.0),
        Point::new(102.0, 82.0),
        Point::new(58.0, 82.0),
    ] {
        fill_node(frame, mapper.map(point), node_size, node);
    }

    stroke_line(
        frame,
        mapper.map(Point::new(72.0, 46.0)),
        mapper.map(Point::new(58.0, 46.0)),
        stroke,
    );
    stroke_line(
        frame,
        mapper.map(Point::new(48.0, 64.0)),
        mapper.map(Point::new(68.0, 82.0)),
        stroke,
    );
    stroke_line(
        frame,
        mapper.map(Point::new(38.0, 64.0)),
        mapper.map(Point::new(32.0, 74.0)),
        stroke,
    );
    stroke_line(
        frame,
        mapper.map(Point::new(32.0, 74.0)),
        mapper.map(Point::new(44.0, 64.0)),
        stroke,
    );
}

fn draw_terminal(
    frame: &mut Frame,
    mapper: &Mapper,
    stroke: Stroke<'_>,
    stroke_color: Color,
    node: Color,
    node_size: f32,
    fit: f32,
) {
    stroke_rect(
        frame,
        mapper,
        Point::new(24.0, 22.0),
        Size::new(72.0, 76.0),
        stroke,
    );

    for point in [
        Point::new(24.0, 22.0),
        Point::new(96.0, 22.0),
        Point::new(96.0, 98.0),
        Point::new(24.0, 98.0),
    ] {
        fill_node(frame, mapper.map(point), node_size, node);
    }

    stroke_line(
        frame,
        mapper.map(Point::new(24.0, 36.0)),
        mapper.map(Point::new(96.0, 36.0)),
        stroke,
    );

    for point in [
        Point::new(34.0, 29.0),
        Point::new(42.0, 29.0),
        Point::new(50.0, 29.0),
    ] {
        fill_node(frame, mapper.map(point), node_size * 0.75, node);
    }

    stroke_line(
        frame,
        mapper.map(Point::new(34.0, 50.0)),
        mapper.map(Point::new(44.0, 50.0)),
        stroke,
    );
    frame.fill_rectangle(
        mapper.map(Point::new(48.0, 47.0)),
        Size::new(6.0 * fit, 9.0 * fit),
        stroke_color,
    );

    for (y, width) in [(62.0, 48.0), (72.0, 36.0), (82.0, 54.0)] {
        stroke_line(
            frame,
            mapper.map(Point::new(34.0, y)),
            mapper.map(Point::new(34.0 + width, y)),
            stroke.with_color(with_alpha(stroke_color, 0.72)),
        );
    }
}

fn draw_text_editor(
    frame: &mut Frame,
    mapper: &Mapper,
    stroke: Stroke<'_>,
    stroke_color: Color,
    node: Color,
    node_size: f32,
) {
    stroke_rect(
        frame,
        mapper,
        Point::new(22.0, 20.0),
        Size::new(76.0, 80.0),
        stroke,
    );

    for point in [
        Point::new(22.0, 20.0),
        Point::new(98.0, 20.0),
        Point::new(98.0, 100.0),
        Point::new(22.0, 100.0),
    ] {
        fill_node(frame, mapper.map(point), node_size, node);
    }

    let divider = stroke.with_color(with_alpha(stroke_color, 0.55));
    stroke_line(
        frame,
        mapper.map(Point::new(40.0, 20.0)),
        mapper.map(Point::new(40.0, 100.0)),
        divider,
    );

    for (y, x0, width, emphasis) in [
        (36.0, 48.0, 42.0, 0.68),
        (46.0, 48.0, 32.0, 0.68),
        (56.0, 48.0, 46.0, 1.0),
        (66.0, 48.0, 28.0, 0.68),
        (76.0, 48.0, 38.0, 0.68),
        (86.0, 48.0, 30.0, 0.68),
    ] {
        let line_stroke = stroke.with_color(with_alpha(stroke_color, emphasis));
        stroke_line(
            frame,
            mapper.map(Point::new(x0, y)),
            mapper.map(Point::new(x0 + width, y)),
            line_stroke,
        );
    }

    for y in [36.0, 46.0, 56.0, 66.0, 76.0, 86.0] {
        fill_node(
            frame,
            mapper.map(Point::new(30.0, y)),
            node_size * 0.65,
            with_alpha(node, 0.75),
        );
    }
}

fn draw_rust(
    frame: &mut Frame,
    mapper: &Mapper,
    stroke: Stroke<'_>,
    stroke_color: Color,
    node: Color,
    node_size: f32,
) {
    let center = mapper.map(Point::new(60.0, 60.0));
    let points: [Point; 6] = [
        Point::new(60.0, 22.0),
        Point::new(92.0, 40.0),
        Point::new(92.0, 80.0),
        Point::new(60.0, 98.0),
        Point::new(28.0, 80.0),
        Point::new(28.0, 40.0),
    ]
    .map(|point| mapper.map(point));

    for index in 0..6 {
        stroke_line(frame, points[index], points[(index + 1) % 6], stroke);
    }

    for point in points {
        fill_node(frame, point, node_size, node);
    }

    fill_node(frame, center, node_size * 1.1, node);

    let spoke = stroke.with_color(with_alpha(stroke_color, 0.45));
    for point in points {
        stroke_line(frame, center, point, spoke);
    }

    let inner: [Point; 3] = [
        mapper.map(Point::new(60.0, 42.0)),
        mapper.map(Point::new(74.0, 68.0)),
        mapper.map(Point::new(46.0, 68.0)),
    ];
    let inner_stroke = stroke.with_color(with_alpha(stroke_color, 0.72));
    for index in 0..3 {
        stroke_line(frame, inner[index], inner[(index + 1) % 3], inner_stroke);
        fill_node(frame, inner[index], node_size * 0.8, node);
    }
}

fn stroke_rect(frame: &mut Frame, mapper: &Mapper, origin: Point, size: Size, stroke: Stroke<'_>) {
    frame.stroke_rectangle(
        mapper.map(origin),
        Size::new(size.width * mapper.fit, size.height * mapper.fit),
        stroke,
    );
}

fn stroke_line(frame: &mut Frame, from: Point, to: Point, stroke: Stroke<'_>) {
    frame.stroke(&Path::line(from, to), stroke);
}

fn fill_node(frame: &mut Frame, center: Point, size: f32, color: Color) {
    frame.fill_rectangle(
        Point::new(center.x - size * 0.5, center.y - size * 0.5),
        Size::new(size, size),
        color,
    );
}

fn blend_colors(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}
