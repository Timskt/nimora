//! Freshness and expiry policy for desktop snapshots.

use crate::snapshot::{DesktopSnapshot, Freshness};

/// Maximum allowed observation lifetime (hard cap at construction).
pub const MAX_SNAPSHOT_LIFETIME_MS: u64 = 30_000;

/// Returns true when the snapshot is past its expiry timestamp.
#[must_use]
pub const fn is_expired(snapshot: &DesktopSnapshot, now_ms: u64) -> bool {
    now_ms >= snapshot.expires_at_ms
}

/// A snapshot may drive full motion planning only while Fresh and not expired.
///
/// Stale and Degraded samples must not feed obstacle-dependent planners.
#[must_use]
pub const fn is_usable(snapshot: &DesktopSnapshot, now_ms: u64) -> bool {
    matches!(snapshot.freshness, Freshness::Fresh) && !is_expired(snapshot, now_ms)
}

/// Obstacle geometry is only trusted when the snapshot is usable.
#[must_use]
pub const fn obstacles_usable(snapshot: &DesktopSnapshot, now_ms: u64) -> bool {
    is_usable(snapshot, now_ms)
}

/// Recomputes freshness from wall-clock age without mutating other fields.
///
/// Expired samples become [`Freshness::Stale`]. Degraded stays degraded until
/// the host rebuilds the snapshot.
#[must_use]
pub fn refresh_freshness(snapshot: &DesktopSnapshot, now_ms: u64) -> Freshness {
    if matches!(snapshot.freshness, Freshness::Degraded) {
        return Freshness::Degraded;
    }
    if is_expired(snapshot, now_ms) {
        Freshness::Stale
    } else {
        Freshness::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{DegradationReason, MeetingHint, MeetingState};

    fn snap(freshness: Freshness, observed: u64, expires: u64) -> DesktopSnapshot {
        DesktopSnapshot {
            spec: crate::DESKTOP_CONTEXT_SPEC.to_owned(),
            windows: Vec::new(),
            foreground: None,
            displays: Vec::new(),
            power: None,
            idle_ms: 0,
            meeting: MeetingState {
                active: false,
                hint: MeetingHint::None,
            },
            cursor: None,
            observed_at_ms: observed,
            expires_at_ms: expires,
            freshness,
            degradation_reason: if freshness == Freshness::Degraded {
                Some(DegradationReason::Timeout)
            } else {
                None
            },
        }
    }

    #[test]
    fn usable_only_when_fresh_and_unexpired() {
        let fresh = snap(Freshness::Fresh, 1_000, 6_000);
        assert!(is_usable(&fresh, 5_999));
        assert!(!is_usable(&fresh, 6_000));
        assert!(!is_usable(&fresh, 6_001));

        let stale = snap(Freshness::Stale, 1_000, 6_000);
        assert!(!is_usable(&stale, 1_001));
        assert!(!obstacles_usable(&stale, 1_001));

        let degraded = snap(Freshness::Degraded, 1_000, 6_000);
        assert!(!is_usable(&degraded, 1_001));
        assert!(!obstacles_usable(&degraded, 1_001));
    }

    #[test]
    fn refresh_marks_expired_as_stale() {
        let fresh = snap(Freshness::Fresh, 1_000, 6_000);
        assert_eq!(refresh_freshness(&fresh, 5_000), Freshness::Fresh);
        assert_eq!(refresh_freshness(&fresh, 6_000), Freshness::Stale);
        let degraded = snap(Freshness::Degraded, 1_000, 6_000);
        assert_eq!(refresh_freshness(&degraded, 6_000), Freshness::Degraded);
    }
}
