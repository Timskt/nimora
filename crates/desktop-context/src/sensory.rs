//! Stable, throttled sensory bands for OS samples → pet directives.
//!
//! Pure, I/O-free helpers so hosts can map battery / idle / meeting / crowding
//! samples into discrete bands and decide when a directive edge should fire.
//! Never reads window titles or pixel content.

use crate::snapshot::{MeetingHint, PowerState};

/// Re-emit the same low/critical battery band at most once per this interval.
pub const BATTERY_SAME_BAND_THROTTLE_MS: u64 = 60_000;

/// Minimum hold before a boolean sensor edge is considered stable (ms).
pub const BOOLEAN_SENSOR_HOLD_MS: u64 = 1_500;

/// Maximum windows retained for occlusion / obstacles (keeps samples stable).
pub const MAX_ENUMERATED_WINDOWS: usize = 64;

/// Idle active band: user still interacting.
pub const IDLE_BAND_ACTIVE: u8 = 0;
/// Idle notice band: ~90s+.
pub const IDLE_BAND_NOTICE: u8 = 1;
/// Idle approach band: ~5 min+.
pub const IDLE_BAND_APPROACH: u8 = 2;
/// Idle rest band: ~30 min+.
pub const IDLE_BAND_REST: u8 = 3;

/// Battery band: unknown / unavailable.
pub const BATTERY_BAND_UNKNOWN: u8 = 0;
/// Battery band: critical (≤10%, on battery, not charging).
pub const BATTERY_BAND_CRITICAL: u8 = 1;
/// Battery band: low (≤20%, on battery, not charging).
pub const BATTERY_BAND_LOW: u8 = 2;
/// Battery band: ok / AC / high.
pub const BATTERY_BAND_OK: u8 = 3;
/// Battery band: charging.
pub const BATTERY_BAND_CHARGING: u8 = 4;

/// Maps power state into a stable sensory band for directive throttling.
///
/// `0` = unknown, `1` = critical, `2` = low, `3` = ok/AC, `4` = charging.
#[must_use]
pub fn battery_sensory_band(power: Option<&PowerState>) -> u8 {
    let Some(power) = power else {
        return BATTERY_BAND_UNKNOWN;
    };
    if power.charging {
        return BATTERY_BAND_CHARGING;
    }
    if !power.on_battery {
        return BATTERY_BAND_OK;
    }
    match power.battery_percent {
        Some(percent) if percent <= 10 => BATTERY_BAND_CRITICAL,
        Some(percent) if percent <= 20 => BATTERY_BAND_LOW,
        _ => BATTERY_BAND_OK,
    }
}

/// Idle duration (seconds) → discrete sensory band.
///
/// `0` active, `1` ≥90s, `2` ≥5min, `3` ≥30min.
#[must_use]
pub const fn idle_sensory_band(idle_secs: u64) -> u8 {
    if idle_secs >= 30 * 60 {
        IDLE_BAND_REST
    } else if idle_secs >= 5 * 60 {
        IDLE_BAND_APPROACH
    } else if idle_secs >= 90 {
        IDLE_BAND_NOTICE
    } else {
        IDLE_BAND_ACTIVE
    }
}

/// Idle milliseconds → band (overflow-safe).
#[must_use]
pub const fn idle_sensory_band_from_ms(idle_ms: u64) -> u8 {
    idle_sensory_band(idle_ms / 1_000)
}

/// Whether battery sensory should emit a pet directive.
///
/// Threshold / charging transitions always win; same low/critical band re-emits
/// at most once per [`BATTERY_SAME_BAND_THROTTLE_MS`].
#[must_use]
pub const fn battery_should_emit(
    previous_band: u8,
    band: u8,
    last_emit_ms: u64,
    now_ms: u64,
) -> bool {
    if band == BATTERY_BAND_UNKNOWN {
        return false;
    }
    if previous_band != band {
        return matches!(
            band,
            BATTERY_BAND_CRITICAL | BATTERY_BAND_LOW | BATTERY_BAND_CHARGING
        ) || matches!(
            previous_band,
            BATTERY_BAND_CRITICAL | BATTERY_BAND_LOW | BATTERY_BAND_CHARGING
        );
    }
    matches!(band, BATTERY_BAND_CRITICAL | BATTERY_BAND_LOW)
        && now_ms.saturating_sub(last_emit_ms) >= BATTERY_SAME_BAND_THROTTLE_MS
}

/// Whether idle sensory should emit: only when crossing into a higher idle band.
#[must_use]
pub const fn idle_should_emit(previous_band: u8, band: u8) -> bool {
    band > previous_band
}

/// Whether meeting sensory should emit: edge-triggered on active flag change.
///
/// Hosts should pair this with `meeting_sensory_directive` and keep first healthy
/// inactive samples quiet (no cold-start "meeting ended" speech). Same-state
/// samples never emit.
#[must_use]
pub const fn meeting_should_emit(previous_active: bool, active: bool) -> bool {
    previous_active != active
}

/// Whether notification-unread sensory should emit: edge-triggered on the
/// privacy-preserving boolean only (no titles or bodies).
///
/// Hosts should pair this with `notification_sensory_directive` and keep the
/// first healthy "clear" sample quiet (no cold-start "messages cleared" speech).
#[must_use]
pub const fn notification_should_emit(previous_unread: bool, has_unread: bool) -> bool {
    previous_unread != has_unread
}

/// Aggregates app-local unread hints into a privacy-safe boolean.
///
/// Inputs are counts only — never notification titles, bodies, or senders.
#[must_use]
pub const fn notification_unread_from_counts(
    outbox_pending: u32,
    outbox_dead_letter: u32,
    attention_pending: u32,
) -> bool {
    outbox_pending > 0 || outbox_dead_letter > 0 || attention_pending > 0
}

/// Clamp idle milliseconds to a sane upper bound (7 days) for stable inputs.
#[must_use]
pub const fn sanitize_idle_ms(idle_ms: u64) -> u64 {
    const MAX_IDLE_MS: u64 = 7 * 24 * 60 * 60 * 1_000;
    if idle_ms > MAX_IDLE_MS {
        MAX_IDLE_MS
    } else {
        idle_ms
    }
}

/// Normalize a display scale factor for geometry conversion (1.0–4.0).
#[must_use]
pub fn sanitize_scale_factor(scale: f64) -> f64 {
    if !scale.is_finite() || scale < 0.5 {
        1.0
    } else {
        scale.clamp(0.5, 4.0)
    }
}

/// Converts a logical (points) axis-aligned rect into physical pixels via `scale`.
///
/// Used when platform window bounds are reported in points while the pet pose is
/// physical (Retina / per-monitor DPI). `scale == 1.0` is a no-op.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn logical_rect_to_physical(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
) -> (i32, i32, u32, u32) {
    let scale = sanitize_scale_factor(scale);
    if (scale - 1.0).abs() < f64::EPSILON {
        return (x, y, width, height);
    }
    let px = (f64::from(x) * scale).round();
    let py = (f64::from(y) * scale).round();
    let pw = (f64::from(width) * scale).round().max(0.0);
    let ph = (f64::from(height) * scale).round().max(0.0);
    let out_x = if px.is_finite() {
        px.clamp(i32::MIN as f64, i32::MAX as f64) as i32
    } else {
        x
    };
    let out_y = if py.is_finite() {
        py.clamp(i32::MIN as f64, i32::MAX as f64) as i32
    } else {
        y
    };
    let out_w = if pw.is_finite() {
        (pw.min(f64::from(u32::MAX))) as u32
    } else {
        width
    };
    let out_h = if ph.is_finite() {
        (ph.min(f64::from(u32::MAX))) as u32
    } else {
        height
    };
    (out_x, out_y, out_w, out_h)
}

/// Compact meeting label for host mapping (privacy-preserving; no titles).
#[must_use]
pub const fn meeting_hint_label(hint: MeetingHint) -> &'static str {
    match hint {
        MeetingHint::Zoom => "zoom",
        MeetingHint::Teams => "teams",
        MeetingHint::Meet => "meet",
        MeetingHint::Webex => "webex",
        MeetingHint::Unknown => "unknown",
        MeetingHint::None => "",
    }
}

/// Debounce gate for boolean sensors (fullscreen, DND, meeting).
///
/// Emits only after the new value has been observed continuously for the hold
/// window, preventing flicker spam into pet directives / presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BooleanSensorGate {
    published: Option<bool>,
    pending: Option<bool>,
    pending_since_ms: u64,
    hold_ms: u64,
}

impl Default for BooleanSensorGate {
    fn default() -> Self {
        Self::new(BOOLEAN_SENSOR_HOLD_MS)
    }
}

impl BooleanSensorGate {
    /// Builds a gate with a custom hold interval.
    #[must_use]
    pub const fn new(hold_ms: u64) -> Self {
        Self {
            published: None,
            pending: None,
            pending_since_ms: 0,
            hold_ms,
        }
    }

    /// Last stably published value, if any.
    #[must_use]
    pub const fn published(self) -> Option<bool> {
        self.published
    }

    /// Observes a raw sample. Returns `Some(stable)` only when the published
    /// value changes after the hold window.
    #[must_use]
    pub fn observe(mut self, value: bool, now_ms: u64) -> (Self, Option<bool>) {
        if self.published == Some(value) {
            self.pending = None;
            return (self, None);
        }
        match self.pending {
            Some(pending) if pending == value => {
                if now_ms.saturating_sub(self.pending_since_ms) >= self.hold_ms {
                    self.published = Some(value);
                    self.pending = None;
                    return (self, Some(value));
                }
                (self, None)
            }
            _ => {
                self.pending = Some(value);
                self.pending_since_ms = now_ms;
                (self, None)
            }
        }
    }

    /// Force-publish (tests / fail-closed reset).
    #[must_use]
    pub fn force(mut self, value: bool) -> Self {
        self.published = Some(value);
        self.pending = None;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_bands_cover_critical_low_charging() {
        assert_eq!(battery_sensory_band(None), BATTERY_BAND_UNKNOWN);
        assert_eq!(
            battery_sensory_band(Some(&PowerState {
                on_battery: true,
                battery_percent: Some(8),
                charging: false,
            })),
            BATTERY_BAND_CRITICAL
        );
        assert_eq!(
            battery_sensory_band(Some(&PowerState {
                on_battery: true,
                battery_percent: Some(15),
                charging: false,
            })),
            BATTERY_BAND_LOW
        );
        assert_eq!(
            battery_sensory_band(Some(&PowerState {
                on_battery: true,
                battery_percent: Some(15),
                charging: true,
            })),
            BATTERY_BAND_CHARGING
        );
        assert_eq!(
            battery_sensory_band(Some(&PowerState {
                on_battery: false,
                battery_percent: Some(50),
                charging: false,
            })),
            BATTERY_BAND_OK
        );
    }

    #[test]
    fn battery_emit_rules_throttle_same_band() {
        assert!(battery_should_emit(0, BATTERY_BAND_CRITICAL, 0, 1_000));
        assert!(!battery_should_emit(
            BATTERY_BAND_CRITICAL,
            BATTERY_BAND_CRITICAL,
            1_000,
            2_000
        ));
        assert!(battery_should_emit(
            BATTERY_BAND_CRITICAL,
            BATTERY_BAND_CRITICAL,
            1_000,
            1_000 + BATTERY_SAME_BAND_THROTTLE_MS
        ));
        assert!(battery_should_emit(
            BATTERY_BAND_CRITICAL,
            BATTERY_BAND_CHARGING,
            1_000,
            1_100
        ));
        assert!(!battery_should_emit(0, BATTERY_BAND_UNKNOWN, 0, 1_000));
    }

    #[test]
    fn idle_bands_and_emit_only_rise() {
        assert_eq!(idle_sensory_band(0), IDLE_BAND_ACTIVE);
        assert_eq!(idle_sensory_band(90), IDLE_BAND_NOTICE);
        assert_eq!(idle_sensory_band(5 * 60), IDLE_BAND_APPROACH);
        assert_eq!(idle_sensory_band(30 * 60), IDLE_BAND_REST);
        assert!(idle_should_emit(0, 1));
        assert!(!idle_should_emit(2, 1));
        assert!(!idle_should_emit(2, 2));
    }

    #[test]
    fn meeting_edge_and_boolean_gate_hold() {
        assert!(meeting_should_emit(false, true));
        assert!(meeting_should_emit(true, false));
        assert!(!meeting_should_emit(true, true));

        let gate = BooleanSensorGate::new(100);
        let (gate, emit) = gate.observe(true, 0);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(true, 50);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(true, 100);
        assert_eq!(emit, Some(true));
        let (gate, emit) = gate.observe(true, 200);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(false, 200);
        assert_eq!(emit, None);
        let (_gate, emit) = gate.observe(false, 300);
        assert_eq!(emit, Some(false));
    }

    #[test]
    fn logical_to_physical_scales_retina() {
        let (x, y, w, h) = logical_rect_to_physical(10, 20, 100, 50, 2.0);
        assert_eq!((x, y, w, h), (20, 40, 200, 100));
        let identity = logical_rect_to_physical(10, 20, 100, 50, 1.0);
        assert_eq!(identity, (10, 20, 100, 50));
    }

    #[test]
    fn notification_should_emit_edges() {
        assert!(notification_should_emit(false, true));
        assert!(notification_should_emit(true, false));
        assert!(!notification_should_emit(true, true));
        assert!(!notification_should_emit(false, false));
    }

    #[test]
    fn notification_unread_from_counts_privacy_safe() {
        assert!(!notification_unread_from_counts(0, 0, 0));
        assert!(notification_unread_from_counts(1, 0, 0));
        assert!(notification_unread_from_counts(0, 2, 0));
        assert!(notification_unread_from_counts(0, 0, 3));
    }
}
