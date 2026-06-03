//! Animation helpers for onboarding feature highlight cards.

use std::time::Instant;

/// Exponential approach toward `target` over `dt` seconds.
pub fn approach(current: f32, target: f32, dt: f32, rate: f32) -> f32 {
    let dt = dt.clamp(0.0, 0.25);
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

/// Soft pulse in `[0.0, 1.0]` for live accent feedback.
pub fn accent_pulse(now: Instant, origin: Instant, speed: f32) -> f32 {
    let t = now.duration_since(origin).as_secs_f32();
    (t * speed * std::f32::consts::TAU).sin() * 0.5 + 0.5
}

/// Highlight level for one card given selection and hover.
pub fn highlight_target(selected: bool, hovered: bool) -> f32 {
    if selected {
        1.0
    } else if hovered {
        0.42
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_moves_toward_target() {
        let next = approach(0.0, 1.0, 0.1, 10.0);
        assert!(next > 0.0 && next < 1.0);
        assert!((approach(1.0, 1.0, 0.1, 10.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn highlight_target_prioritizes_selection() {
        assert_eq!(highlight_target(true, true), 1.0);
        assert_eq!(highlight_target(false, true), 0.42);
        assert_eq!(highlight_target(false, false), 0.0);
    }
}
