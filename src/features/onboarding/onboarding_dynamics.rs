//! Pure animation dynamics for the galaxy orb.

/// Speed multiplier at maximum hold progress.
pub const MAX_SPEED_MULTIPLIER: f32 = 3.0;

/// Hard ceiling for defensive clamping.
pub const SPEED_CLAMP: f32 = MAX_SPEED_MULTIPLIER + 0.5;

/// Zoom factor at maximum hold progress.
pub const MAX_ZOOM: f32 = 1.6;

/// Translate normalised hold-progress into `(speed_multiplier, zoom)`.
pub fn dynamics_for_progress(progress: f32) -> (f32, f32) {
    let p = progress.clamp(0.0, 1.0);
    let speed = 1.0 + (MAX_SPEED_MULTIPLIER - 1.0) * p;
    let zoom = 1.0 + (MAX_ZOOM - 1.0) * p;
    (speed, zoom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints() {
        let (speed, zoom) = dynamics_for_progress(0.0);
        assert!((speed - 1.0).abs() < 1e-6);
        assert!((zoom - 1.0).abs() < 1e-6);

        let (speed, zoom) = dynamics_for_progress(1.0);
        assert!((speed - MAX_SPEED_MULTIPLIER).abs() < 1e-6);
        assert!((zoom - MAX_ZOOM).abs() < 1e-6);
    }

    #[test]
    fn monotonic() {
        let mut prev_speed = 0.0;
        let mut prev_zoom = 0.0;
        for step in 0..=10 {
            let p = step as f32 / 10.0;
            let (speed, zoom) = dynamics_for_progress(p);
            assert!(speed >= prev_speed);
            assert!(zoom >= prev_zoom);
            prev_speed = speed;
            prev_zoom = zoom;
        }
    }
}
