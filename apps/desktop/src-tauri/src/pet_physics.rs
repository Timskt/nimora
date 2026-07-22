//! Spring-damper motion for the desktop lifeform's native window.
//!
//! The autonomous roam loop previously moved the pet with a fixed twelve-frame
//! linear interpolation (`start + delta * frame / frames`). Linear motion reads
//! as a mechanical slide: constant speed, instant start, instant stop. A living
//! creature accelerates, decelerates, and — when underdamped — overshoots and
//! settles. This module models that with a critically-dampable spring so the
//! host can drive the window toward a target with organic easing while keeping
//! the existing per-frame interruption and safety gates.
//!
//! The model is a classic damped harmonic oscillator integrated with a
//! symplectic (semi-implicit) Euler scheme. Each visible frame is internally
//! sub-stepped so a stiff spring stays numerically stable regardless of the
//! caller's frame duration; see `ADR-009` for why this was chosen over the
//! analytical closed form and over a plain tween.
//!
//! The module is deliberately Tauri-free and operates on plain scalars so it
//! can be unit-tested in isolation and stays within the workspace architecture
//! boundary (no window, renderer, or IPC dependency).

/// Longest internal integration sub-step, in seconds.
///
/// A visible frame is split into whole sub-steps no longer than this so the
/// semi-implicit integrator remains stable for stiff springs even when the
/// caller drives it at a coarse frame rate. `1/240 s` keeps motion stable for
/// the stiffness range this crate uses while adding only a few arithmetic
/// operations per frame.
const MAX_SUBSTEP_SECONDS: f64 = 1.0 / 240.0;

/// A one-dimensional spring-damper carrying its own position and velocity.
///
/// Drive it toward a moving or fixed target by calling [`Spring::advance`] once
/// per frame. Construct one spring per spatial axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spring {
    position: f64,
    velocity: f64,
    stiffness: f64,
    damping: f64,
}

impl Spring {
    /// Creates a spring at rest at `position` with an explicit `stiffness`
    /// (restoring force per unit displacement) and `damping` (resistive force
    /// per unit velocity). Both coefficients are clamped to be non-negative;
    /// non-finite inputs collapse to zero so a caller can never inject `NaN`
    /// motion into the native window position.
    #[must_use]
    pub(crate) fn new(position: f64, stiffness: f64, damping: f64) -> Self {
        Self {
            position: sanitize(position),
            velocity: 0.0,
            stiffness: sanitize(stiffness).max(0.0),
            damping: sanitize(damping).max(0.0),
        }
    }

    /// Creates a critically damped spring: the fastest approach to the target
    /// that never overshoots. Critical damping for a unit mass is
    /// `2 * sqrt(stiffness)`.
    ///
    /// Use this for calm, purposeful motion (walking to a resting spot). For a
    /// springier, playful feel pass a smaller damping to [`Spring::new`] to
    /// allow overshoot and settle.
    #[must_use]
    pub(crate) fn critically_damped(position: f64, stiffness: f64) -> Self {
        let stiffness = sanitize(stiffness).max(0.0);
        Self::new(position, stiffness, 2.0 * stiffness.sqrt())
    }

    /// Advances the spring toward `target` over `dt` seconds and returns the new
    /// position.
    ///
    /// A non-positive or non-finite `dt` is a no-op, so a stalled or misbehaving
    /// clock can never teleport the pet. The frame is integrated in whole
    /// sub-steps of at most [`MAX_SUBSTEP_SECONDS`] for stability.
    pub(crate) fn advance(&mut self, target: f64, dt: f64) -> f64 {
        let target = sanitize(target);
        if !(dt.is_finite() && dt > 0.0) {
            return self.position;
        }
        // Split the frame into equal whole sub-steps no longer than the stable
        // ceiling. `steps` is always >= 1 and finite because `dt` is finite and
        // positive here.
        let steps = (dt / MAX_SUBSTEP_SECONDS).ceil().max(1.0);
        let h = dt / steps;
        let mut taken = 0.0_f64;
        while taken < steps {
            // Semi-implicit Euler: integrate velocity first, then position with
            // the freshly updated velocity. This is markedly more stable than
            // explicit Euler for oscillators.
            let restoring = -self.stiffness * (self.position - target);
            let resistive = -self.damping * self.velocity;
            self.velocity += (restoring + resistive) * h;
            self.position += self.velocity * h;
            taken += 1.0;
        }
        self.position = sanitize(self.position);
        self.velocity = sanitize(self.velocity);
        self.position
    }

    /// Reports whether the spring has effectively arrived: within
    /// `position_epsilon` of `target` and slower than `velocity_epsilon`.
    ///
    /// Both thresholds must hold; a fast pass through the target does not count
    /// as settled, which is what lets an underdamped spring overshoot and swing
    /// back before coming to rest.
    #[must_use]
    pub(crate) fn is_settled(
        &self,
        target: f64,
        position_epsilon: f64,
        velocity_epsilon: f64,
    ) -> bool {
        (self.position - sanitize(target)).abs() <= position_epsilon.abs()
            && self.velocity.abs() <= velocity_epsilon.abs()
    }
}

/// Replaces a non-finite value with `0.0`, leaving finite values untouched.
///
/// Every value entering the integrator passes through here so infinities and
/// `NaN` can never propagate into the window coordinate the host later rounds
/// and applies.
fn sanitize(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STIFFNESS: f64 = 120.0;
    const DT: f64 = 0.025;

    /// Asserts two finite values are within `EPS` of each other. `assert_eq!` on
    /// floats is rejected by pedantic clippy (`float_cmp`) and is wrong in
    /// principle for integrated motion, so tests compare with a tolerance.
    fn approx(actual: f64, expected: f64) {
        const EPS: f64 = 1e-9;
        assert!(
            (actual - expected).abs() <= EPS,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn critically_damped_spring_approaches_without_overshooting() {
        let mut spring = Spring::critically_damped(0.0, STIFFNESS);
        let mut previous = spring.position;
        let mut max_position = spring.position;
        for _ in 0..600 {
            let position = spring.advance(100.0, DT);
            max_position = max_position.max(position);
            // Monotonic, non-decreasing approach from below the target.
            assert!(position >= previous - 1e-9, "critical damping regressed");
            previous = position;
        }
        // Never crosses the target (no overshoot) and effectively arrives.
        assert!(max_position <= 100.0 + 1e-6, "critical damping overshot");
        assert!(spring.is_settled(100.0, 0.5, 0.5), "did not settle");
    }

    #[test]
    fn underdamped_spring_overshoots_then_settles() {
        // Light damping relative to critical -> springy overshoot.
        let mut spring = Spring::new(0.0, STIFFNESS, 2.0);
        let mut overshot = false;
        for _ in 0..2000 {
            let position = spring.advance(100.0, DT);
            if position > 100.0 + 1.0 {
                overshot = true;
            }
        }
        assert!(overshot, "underdamped spring should overshoot the target");
        assert!(
            spring.is_settled(100.0, 0.5, 0.5),
            "underdamped spring should still come to rest"
        );
    }

    #[test]
    fn overdamped_spring_never_overshoots_but_still_arrives() {
        let stiffness = STIFFNESS;
        // Damping well above critical -> sluggish, no overshoot.
        let mut spring = Spring::new(0.0, stiffness, 4.0 * stiffness.sqrt());
        let mut max_position = spring.position;
        for _ in 0..4000 {
            max_position = max_position.max(spring.advance(100.0, DT));
        }
        assert!(max_position <= 100.0 + 1e-6, "overdamped spring overshot");
        assert!(
            spring.is_settled(100.0, 0.75, 0.75),
            "overdamped spring should eventually arrive"
        );
    }

    #[test]
    fn non_positive_or_non_finite_dt_is_a_no_op() {
        let mut spring = Spring::critically_damped(10.0, STIFFNESS);
        approx(spring.advance(500.0, 0.0), 10.0);
        approx(spring.advance(500.0, -1.0), 10.0);
        approx(spring.advance(500.0, f64::NAN), 10.0);
        approx(spring.advance(500.0, f64::INFINITY), 10.0);
        approx(spring.position, 10.0);
        approx(spring.velocity, 0.0);
    }

    #[test]
    fn non_finite_target_is_sanitized_to_zero() {
        let mut spring = Spring::critically_damped(0.0, STIFFNESS);
        // A NaN target must not poison the position; it is treated as 0.0.
        for _ in 0..10 {
            let position = spring.advance(f64::NAN, DT);
            assert!(position.is_finite(), "NaN target produced non-finite motion");
        }
    }

    #[test]
    fn non_finite_coefficients_collapse_to_zero() {
        let spring = Spring::new(5.0, f64::NAN, f64::INFINITY);
        approx(spring.stiffness, 0.0);
        approx(spring.damping, 0.0);
        approx(spring.position, 5.0);
    }

    #[test]
    fn substepping_keeps_a_stiff_spring_stable_at_coarse_frames() {
        // A stiff spring driven at a coarse 100ms frame would diverge under a
        // single explicit Euler step; sub-stepping must keep it bounded.
        let mut spring = Spring::critically_damped(0.0, 900.0);
        let mut max_abs = 0.0_f64;
        for _ in 0..400 {
            max_abs = max_abs.max(spring.advance(100.0, 0.1).abs());
        }
        assert!(max_abs.is_finite(), "stiff spring diverged");
        assert!(
            max_abs <= 200.0,
            "stiff spring was not bounded near the target (max {max_abs})"
        );
        assert!(spring.is_settled(100.0, 1.0, 1.0), "stiff spring did not settle");
    }

    #[test]
    fn zero_stiffness_spring_does_not_move() {
        // With no restoring force and no initial velocity, the pet stays put.
        let mut spring = Spring::new(42.0, 0.0, 0.0);
        for _ in 0..100 {
            approx(spring.advance(1000.0, DT), 42.0);
        }
    }
}
