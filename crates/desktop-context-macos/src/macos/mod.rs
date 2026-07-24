//! macOS platform sampling: CoreGraphics windows, NSWorkspace foreground,
//! multi-monitor displays, idle, power, meeting.

mod cf;
mod displays;
mod foreground;
mod idle;
mod power;
mod windows;

use crate::filter::filter_windows;
use crate::meeting::meeting_from_process_names;
use crate::types::{EnvironmentSample, ForegroundFact, MeetingFact, WindowFact};
use crate::DesktopContextMacosError;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Collect a full environment sample on macOS.
///
/// # Errors
///
/// Returns a structured error when the window list is unavailable or sampling fails.
pub(crate) fn sample_platform(
    timeout: Duration,
    deadline: Instant,
) -> Result<EnvironmentSample, DesktopContextMacosError> {
    ensure_deadline(deadline)?;

    let own_pid = std::process::id();
    let raw_windows = windows::list_windows().map_err(|error| match error {
        windows::WindowListError::Unavailable => DesktopContextMacosError::WindowListUnavailable,
        windows::WindowListError::PermissionDenied => DesktopContextMacosError::PermissionDenied,
    })?;
    ensure_deadline(deadline)?;

    let windows = filter_windows(raw_windows, Some(own_pid));
    let foreground = sample_foreground(&windows);
    ensure_deadline(deadline)?;

    let displays = displays::list_displays();
    ensure_deadline(deadline)?;

    let idle_ms = idle::idle_ms().unwrap_or(0);
    ensure_deadline(deadline)?;

    let power = power::sample_power();
    let meeting = sample_meeting(&windows, foreground.as_ref());
    ensure_deadline(deadline)?;

    let _ = timeout;

    Ok(EnvironmentSample {
        windows,
        foreground,
        displays,
        idle_ms,
        power,
        meeting,
        observed_at_ms: now_ms(),
    })
}

fn ensure_deadline(deadline: Instant) -> Result<(), DesktopContextMacosError> {
    if Instant::now() >= deadline {
        Err(DesktopContextMacosError::SampleTimeout)
    } else {
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Prefer NSWorkspace.frontmostApplication; fall back to CG frontmost window owner.
fn sample_foreground(windows: &[WindowFact]) -> Option<ForegroundFact> {
    if let Some(fg) = foreground::frontmost_application() {
        return Some(fg);
    }
    windows
        .iter()
        .find(|window| window.onscreen && !window.is_minimized)
        .map(|window| ForegroundFact {
            app_name: window.owner_name.clone(),
            pid: window.owner_pid,
            degraded: true,
        })
}

/// Meeting detection from window owner names and optional foreground app name.
fn sample_meeting(windows: &[WindowFact], foreground: Option<&ForegroundFact>) -> MeetingFact {
    let mut names: Vec<&str> = windows
        .iter()
        .map(|window| window.owner_name.as_str())
        .collect();
    if let Some(fg) = foreground {
        names.push(fg.app_name.as_str());
    }
    meeting_from_process_names(names)
}
