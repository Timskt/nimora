//! Local desktop environment sample DTOs.
//!
//! These are platform-adapter facts. The parent host maps them into
//! `nimora-desktop-context` snapshot types (not a dependency of this crate).

use serde::{Deserialize, Serialize};

/// One complete desktop environment sample from the macOS adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSample {
    pub windows: Vec<WindowFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<ForegroundFact>,
    /// Physical displays with work areas (primary first when known).
    #[serde(default)]
    pub displays: Vec<DisplayFact>,
    pub idle_ms: u64,
    pub power: PowerFact,
    pub meeting: MeetingFact,
    /// Wall-clock observation time in milliseconds since Unix epoch.
    /// Hosts may override with their own clock before building a snapshot.
    pub observed_at_ms: u64,
}

/// One physical display with work area (menu bar / dock excluded when known).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayFact {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub work_area: WorkAreaFact,
    pub scale_factor: f64,
    /// True when this is the main display.
    #[serde(default)]
    pub is_primary: bool,
}

/// Integer work-area rectangle in physical desktop coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkAreaFact {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Observed OS window fact (privacy-preserving; titles omitted by default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFact {
    pub id: String,
    /// Always empty unless a future optional title-hash feature is enabled.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// CGWindow layer (0 = normal).
    pub layer: i32,
    /// Front-to-back order index (0 = frontmost among collected windows).
    pub z_order: i32,
    pub owner_pid: u32,
    #[serde(default)]
    pub owner_name: String,
    pub onscreen: bool,
    pub is_minimized: bool,
    /// Dock / menu bar / Window Server chrome (should not be obstacles).
    #[serde(default)]
    pub is_shell: bool,
}

impl WindowFact {
    /// Axis-aligned bounds as `(x, y, width, height)`.
    #[must_use]
    pub const fn bounds(&self) -> (i32, i32, u32, u32) {
        (self.x, self.y, self.width, self.height)
    }
}

/// Foreground application summary without window titles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundFact {
    pub app_name: String,
    pub pid: u32,
    /// True when derived from a weaker process-list fallback.
    #[serde(default)]
    pub degraded: bool,
}

/// Battery / AC power summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerFact {
    /// When false, remaining fields are unspecified placeholders.
    pub available: bool,
    pub on_battery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    pub charging: bool,
}

impl PowerFact {
    /// Unavailable / degraded power sample (fail-closed default).
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            available: false,
            on_battery: false,
            battery_percent: None,
            charging: false,
        }
    }
}

/// Best-effort meeting application hint (no titles or content).
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

/// Meeting activity fact derived from process/app name signals only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingFact {
    pub active: bool,
    pub hint: MeetingHint,
}

impl MeetingFact {
    #[must_use]
    pub const fn inactive() -> Self {
        Self {
            active: false,
            hint: MeetingHint::None,
        }
    }
}
