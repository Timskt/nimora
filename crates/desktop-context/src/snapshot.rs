//! Versioned desktop observation types.

use crate::freshness::MAX_SNAPSHOT_LIFETIME_MS;
use crate::DesktopContextError;
use serde::{Deserialize, Serialize};

/// Wire/spec identifier for desktop context snapshots.
pub const DESKTOP_CONTEXT_SPEC: &str = "nimora.desktop-context/1";

/// How trustworthy a snapshot is for motion decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Sample is recent and complete enough for full motion planning.
    Fresh,
    /// Sample is past its soft expiry; treat as free-wander only.
    Stale,
    /// Sample is partial or degraded; do not use obstacle geometry.
    Degraded,
}

/// Why a snapshot was marked degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradationReason {
    Timeout,
    PermissionDenied,
    PlatformUnavailable,
    PartialSample,
    ClockSkew,
}

/// Best-effort meeting application hint (privacy-preserving; no titles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingHint {
    Zoom,
    Teams,
    Meet,
    Webex,
    Unknown,
    #[default]
    None,
}

/// Axis-aligned integer rectangle in physical desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Observed OS window, with optional hashed title only (never raw user titles).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopWindow {
    pub id: String,
    /// Privacy-preserving hash of the window title when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_hash: Option<String>,
    /// Host set this when a raw title was redacted before persistence.
    #[serde(default)]
    pub title_redacted: bool,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub z_order: i32,
    pub owner_pid: u32,
    #[serde(default)]
    pub owner_name: String,
    pub onscreen: bool,
    pub is_minimized: bool,
    pub is_fullscreen_candidate: bool,
    /// Shell/chrome windows (taskbar, dock) should be excluded from obstacles.
    #[serde(default)]
    pub is_shell: bool,
}

impl DesktopWindow {
    /// Returns the window bounds as a [`WorkArea`].
    #[must_use]
    pub const fn bounds(&self) -> WorkArea {
        WorkArea {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Foreground application summary without window titles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ForegroundApp {
    pub app_name: String,
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
}

/// Physical display geometry and work area.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopDisplay {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub work_area: WorkArea,
    pub scale_factor: f64,
}

/// Battery / AC power summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerState {
    pub on_battery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    pub charging: bool,
}

/// Meeting activity hint derived from process/app signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingState {
    pub active: bool,
    pub hint: MeetingHint,
}

/// Optional cursor sample in physical coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

/// Construction inputs for a versioned [`DesktopSnapshot`].
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopSnapshotParts {
    pub windows: Vec<DesktopWindow>,
    pub foreground: Option<ForegroundApp>,
    pub displays: Vec<DesktopDisplay>,
    pub power: Option<PowerState>,
    pub idle_ms: u64,
    pub meeting: MeetingState,
    pub cursor: Option<CursorPosition>,
    pub observed_at_ms: u64,
    pub expires_at_ms: u64,
    pub freshness: Freshness,
    pub degradation_reason: Option<DegradationReason>,
}

/// Versioned desktop lifeform context snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopSnapshot {
    pub spec: String,
    #[serde(default)]
    pub windows: Vec<DesktopWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<ForegroundApp>,
    #[serde(default)]
    pub displays: Vec<DesktopDisplay>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<PowerState>,
    pub idle_ms: u64,
    pub meeting: MeetingState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorPosition>,
    pub observed_at_ms: u64,
    pub expires_at_ms: u64,
    pub freshness: Freshness,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradation_reason: Option<DegradationReason>,
}

impl DesktopSnapshot {
    /// Builds a validated snapshot with the current wire spec.
    ///
    /// # Errors
    ///
    /// Returns an error when the lifetime is zero, inverted, or longer than the policy cap.
    pub fn new(parts: DesktopSnapshotParts) -> Result<Self, DesktopContextError> {
        let lifetime = parts
            .expires_at_ms
            .checked_sub(parts.observed_at_ms)
            .ok_or(DesktopContextError::InvalidLifetime)?;
        if lifetime == 0 || lifetime > MAX_SNAPSHOT_LIFETIME_MS {
            return Err(DesktopContextError::InvalidLifetime);
        }
        Ok(Self {
            spec: DESKTOP_CONTEXT_SPEC.to_owned(),
            windows: parts.windows,
            foreground: parts.foreground,
            displays: parts.displays,
            power: parts.power,
            idle_ms: parts.idle_ms,
            meeting: parts.meeting,
            cursor: parts.cursor,
            observed_at_ms: parts.observed_at_ms,
            expires_at_ms: parts.expires_at_ms,
            freshness: parts.freshness,
            degradation_reason: parts.degradation_reason,
        })
    }

    /// Validates wire fields after deserialization.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown specs or inverted/oversized lifetimes.
    pub fn validate(&self) -> Result<(), DesktopContextError> {
        if self.spec != DESKTOP_CONTEXT_SPEC {
            return Err(DesktopContextError::InvalidSpec);
        }
        let lifetime = self
            .expires_at_ms
            .checked_sub(self.observed_at_ms)
            .ok_or(DesktopContextError::InvalidLifetime)?;
        if lifetime == 0 || lifetime > MAX_SNAPSHOT_LIFETIME_MS {
            return Err(DesktopContextError::InvalidLifetime);
        }
        Ok(())
    }

    /// Obstacle rects: onscreen, non-minimized, non-shell windows.
    #[must_use]
    pub fn obstacle_windows(&self) -> Vec<WorkArea> {
        self.windows
            .iter()
            .filter(|window| window.onscreen && !window.is_minimized && !window.is_shell)
            .map(DesktopWindow::bounds)
            .collect()
    }

    /// Primary display (adapters place the primary first).
    #[must_use]
    pub fn primary_display(&self) -> Option<&DesktopDisplay> {
        crate::displays::primary_display(&self.displays)
    }

    /// Work areas for all known displays.
    #[must_use]
    pub fn work_areas(&self) -> Vec<WorkArea> {
        crate::displays::work_areas(&self.displays)
    }

    /// Display containing a physical point, if any.
    #[must_use]
    pub fn display_containing_point(&self, x: f64, y: f64) -> Option<&DesktopDisplay> {
        crate::displays::display_containing_point(&self.displays, x, y)
    }

    /// Work area for the display under `(x, y)`, else primary work area.
    #[must_use]
    pub fn work_area_for_point(&self, x: f64, y: f64) -> Option<WorkArea> {
        crate::displays::work_area_for_point(&self.displays, x, y)
    }

    /// Union of full multi-monitor bounds when displays are present.
    #[must_use]
    pub fn union_display_bounds(&self) -> Option<WorkArea> {
        crate::displays::union_display_bounds(&self.displays)
    }

    /// Occluder rects for a pet owned by `pet_owner_pid` (shell/own/min filtered).
    #[must_use]
    pub fn occluders_for_pet(&self, pet_owner_pid: u32) -> Vec<crate::OccluderRect> {
        crate::occluders_from_snapshot(self, pet_owner_pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_parts(freshness: Freshness) -> DesktopSnapshotParts {
        DesktopSnapshotParts {
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
            observed_at_ms: 1_000,
            expires_at_ms: 6_000,
            freshness,
            degradation_reason: if freshness == Freshness::Degraded {
                Some(DegradationReason::PartialSample)
            } else {
                None
            },
        }
    }

    #[test]
    fn rejects_invalid_lifetime() {
        let mut parts = minimal_parts(Freshness::Fresh);
        parts.expires_at_ms = parts.observed_at_ms;
        assert_eq!(
            DesktopSnapshot::new(parts),
            Err(DesktopContextError::InvalidLifetime)
        );

        let mut parts = minimal_parts(Freshness::Fresh);
        parts.expires_at_ms = parts.observed_at_ms + MAX_SNAPSHOT_LIFETIME_MS + 1;
        assert_eq!(
            DesktopSnapshot::new(parts),
            Err(DesktopContextError::InvalidLifetime)
        );
    }

    #[test]
    fn serde_roundtrip_omits_raw_titles() {
        let snapshot = DesktopSnapshot::new(minimal_parts(Freshness::Fresh)).expect("valid");
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(!json.contains("\"title\":"));
        assert!(json.contains("nimora.desktop-context/1"));
        let parsed: DesktopSnapshot = serde_json::from_str(&json).expect("deserialize");
        parsed.validate().expect("valid");
        assert_eq!(parsed.spec, DESKTOP_CONTEXT_SPEC);
    }

    #[test]
    fn window_stores_hash_not_raw_title_field() {
        let window = DesktopWindow {
            id: "w1".into(),
            title_hash: Some("abc123".into()),
            title_redacted: true,
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            z_order: 1,
            owner_pid: 42,
            owner_name: "App".into(),
            onscreen: true,
            is_minimized: false,
            is_fullscreen_candidate: false,
            is_shell: false,
        };
        let json = serde_json::to_string(&window).expect("serialize");
        assert!(json.contains("titleHash"));
        assert!(json.contains("titleRedacted"));
        assert!(!json.contains("\"title\":"));
    }

    #[test]
    fn multi_monitor_helpers_on_snapshot() {
        let mut parts = minimal_parts(Freshness::Fresh);
        parts.displays = vec![
            DesktopDisplay {
                id: "a".into(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                work_area: WorkArea {
                    x: 0,
                    y: 25,
                    width: 1920,
                    height: 1055,
                },
                scale_factor: 2.0,
            },
            DesktopDisplay {
                id: "b".into(),
                x: 1920,
                y: 0,
                width: 1280,
                height: 800,
                work_area: WorkArea {
                    x: 1920,
                    y: 0,
                    width: 1280,
                    height: 800,
                },
                scale_factor: 1.0,
            },
        ];
        parts.windows = vec![DesktopWindow {
            id: "w".into(),
            title_hash: None,
            title_redacted: true,
            x: 10,
            y: 40,
            width: 400,
            height: 300,
            z_order: 1,
            owner_pid: 9,
            owner_name: "App".into(),
            onscreen: true,
            is_minimized: false,
            is_fullscreen_candidate: false,
            is_shell: false,
        }];
        let snapshot = DesktopSnapshot::new(parts).expect("valid");
        assert_eq!(snapshot.primary_display().map(|d| d.id.as_str()), Some("a"));
        assert_eq!(
            snapshot.display_containing_point(2000.0, 10.0).map(|d| d.id.as_str()),
            Some("b")
        );
        assert_eq!(snapshot.work_areas().len(), 2);
        let union = snapshot.union_display_bounds().expect("union");
        assert_eq!(union.width, 3200);
        assert_eq!(snapshot.obstacle_windows().len(), 1);
        assert_eq!(snapshot.occluders_for_pet(1).len(), 1);
        assert!(snapshot.occluders_for_pet(9).is_empty());
    }

    #[test]
    fn meeting_and_power_roundtrip() {
        let mut parts = minimal_parts(Freshness::Fresh);
        parts.power = Some(PowerState {
            on_battery: true,
            battery_percent: Some(42),
            charging: false,
        });
        parts.meeting = MeetingState {
            active: true,
            hint: MeetingHint::Zoom,
        };
        parts.idle_ms = 12_000;
        let snapshot = DesktopSnapshot::new(parts).expect("valid");
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: DesktopSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.idle_ms, 12_000);
        assert_eq!(parsed.meeting.hint, MeetingHint::Zoom);
        assert_eq!(
            parsed.power.map(|p| p.battery_percent),
            Some(Some(42))
        );
    }
}
