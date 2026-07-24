//! Deterministic spring-damper motion, jump helpers, and collision response.
//!
//! Integration uses a fixed timestep with explicit Euler on acceleration derived
//! from a damped harmonic oscillator — no linear interpolation of position.

use serde::{Deserialize, Serialize};

/// Default stiffness for a desktop pet spring (px/s² per px of stretch).
pub const DEFAULT_STIFFNESS: f64 = 48.0;
/// Default damping coefficient (1/s scale after mass).
pub const DEFAULT_DAMPING: f64 = 14.0;
/// Default mass in abstract units.
pub const DEFAULT_MASS: f64 = 1.0;

/// Spring-damper parameters for the pet body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpringParams {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            stiffness: DEFAULT_STIFFNESS,
            damping: DEFAULT_DAMPING,
            mass: DEFAULT_MASS,
        }
    }
}

/// Continuous position and velocity in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpringState {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

impl SpringState {
    /// Constructs a rest state at the given origin.
    #[must_use]
    pub const fn at_rest(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
        }
    }

    /// Euclidean speed in px/s.
    #[must_use]
    pub fn speed(&self) -> f64 {
        self.vx.hypot(self.vy)
    }
}

/// Advances one fixed timestep toward `target` using spring-damper forces.
///
/// Force model:
/// `a = (k * (target - position) - c * velocity) / mass`
/// then `v += a * dt`, `p += v * dt`.
#[must_use]
pub fn integrate(
    state: SpringState,
    target_x: f64,
    target_y: f64,
    params: SpringParams,
    dt: f64,
) -> SpringState {
    if !dt.is_finite() || dt <= 0.0 || !params.mass.is_finite() || params.mass <= 0.0 {
        return state;
    }
    let ax = (params.stiffness * (target_x - state.x) - params.damping * state.vx) / params.mass;
    let ay = (params.stiffness * (target_y - state.y) - params.damping * state.vy) / params.mass;
    if !ax.is_finite() || !ay.is_finite() {
        return state;
    }
    let vx = state.vx + ax * dt;
    let vy = state.vy + ay * dt;
    let x = state.x + vx * dt;
    let y = state.y + vy * dt;
    SpringState { x, y, vx, vy }
}

/// Integrates until position and velocity settle near the target, or `max_steps`.
#[must_use]
pub fn integrate_to_target(
    mut state: SpringState,
    target_x: f64,
    target_y: f64,
    params: SpringParams,
    dt: f64,
    max_steps: u32,
    settle_epsilon: f64,
) -> SpringState {
    let epsilon = settle_epsilon.max(0.0);
    for _ in 0..max_steps {
        let next = integrate(state, target_x, target_y, params, dt);
        let position_error = (next.x - target_x).hypot(next.y - target_y);
        if position_error <= epsilon && next.speed() <= epsilon {
            return next;
        }
        state = next;
    }
    state
}

/// Samples a fixed-step spring trajectory toward a target, optionally bouncing.
///
/// When `bounds` is `Some((min_x, min_y, max_x, max_y))`, each step is resolved
/// with [`bounce_on_bounds`] so multi-display union stages keep the pet inside
/// the combined work-area origin box. Never uses linear position interpolation.
#[must_use]
pub fn sample_spring_trajectory(
    mut state: SpringState,
    target_x: f64,
    target_y: f64,
    params: SpringParams,
    dt: f64,
    max_steps: u32,
    settle_epsilon: f64,
    bounds: Option<(f64, f64, f64, f64)>,
    restitution: f64,
) -> Vec<SpringState> {
    let epsilon = settle_epsilon.max(0.0);
    let mut frames = Vec::with_capacity(max_steps as usize);
    for _ in 0..max_steps {
        state = integrate(state, target_x, target_y, params, dt);
        if let Some((min_x, min_y, max_x, max_y)) = bounds {
            state = bounce_on_bounds(state, min_x, min_y, max_x, max_y, restitution, 0.0).state;
        }
        frames.push(state);
        let position_error = (state.x - target_x).hypot(state.y - target_y);
        if position_error <= epsilon && state.speed() <= epsilon.max(1.0) {
            break;
        }
    }
    frames
}

/// Squash/stretch scale factors derived from velocity magnitude.
///
/// Returns `(scale_x, scale_y)` bounded to keep the pet readable.
#[must_use]
pub fn squash_stretch_scale(state: &SpringState, max_speed: f64) -> (f64, f64) {
    let speed = state.speed();
    let max_speed = if max_speed.is_finite() && max_speed > 0.0 {
        max_speed
    } else {
        400.0
    };
    let t = (speed / max_speed).clamp(0.0, 1.0);
    let stretch = 1.0 + 0.18 * t;
    let squash = 1.0 - 0.12 * t;
    let scale_x = stretch.clamp(0.85, 1.25);
    let scale_y = squash.clamp(0.85, 1.25);
    (scale_x, scale_y)
}

/// One sample along a parabolic jump trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JumpSample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

/// Samples a ballistic jump from `start` to `end` under constant gravity.
///
/// `peak_height` is the extra rise above the higher of start/end Y (screen Y grows down,
/// so peak is numerically smaller Y). Returns evenly spaced samples including endpoints.
#[must_use]
pub fn jump_parabola(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    peak_height: f64,
    sample_count: usize,
) -> Vec<JumpSample> {
    let samples = sample_count.max(2);
    let peak = peak_height.max(0.0);
    let apex_y = start_y.min(end_y) - peak;
    let mut out = Vec::with_capacity(samples);
    let denom = u32::try_from(samples.saturating_sub(1)).unwrap_or(1).max(1);
    for index in 0..samples {
        let numerator = u32::try_from(index).unwrap_or(u32::MAX);
        let t = f64::from(numerator) / f64::from(denom);
        let mid_x = (start_x + end_x) * 0.5;
        let one_minus = 1.0 - t;
        let x = one_minus * one_minus * start_x + 2.0 * one_minus * t * mid_x + t * t * end_x;
        let y = one_minus * one_minus * start_y + 2.0 * one_minus * t * apex_y + t * t * end_y;
        out.push(JumpSample { t, x, y });
    }
    out
}

/// Result of resolving a hard boundary collision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollisionOutcome {
    pub state: SpringState,
    /// True when impact speed exceeded the daze threshold.
    pub dazed: bool,
    pub bounced: bool,
}

/// Clamps the pet origin into bounds and reflects velocity on hard hits.
///
/// `min_*` / `max_*` are inclusive origin limits for the pet top-left.
/// `restitution` in \[0, 1\] scales reflected velocity. `daze_speed` marks hard hits.
#[must_use]
pub fn bounce_on_bounds(
    mut state: SpringState,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    restitution: f64,
    daze_speed: f64,
) -> CollisionOutcome {
    let restitution = restitution.clamp(0.0, 1.0);
    let mut bounced = false;
    let impact_speed_x;
    let impact_speed_y;

    if state.x < min_x {
        impact_speed_x = state.vx.abs();
        state.x = min_x;
        if state.vx < 0.0 {
            state.vx = -state.vx * restitution;
            bounced = true;
        }
    } else if state.x > max_x {
        impact_speed_x = state.vx.abs();
        state.x = max_x;
        if state.vx > 0.0 {
            state.vx = -state.vx * restitution;
            bounced = true;
        }
    } else {
        impact_speed_x = 0.0;
    }

    if state.y < min_y {
        impact_speed_y = state.vy.abs();
        state.y = min_y;
        if state.vy < 0.0 {
            state.vy = -state.vy * restitution;
            bounced = true;
        }
    } else if state.y > max_y {
        impact_speed_y = state.vy.abs();
        state.y = max_y;
        if state.vy > 0.0 {
            state.vy = -state.vy * restitution;
            bounced = true;
        }
    } else {
        impact_speed_y = 0.0;
    }

    let impact = impact_speed_x.hypot(impact_speed_y);
    let dazed = bounced && impact >= daze_speed.max(0.0);
    CollisionOutcome {
        state,
        dazed,
        bounced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_near_target_without_lerp() {
        let params = SpringParams::default();
        let start = SpringState::at_rest(0.0, 0.0);
        let settled = integrate_to_target(start, 100.0, 50.0, params, 1.0 / 60.0, 2_000, 0.5);
        assert!((settled.x - 100.0).abs() < 1.0, "x={}", settled.x);
        assert!((settled.y - 50.0).abs() < 1.0, "y={}", settled.y);
        assert!(settled.speed() < 1.0, "speed={}", settled.speed());
        let mid = integrate(start, 100.0, 50.0, params, 1.0 / 60.0);
        assert!(mid.vx > 0.0);
        assert!(mid.x > 0.0);
        // Not a pure linear step of the remaining distance over dt.
        let pure_linear_step = 100.0 * (1.0 / 60.0);
        assert!(
            (mid.x - pure_linear_step).abs() > 1e-6 || mid.vx != 0.0,
            "expected spring dynamics, got {mid:?}"
        );
    }

    #[test]
    fn integrator_is_deterministic() {
        let params = SpringParams::default();
        let start = SpringState {
            x: 10.0,
            y: 20.0,
            vx: 3.0,
            vy: -1.5,
        };
        let first = integrate(start, 80.0, 40.0, params, 1.0 / 60.0);
        let second = integrate(start, 80.0, 40.0, params, 1.0 / 60.0);
        assert_eq!(first, second);
        let mut left = start;
        let mut right = start;
        for _ in 0..120 {
            left = integrate(left, 80.0, 40.0, params, 1.0 / 60.0);
            right = integrate(right, 80.0, 40.0, params, 1.0 / 60.0);
        }
        assert_eq!(left, right);
    }

    #[test]
    fn bounds_clamp_and_bounce() {
        let state = SpringState {
            x: 105.0,
            y: 50.0,
            vx: 200.0,
            vy: 0.0,
        };
        let outcome = bounce_on_bounds(state, 0.0, 0.0, 100.0, 100.0, 0.5, 50.0);
        assert!(outcome.bounced);
        assert!(outcome.dazed);
        assert!((outcome.state.x - 100.0).abs() < 1e-9);
        assert!(outcome.state.vx < 0.0);
    }

    #[test]
    fn jump_samples_include_endpoints() {
        let samples = jump_parabola(0.0, 100.0, 200.0, 100.0, 40.0, 5);
        assert_eq!(samples.len(), 5);
        assert!((samples[0].x - 0.0).abs() < 1e-9);
        assert!((samples[0].y - 100.0).abs() < 1e-9);
        assert!((samples[4].x - 200.0).abs() < 1e-9);
        assert!((samples[4].y - 100.0).abs() < 1e-9);
        let mid = samples[2];
        assert!(mid.y < 100.0, "expected peak rise, y={}", mid.y);
    }

    #[test]
    fn squash_stretch_is_bounded() {
        let rest = SpringState::at_rest(0.0, 0.0);
        let (sx, sy) = squash_stretch_scale(&rest, 400.0);
        assert!((sx - 1.0).abs() < 1e-9);
        assert!((sy - 1.0).abs() < 1e-9);
        let fast = SpringState {
            x: 0.0,
            y: 0.0,
            vx: 1_000.0,
            vy: 0.0,
        };
        let (sx, sy) = squash_stretch_scale(&fast, 400.0);
        assert!((0.85..=1.25).contains(&sx));
        assert!((0.85..=1.25).contains(&sy));
    }

    #[test]
    fn sample_spring_trajectory_is_deterministic_and_progresses() {
        let params = SpringParams::default();
        let start = SpringState::at_rest(0.0, 0.0);
        let bounds = Some((0.0, 0.0, 2000.0, 800.0));
        let first = sample_spring_trajectory(
            start, 1600.0, 400.0, params, 1.0 / 60.0, 48, 0.75, bounds, 0.35,
        );
        let second = sample_spring_trajectory(
            start, 1600.0, 400.0, params, 1.0 / 60.0, 48, 0.75, bounds, 0.35,
        );
        assert_eq!(first, second);
        assert!(!first.is_empty());
        let last = first.last().copied().expect("last");
        assert!(last.x > 0.0, "expected eastward progress, last={last:?}");
        // Union-style wide bounds must not clamp a mid-span walk to a single monitor.
        assert!(last.x > 100.0, "multi-display span should allow x>100, last={last:?}");
    }

    #[test]
    fn sample_spring_trajectory_respects_tight_bounds() {
        let params = SpringParams::default();
        let start = SpringState::at_rest(10.0, 10.0);
        let frames = sample_spring_trajectory(
            start,
            500.0,
            500.0,
            params,
            1.0 / 60.0,
            24,
            0.5,
            Some((0.0, 0.0, 50.0, 50.0)),
            0.35,
        );
        assert!(!frames.is_empty());
        for frame in &frames {
            assert!(frame.x <= 50.0 + 1e-6, "x={}", frame.x);
            assert!(frame.y <= 50.0 + 1e-6, "y={}", frame.y);
            assert!(frame.x >= -1e-6, "x={}", frame.x);
            assert!(frame.y >= -1e-6, "y={}", frame.y);
        }
    }
}
