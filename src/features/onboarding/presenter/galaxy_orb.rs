//! Galaxy orb canvas — the preserved centerpiece.
//!
//! Renders a realistic pixel-particle spiral galaxy. Each stellar
//! population uses physically-motivated colours:
//!
//! - **Spiral arms**: blue-white (hot O/B stars, young population I)
//! - **Inter-arm disc**: warm golden (intermediate-age stars)
//! - **Bulge**: warm yellow → gold (old population II)
//! - **Nucleus**: white-hot (AGN core)
//! - **HII regions**: pink-magenta (hydrogen-alpha emission)
//! - **Dust lanes**: dark reddish-brown (interstellar absorption)
//! - **Halo**: cool blue-white (old, metal-poor stars)
//! - **Jets**: pale cyan (synchrotron radiation)
//!
//! The spiral structure, differential rotation, and hold-to-zoom
//! dynamics are preserved exactly.

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
use crate::shared::design::tokens::ForegroundToken;

use crate::features::onboarding::application::onboarding_dynamics::{MAX_ZOOM, SPEED_CLAMP};

const LOGICAL_SIZE: Size = Size {
    width: 360.0,
    height: 240.0,
};

const DISC_RADIUS: f32 = 116.0;
const DISC_TILT: f32 = 0.40;
const ARM_COUNT: usize = 2;
const ARM_PITCH: f32 = 0.42;
const ARM_WIDTH: f32 = 0.55;

const DISC_STAR_COUNT: usize = 360;
const ARM_SATELLITE_COUNT: usize = 96;
const HALO_STAR_COUNT: usize = 168;
const GLOBULAR_CLUSTER_COUNT: usize = 36;
const BULGE_BLOCK_COUNT: usize = 70;
const NUCLEUS_BLOCK_COUNT: usize = 30;
const STARFIELD_COUNT: usize = 90;
const JET_SEGMENTS: usize = 16;

const SNAP_GRID: f32 = 3.0;

/// Galaxy palette with two accent zones:
///
/// - **Outer ring** → blue galaxy accent (from design theme)
/// - **Center orb** → sun accent (warm golden-orange)
///
/// Each layer blends its base colour toward the appropriate accent
/// based on radial distance from the nucleus.
struct GalaxyPalette {
    arm_young: Color,
    arm_old: Color,
    bulge: Color,
    nucleus: Color,
    hii_region: Color,
    dust: Color,
    halo: Color,
    jet: Color,
    starfield: Color,
    /// Blue galaxy accent from the design theme — used for outer ring.
    blue_galaxy: Color,
    /// Warm sun accent — used for center orb (bulge, nucleus, jets).
    sun: Color,
}

impl GalaxyPalette {
    fn from_theme(theme: &OpenZoneTheme) -> Self {
        let blue_galaxy = theme.foreground(ForegroundToken::Accent);
        Self {
            arm_young: Color::from_rgb(0.70, 0.80, 1.0),
            arm_old: Color::from_rgb(1.0, 0.85, 0.55),
            bulge: Color::from_rgb(1.0, 0.78, 0.45),
            nucleus: Color::from_rgb(1.0, 1.0, 0.95),
            hii_region: Color::from_rgb(1.0, 0.55, 0.70),
            dust: Color::from_rgb(0.25, 0.18, 0.15),
            halo: Color::from_rgb(0.60, 0.70, 0.95),
            jet: Color::from_rgb(0.50, 0.85, 0.95),
            starfield: Color::from_rgb(0.95, 0.95, 1.0),
            blue_galaxy,
            sun: Color::from_rgb(1.0, 0.72, 0.22),
        }
    }

    /// Radial tint: blends a base colour toward the appropriate accent.
    ///
    /// `r` is normalised radius in `[0, 1]` (0 = center, 1 = edge).
    /// Inner regions tint toward sun; outer regions tint toward blue galaxy.
    fn radial_tint(&self, base: Color, r: f32, strength: f32) -> Color {
        let r = r.clamp(0.0, 1.0);
        // Transition zone: 0.0–0.3 is pure sun, 0.3–0.7 blends, 0.7–1.0 is pure blue
        let blue_weight = ((r - 0.3) / 0.4).clamp(0.0, 1.0);
        let accent = blend(self.sun, self.blue_galaxy, blue_weight);
        blend(base, accent, strength)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GalaxyOrbProgram {
    theme: OpenZoneTheme,
    started_at: Instant,
    now: Instant,
    speed_multiplier: f32,
    zoom: f32,
}

impl GalaxyOrbProgram {
    pub fn new(theme: OpenZoneTheme, started_at: Instant, now: Instant) -> Self {
        Self::with_dynamics(theme, started_at, now, 1.0, 1.0)
    }

    pub fn with_dynamics(
        theme: OpenZoneTheme,
        started_at: Instant,
        now: Instant,
        speed_multiplier: f32,
        zoom: f32,
    ) -> Self {
        Self {
            theme,
            started_at,
            now,
            speed_multiplier: speed_multiplier.clamp(0.0, SPEED_CLAMP),
            zoom: zoom.clamp(1.0, MAX_ZOOM),
        }
    }

    fn elapsed_seconds(&self) -> f32 {
        self.now
            .saturating_duration_since(self.started_at)
            .as_secs_f32()
    }
}

impl<Message> Program<Message> for GalaxyOrbProgram {
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
        let t = self.elapsed_seconds() * self.speed_multiplier;
        let pal = GalaxyPalette::from_theme(&self.theme);

        let fit_scale =
            (bounds.width / LOGICAL_SIZE.width).min(bounds.height / LOGICAL_SIZE.height);
        let scale = fit_scale * self.zoom;
        let translate = Point {
            x: (bounds.width - LOGICAL_SIZE.width * scale) * 0.5,
            y: (bounds.height - LOGICAL_SIZE.height * scale) * 0.5,
        };

        let project = |p: Point| Point {
            x: translate.x + p.x * scale,
            y: translate.y + p.y * scale,
        };

        draw_starfield(&mut frame, &pal, t, scale, project);
        draw_galactic_halo(&mut frame, &pal, t, scale, project);
        draw_jet(&mut frame, &pal, t, scale, project);
        draw_globular_clusters(&mut frame, &pal, t, scale, project);
        draw_disc(&mut frame, &pal, t, scale, project);
        draw_arm_satellites(&mut frame, &pal, t, scale, project);
        draw_bulge(&mut frame, &pal, t, scale, project);
        draw_nucleus(&mut frame, &pal, t, scale, project);
        draw_scanline(&mut frame, &self.theme, t, scale, project);

        vec![frame.into_geometry()]
    }
}

fn draw_starfield(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    for i in 0..STARFIELD_COUNT {
        let seed = 13_700.0 + i as f32;
        let x = noise(seed, 3.0) * LOGICAL_SIZE.width;
        let y = noise(seed, 7.0) * LOGICAL_SIZE.height;

        let dx = x - LOGICAL_SIZE.width * 0.5;
        let dy = (y - LOGICAL_SIZE.height * 0.5) / DISC_TILT;
        if (dx * dx + dy * dy).sqrt() < DISC_RADIUS * 0.65 {
            continue;
        }

        let phase = noise(seed, 19.0) * std::f32::consts::TAU;
        let twinkle = ((t * 1.2 + phase).sin() * 0.5 + 0.5).powf(1.4);
        let alpha = (0.05 + twinkle * 0.40).clamp(0.0, 1.0);
        let size = scale.max(1.0) * (0.9 + noise(seed, 31.0) * 1.1);
        let color = pal.starfield;

        let p = project(Point {
            x: snap(x),
            y: snap(y),
        });
        frame.fill_rectangle(
            Point {
                x: p.x - size * 0.5,
                y: p.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(color, alpha),
        );
    }
}

fn draw_galactic_halo(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let cool = pal.halo;
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    for i in 0..HALO_STAR_COUNT {
        let seed = 2_900.0 + i as f32;
        let radial = 0.92 + noise(seed, 3.0).powf(0.65) * 0.55;
        let angle =
            noise(seed, 11.0) * std::f32::consts::TAU + t * (0.04 + noise(seed, 19.0) * 0.03);

        let jitter_x = (noise(seed, 29.0) - 0.5) * 12.0;
        let jitter_y = (noise(seed, 37.0) - 0.5) * 9.0;

        let p = Point {
            x: center.x + angle.cos() * DISC_RADIUS * radial + jitter_x,
            y: center.y + angle.sin() * DISC_RADIUS * radial * DISC_TILT + jitter_y,
        };

        let phase = noise(seed, 47.0) * std::f32::consts::TAU;
        let twinkle = ((t * 0.7 + phase).sin() * 0.5 + 0.5) * 0.18;
        let alpha = 0.06 + twinkle;
        let size = (1.0 + noise(seed, 53.0) * 1.6) * scale;

        // Apply blue galaxy tint to outer halo (radial > 0.9)
        let color = pal.radial_tint(cool, radial, 0.7);

        let projected = project(p);
        frame.fill_rectangle(
            Point {
                x: projected.x - size * 0.5,
                y: projected.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(color, alpha),
        );
    }
}

fn draw_jet(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let warm = pal.nucleus;
    let cool = pal.jet;
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    let swell = ((t * 0.35).sin() * 0.5 + 0.5).powf(1.4);
    let intensity = 0.35 + swell * 0.55;

    let inner = 12.0;
    let outer = 96.0;

    for direction in [-1.0, 1.0] {
        for s in 0..JET_SEGMENTS {
            let f = s as f32 / (JET_SEGMENTS - 1) as f32;
            let r = inner + (outer - inner) * f;

            let wobble = (t * 0.6 + f * 6.0 + direction).sin() * (1.0 + f * 3.5);
            let raw_x = center.x + wobble;
            let raw_y = center.y + direction * r;

            let p = project(Point {
                x: snap(raw_x),
                y: snap(raw_y),
            });

            let falloff = (1.0 - f).powf(1.1);
            let alpha = (intensity * falloff * 0.55).clamp(0.0, 1.0);
            let width = (3.0 - f * 1.6).max(1.0) * scale;
            let height = (2.0 + falloff * 1.8) * scale;

            // Use blue galaxy tint for jets (no sun accent)
            let base_colour = blend(warm, cool, f * 0.85);
            let colour = pal.radial_tint(base_colour, 0.8, 0.7);

            frame.fill_rectangle(
                Point {
                    x: p.x - width * 0.5,
                    y: p.y - height * 0.5,
                },
                Size { width, height },
                with_alpha(colour, alpha),
            );

            if f < 0.55 {
                let side_size = scale.max(1.0);
                for off in [-1.0, 1.0] {
                    frame.fill_rectangle(
                        Point {
                            x: p.x + off * width * 0.65 - side_size * 0.5,
                            y: p.y - side_size * 0.5,
                        },
                        Size {
                            width: side_size,
                            height: side_size,
                        },
                        with_alpha(colour, alpha * 0.5),
                    );
                }
            }
        }
    }
}

fn draw_globular_clusters(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    for i in 0..GLOBULAR_CLUSTER_COUNT {
        let seed = 11_200.0 + i as f32;
        let angle_offset = noise(seed, 13.0) * std::f32::consts::TAU;
        let orbit_radius = DISC_RADIUS * (0.35 + noise(seed, 17.0) * 0.55);
        let plane = 0.35 + noise(seed, 31.0) * 0.55;
        let phase = noise(seed, 53.0) * std::f32::consts::TAU;

        let omega = 0.10 + noise(seed, 23.0) * 0.16;
        let angle = angle_offset + t * omega + phase;

        let p = Point {
            x: center.x + angle.cos() * orbit_radius,
            y: center.y + angle.sin() * orbit_radius * plane,
        };

        let twinkle = ((t * 1.3 + phase).sin() * 0.5 + 0.5) * 0.42;
        let size = (1.6 + noise(seed, 23.0) * 1.8) * scale;

        // Use blue galaxy accent for globular clusters (no purple)
        let r_norm = orbit_radius / DISC_RADIUS;
        let colour = pal.radial_tint(pal.arm_young, r_norm, 0.8);

        let projected = project(p);
        let glow_size = size * 1.8;
        frame.fill_rectangle(
            Point {
                x: projected.x - glow_size * 0.5,
                y: projected.y - glow_size * 0.5,
            },
            Size {
                width: glow_size,
                height: glow_size,
            },
            with_alpha(colour, 0.06 + twinkle * 0.10),
        );
        frame.fill_rectangle(
            Point {
                x: projected.x - size * 0.5,
                y: projected.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(colour, 0.18 + twinkle * 0.28),
        );
    }
}

fn draw_disc(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    let mut placed = 0usize;
    let mut attempt = 0usize;
    let max_attempts = DISC_STAR_COUNT * 14;

    while placed < DISC_STAR_COUNT && attempt < max_attempts {
        let seed = 2_400.0 + attempt as f32;
        let nx = noise(seed, 3.0) * 2.0 - 1.0;
        let ny = noise(seed, 9.0) * 2.0 - 1.0;
        let r = (nx * nx + ny * ny).sqrt();

        if !(0.04..=1.0).contains(&r) {
            attempt += 1;
            continue;
        }

        let (density, arm_distance) = arm_density(nx, ny, t);
        if noise(seed, 15.0) > density {
            attempt += 1;
            continue;
        }

        let raw_x = center.x + nx * DISC_RADIUS;
        let raw_y = center.y + ny * DISC_RADIUS * DISC_TILT;

        let phase = noise(seed, 53.0) * std::f32::consts::TAU;
        let drift_x = (t * 0.5 + phase).sin() * 0.9;
        let drift_y = (t * 0.4 + phase * 1.3).cos() * 0.6;

        let block_x = snap(raw_x + drift_x);
        let block_y = snap(raw_y + drift_y);

        let energy = (density * (1.0 - arm_distance * 0.4)).clamp(0.0, 1.0);

        let base_colour = if energy > 0.7 {
            let hii_blend = noise(seed, 97.0);
            if hii_blend > 0.85 {
                blend(pal.arm_young, pal.hii_region, (hii_blend - 0.85) * 6.67)
            } else {
                pal.arm_young
            }
        } else if energy > 0.3 {
            let interm = (energy - 0.3) / 0.4;
            blend(pal.arm_old, pal.arm_young, interm)
        } else if arm_distance < 0.2 {
            pal.dust
        } else {
            blend(pal.arm_old, pal.dust, 0.3)
        };

        // Apply radial tinting: inner disc → sun, outer disc → blue galaxy
        let colour = pal.radial_tint(base_colour, r, 0.45);

        let shimmer_phase = noise(seed, 71.0) * std::f32::consts::TAU;
        let shimmer = ((t * 1.4 + shimmer_phase).sin() * 0.5 + 0.5) * (0.18 + energy * 0.18);
        let base_alpha = 0.22 + energy * 0.65;
        let alpha = (base_alpha * (0.78 + shimmer)).clamp(0.05, 1.0);

        let pulse_phase = noise(seed, 89.0) * std::f32::consts::TAU;
        let pulse = (t * 1.0 + pulse_phase).sin() * 0.5 + 0.5;
        let block_size = (2.6 + energy * 4.2 + pulse * 0.7).clamp(2.0, 8.0);

        let projected = project(Point {
            x: block_x,
            y: block_y,
        });
        let size = block_size * scale;
        frame.fill_rectangle(
            Point {
                x: projected.x - size * 0.5,
                y: projected.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(colour, alpha),
        );

        if energy > 0.55 {
            let hi_size = (size * 0.32).max(scale * 1.2);
            let hot = blend(
                colour,
                Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                0.55,
            );
            frame.fill_rectangle(
                Point {
                    x: projected.x - size * 0.5 + hi_size * 0.4,
                    y: projected.y - size * 0.5 + hi_size * 0.4,
                },
                Size {
                    width: hi_size,
                    height: hi_size,
                },
                with_alpha(hot, (alpha * 0.85).clamp(0.0, 1.0)),
            );
        }

        placed += 1;
        attempt += 1;
    }
}

fn draw_arm_satellites(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let warm = pal.arm_young;
    let cool = pal.hii_region;
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    let mut placed = 0usize;
    let mut attempt = 0usize;
    let max_attempts = ARM_SATELLITE_COUNT * 18;

    while placed < ARM_SATELLITE_COUNT && attempt < max_attempts {
        let seed = 5_500.0 + attempt as f32;
        let r = 0.18 + noise(seed, 3.0).powf(0.8) * 0.78;
        let arm_index = (noise(seed, 7.0) * ARM_COUNT as f32).floor();
        let arm_jitter = (noise(seed, 11.0) - 0.5) * ARM_WIDTH * 0.6;
        let omega = 0.18 / (0.40 + r);
        let theta_arm = ARM_PITCH * (1.0 + r * 6.0).ln()
            + arm_index * std::f32::consts::TAU / ARM_COUNT as f32
            + arm_jitter
            - t * omega;

        let nx = r * theta_arm.cos();
        let ny = r * theta_arm.sin();

        let raw_x = center.x + nx * DISC_RADIUS;
        let raw_y = center.y + ny * DISC_RADIUS * DISC_TILT;

        let phase = noise(seed, 29.0) * std::f32::consts::TAU;
        let twinkle = ((t * 1.3 + phase).sin() * 0.5 + 0.5) * 0.55;

        let base_colour = if noise(seed, 83.0) > 0.75 { cool } else { warm };
        // Apply blue galaxy tint to outer arm satellites
        let colour = pal.radial_tint(base_colour, r, 0.6);
        let alpha = (0.30 + twinkle * 0.45).clamp(0.0, 1.0);
        let size = (2.2 + (1.0 - r) * 2.0) * scale;

        let projected = project(Point {
            x: snap(raw_x),
            y: snap(raw_y),
        });
        let glow_size = size * 1.7;
        frame.fill_rectangle(
            Point {
                x: projected.x - glow_size * 0.5,
                y: projected.y - glow_size * 0.5,
            },
            Size {
                width: glow_size,
                height: glow_size,
            },
            with_alpha(colour, alpha * 0.30),
        );
        frame.fill_rectangle(
            Point {
                x: projected.x - size * 0.5,
                y: projected.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(colour, alpha),
        );

        placed += 1;
        attempt += 1;
    }
}

fn draw_bulge(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let warm = pal.bulge;
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    let breath = (t * 0.6).sin() * 0.5 + 0.5;
    let breath_alpha = 0.55 + breath * 0.30;

    let mut placed = 0usize;
    let mut attempt = 0usize;
    let max_attempts = BULGE_BLOCK_COUNT * 16;

    while placed < BULGE_BLOCK_COUNT && attempt < max_attempts {
        let seed = 6_200.0 + attempt as f32;
        let nx = noise(seed, 3.0) * 2.0 - 1.0;
        let ny = noise(seed, 9.0) * 2.0 - 1.0;
        let density = gaussian2d(nx, ny, 0.30, 0.24);

        if noise(seed, 15.0) > density {
            attempt += 1;
            continue;
        }

        let r = (nx * nx + ny * ny).sqrt();
        let phase = noise(seed, 53.0) * std::f32::consts::TAU;
        let drift_x = (t * 0.7 + phase).sin() * 0.7;
        let drift_y = (t * 0.5 + phase * 1.3).cos() * 0.5;

        let raw_x = center.x + nx * 38.0 + drift_x;
        let raw_y = center.y + ny * 32.0 + drift_y;

        let block_x = snap(raw_x);
        let block_y = snap(raw_y);

        // Apply sun accent tint to center orb
        let r_norm = r / 0.38; // Normalize to 0-1 range
        let tinted = pal.radial_tint(warm, r_norm, 0.7);

        let hot = blend(
            tinted,
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            (1.0 - r * 1.3).clamp(0.0, 0.6),
        );

        let shimmer_phase = noise(seed, 71.0) * std::f32::consts::TAU;
        let shimmer = (t * 1.6 + shimmer_phase).sin() * 0.5 + 0.5;
        let alpha = (breath_alpha * density * (0.7 + shimmer * 0.3)).clamp(0.06, 0.95);
        let size = (2.4 + density * 3.4) * scale;

        let projected = project(Point {
            x: block_x,
            y: block_y,
        });
        frame.fill_rectangle(
            Point {
                x: projected.x - size * 0.5,
                y: projected.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(hot, alpha),
        );
        placed += 1;
        attempt += 1;
    }
}

fn draw_nucleus(
    frame: &mut Frame,
    pal: &GalaxyPalette,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let warm = pal.nucleus;
    // Apply strong sun accent to the very center
    let tinted = pal.radial_tint(warm, 0.1, 0.85);
    let hot = blend(
        tinted,
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        0.65,
    );
    let center = Point {
        x: LOGICAL_SIZE.width * 0.5,
        y: LOGICAL_SIZE.height * 0.5,
    };

    let breath = (t * 0.85).sin() * 0.5 + 0.5;
    let breath_alpha = 0.65 + breath * 0.30;

    let mut placed = 0usize;
    let mut attempt = 0usize;
    let max_attempts = NUCLEUS_BLOCK_COUNT * 16;

    while placed < NUCLEUS_BLOCK_COUNT && attempt < max_attempts {
        let seed = 7_700.0 + attempt as f32;
        let nx = noise(seed, 3.0) * 2.0 - 1.0;
        let ny = noise(seed, 9.0) * 2.0 - 1.0;
        let density = gaussian2d(nx, ny, 0.16, 0.14);

        if noise(seed, 15.0) < density {
            let phase = noise(seed, 53.0) * std::f32::consts::TAU;
            let drift_x = (t * 0.9 + phase).sin() * 0.6;
            let drift_y = (t * 0.7 + phase * 1.3).cos() * 0.5;

            let raw_x = center.x + nx * 18.0 + drift_x;
            let raw_y = center.y + ny * 14.0 + drift_y;

            let shimmer_phase = noise(seed, 71.0) * std::f32::consts::TAU;
            let shimmer = (t * 2.4 + shimmer_phase).sin() * 0.5 + 0.5;
            let alpha = (breath_alpha * (0.7 + shimmer * 0.3)).clamp(0.2, 1.0);
            let size = (2.4 + density * 3.0) * scale;

            let projected = project(Point {
                x: snap(raw_x),
                y: snap(raw_y),
            });
            frame.fill_rectangle(
                Point {
                    x: projected.x - size * 0.5,
                    y: projected.y - size * 0.5,
                },
                Size {
                    width: size,
                    height: size,
                },
                with_alpha(hot, alpha),
            );
            placed += 1;
        }
        attempt += 1;
    }
}

fn draw_scanline(
    frame: &mut Frame,
    theme: &OpenZoneTheme,
    t: f32,
    scale: f32,
    project: impl Fn(Point) -> Point,
) {
    let accent = theme.foreground(ForegroundToken::Accent);

    let cycle = 8.0;
    let phase = ((t / cycle) - (t / cycle).floor()).clamp(0.0, 1.0);
    let band_y = phase * LOGICAL_SIZE.height;

    let cols = 60;
    for c in 0..cols {
        let fx = c as f32 / (cols - 1) as f32;
        let x = fx * LOGICAL_SIZE.width;
        let jitter = (noise(c as f32, 3.0) - 0.5) * 1.2;
        let p = project(Point {
            x: snap(x),
            y: snap(band_y + jitter),
        });
        let edge = (1.0 - (fx - 0.5).abs() * 1.8).clamp(0.0, 1.0);
        let alpha = 0.06 * edge;
        let size = scale * 1.6;

        frame.fill_rectangle(
            Point {
                x: p.x - size * 0.5,
                y: p.y - size * 0.5,
            },
            Size {
                width: size,
                height: size,
            },
            with_alpha(accent, alpha),
        );
    }
}

fn arm_density(x: f32, y: f32, t: f32) -> (f32, f32) {
    let r = (x * x + y * y).sqrt();
    if r < 1e-3 {
        return (0.0, 0.0);
    }

    let theta = y.atan2(x);
    let omega = 0.18 / (0.40 + r);
    let theta_r = theta + t * omega;

    let arm_phase = theta_r - ARM_PITCH * (1.0 + r * 6.0).ln();
    let arm_n = ARM_COUNT as f32;
    let wrapped = wrap_pi(arm_phase * arm_n) / arm_n;
    let arm_distance = (wrapped.abs() / (std::f32::consts::PI / arm_n)).clamp(0.0, 1.0);

    let arm_strength = (-(wrapped / ARM_WIDTH).powi(2) * 4.0).exp();

    let envelope = (-((r - 0.55) / 0.32).powi(2)).exp() * 0.85 + (1.0 - r).max(0.0).powi(2) * 0.25;

    let density = (arm_strength * envelope).clamp(0.0, 1.0);
    (density, arm_distance)
}

fn wrap_pi(angle: f32) -> f32 {
    let two_pi = std::f32::consts::TAU;
    let mut a = angle % two_pi;
    if a > std::f32::consts::PI {
        a -= two_pi;
    } else if a <= -std::f32::consts::PI {
        a += two_pi;
    }
    a
}

fn gaussian2d(x: f32, y: f32, sigma_x: f32, sigma_y: f32) -> f32 {
    (-0.5 * ((x / sigma_x).powi(2) + (y / sigma_y).powi(2))).exp()
}

fn noise(value: f32, seed: f32) -> f32 {
    let mixed = (value * 12.9898 + seed * 78.233).sin() * 43_758.547;
    mixed - mixed.floor()
}

fn snap(value: f32) -> f32 {
    (value / SNAP_GRID).round() * SNAP_GRID
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}

fn blend(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_in_unit_range() {
        for i in 0..256 {
            let n = noise(i as f32, (i as f32) * 1.7);
            assert!((0.0..=1.0).contains(&n));
        }
    }

    #[test]
    fn noise_deterministic() {
        let a = noise(7.0, 13.0);
        let b = noise(7.0, 13.0);
        assert_eq!(a, b);
    }

    #[test]
    fn blend_endpoints() {
        let a = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let b = Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        assert_eq!(blend(a, b, 0.0), a);
        assert_eq!(blend(a, b, 1.0), b);
    }
}
