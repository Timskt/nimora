//! macOS desktop environment sampler for Nimora desktop-context.
//!
//! Samples window geometry, foreground app, idle time, power, and meeting
//! process hints. Privacy: window titles are never stored (empty by default).
//!
//! # Host wiring
//!
//! ```ignore
//! use nimora_desktop_context_macos::{sample, EnvironmentSample};
//! use std::time::Duration;
//!
//! // In the desktop host sensor loop (every ~5s, timeout ~2s):
//! match sample(Duration::from_secs(2)) {
//!     Ok(env) => {
//!         // Map EnvironmentSample → DesktopSnapshot in the parent host:
//!         // windows → DesktopWindow { title_hash: None, title_redacted: true, ... }
//!         // displays → DesktopDisplay (multi-monitor work areas)
//!         // foreground → ForegroundApp
//!         // power → Option<PowerState> (None when !power.available)
//!         // meeting → MeetingState
//!         // observed_at_ms + lease → expires_at_ms
//!         let _ = env;
//!     }
//!     Err(error) => {
//!         // Fail closed: mark sensor degraded / let lease expire.
//!         let _ = error;
//!     }
//! }
//! ```
//!
//! Workspace member: parent hosts depend via path or workspace packaging and
//! map [`EnvironmentSample`] into `nimora_desktop_context::DesktopSnapshot`.

mod filter;
mod meeting;
mod rect;
mod types;

#[cfg(target_os = "macos")]
mod macos;

pub use filter::{
    filter_windows, is_shell_layer, is_shell_owner_name, is_zero_size, should_drop_window,
    SHELL_LAYER_MIN,
};
pub use meeting::{meeting_from_process_names, meeting_hint_from_process_name, prefer_meeting_hint};
pub use rect::{rect_covers_monitor, RectBounds};
pub use types::{
    DisplayFact, EnvironmentSample, ForegroundFact, MeetingFact, MeetingHint, PowerFact,
    WindowFact, WorkAreaFact,
};

use std::time::{Duration, Instant};

/// Errors produced by the macOS desktop-context adapter.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum DesktopContextMacosError {
    #[error("macOS desktop-context sample timeout must be non-zero")]
    InvalidTimeout,
    #[error("macOS desktop-context adapter is unavailable")]
    AdapterUnavailable,
    #[error("macOS desktop-context sample timed out")]
    SampleTimeout,
    #[error("macOS desktop-context lacks permission for window list")]
    PermissionDenied,
    #[error("macOS desktop-context window list is unavailable")]
    WindowListUnavailable,
    #[error("macOS desktop-context query failed")]
    QueryFailed,
}

/// Samples the desktop environment with a hard wall-clock timeout.
///
/// On non-macOS targets this always returns [`DesktopContextMacosError::AdapterUnavailable`].
/// Zero timeouts fail closed before any platform access.
///
/// # Errors
///
/// Returns a stable adapter error for invalid input, timeout, permission denial,
/// or platform failure. Never panics on missing permissions.
pub fn sample(timeout: Duration) -> Result<EnvironmentSample, DesktopContextMacosError> {
    if timeout.is_zero() {
        return Err(DesktopContextMacosError::InvalidTimeout);
    }
    sample_impl(timeout)
}

#[cfg(target_os = "macos")]
fn sample_impl(timeout: Duration) -> Result<EnvironmentSample, DesktopContextMacosError> {
    let deadline = Instant::now() + timeout;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    // Isolated worker so a stuck CoreGraphics/AppKit call cannot block the host
    // forever; join is bounded by `timeout`.
    std::thread::Builder::new()
        .name("nimora-desktop-context-macos".into())
        .spawn(move || {
            let result = macos::sample_platform(timeout, deadline);
            let _ = sender.send(result);
        })
        .map_err(|_| DesktopContextMacosError::QueryFailed)?;

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(DesktopContextMacosError::SampleTimeout)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(DesktopContextMacosError::QueryFailed)
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn sample_impl(_timeout: Duration) -> Result<EnvironmentSample, DesktopContextMacosError> {
    let _ = Instant::now();
    Err(DesktopContextMacosError::AdapterUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WindowFact;

    fn window(
        owner_pid: u32,
        owner_name: &str,
        layer: i32,
        width: u32,
        height: u32,
    ) -> WindowFact {
        WindowFact {
            id: "1".into(),
            title: "SECRET TITLE".into(),
            x: 0,
            y: 0,
            width,
            height,
            layer,
            z_order: 0,
            owner_pid,
            owner_name: owner_name.into(),
            onscreen: true,
            is_minimized: false,
            is_shell: layer >= SHELL_LAYER_MIN,
        }
    }

    #[test]
    fn zero_timeout_fails_before_platform_access() {
        assert_eq!(
            sample(Duration::ZERO),
            Err(DesktopContextMacosError::InvalidTimeout)
        );
    }

    #[test]
    fn filter_drops_own_pid_shell_and_zero_size() {
        let own = 4242_u32;
        let input = vec![
            window(own, "Nimora", 0, 100, 100),
            window(7, "Dock", 20, 100, 40),
            window(8, "Safari", 0, 0, 0),
            window(9, "Notes", 0, 200, 150),
            window(10, "SystemUIServer", 25, 50, 20),
            window(11, "Code", 0, 800, 600),
        ];
        let filtered = filter_windows(input, Some(own));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].owner_name, "Notes");
        assert_eq!(filtered[1].owner_name, "Code");
        // Privacy: titles forced empty.
        assert!(filtered.iter().all(|window| window.title.is_empty()));
    }

    #[test]
    fn shell_owner_and_layer_heuristics() {
        assert!(is_shell_layer(20));
        assert!(is_shell_layer(25));
        assert!(!is_shell_layer(0));
        assert!(!is_shell_layer(3));
        assert!(is_shell_owner_name("Dock"));
        assert!(is_shell_owner_name("Control Center"));
        assert!(!is_shell_owner_name("Safari"));
        assert!(is_zero_size(0, 10));
        assert!(is_zero_size(10, 0));
        assert!(!is_zero_size(1, 1));
    }

    #[test]
    fn meeting_process_name_match() {
        assert_eq!(
            meeting_hint_from_process_name("zoom.us"),
            MeetingHint::Zoom
        );
        assert_eq!(
            meeting_hint_from_process_name("Microsoft Teams"),
            MeetingHint::Teams
        );
        assert_eq!(
            meeting_hint_from_process_name("Google Meet"),
            MeetingHint::Meet
        );
        assert_eq!(
            meeting_hint_from_process_name("Cisco Meeting Center"),
            MeetingHint::Webex
        );
        assert_eq!(
            meeting_hint_from_process_name("webex"),
            MeetingHint::Webex
        );
        assert_eq!(
            meeting_hint_from_process_name("Safari"),
            MeetingHint::None
        );
        // Bare "meet" alone must not false-positive.
        assert_eq!(
            meeting_hint_from_process_name("Meetings"),
            MeetingHint::None
        );

        let fact = meeting_from_process_names(["Finder", "zoom.us", "Notes"]);
        assert!(fact.active);
        assert_eq!(fact.hint, MeetingHint::Zoom);

        let inactive = meeting_from_process_names(["Finder", "Safari"]);
        assert!(!inactive.active);
        assert_eq!(inactive.hint, MeetingHint::None);
    }

    #[test]
    fn filter_sorts_by_z_and_caps_and_drops_minimized() {
        let mut windows = Vec::new();
        for i in 0..70 {
            windows.push(WindowFact {
                id: format!("{i}"),
                title: "SECRET".into(),
                x: 0,
                y: 0,
                width: 100,
                height: 100,
                layer: 0,
                z_order: 70 - i, // reverse so sort is exercised
                owner_pid: 100 + i as u32,
                owner_name: format!("App{i}"),
                onscreen: true,
                is_minimized: i == 0,
                is_shell: false,
            });
        }
        let filtered = filter_windows(windows, None);
        // one minimized dropped; cap at 64 of remaining 69
        assert_eq!(filtered.len(), 64);
        assert!(filtered.windows(2).all(|w| w[0].z_order <= w[1].z_order));
        assert!(filtered.iter().all(|w| w.title.is_empty()));
        assert!(!filtered.iter().any(|w| w.is_minimized));
    }

    #[test]
    fn facetime_is_unknown_meeting_not_false_positive_meetings() {
        assert_eq!(
            meeting_hint_from_process_name("FaceTime"),
            MeetingHint::Unknown
        );
        let fact = meeting_from_process_names(["FaceTime"]);
        assert!(fact.active);
        assert_eq!(fact.hint, MeetingHint::Unknown);
    }

    #[test]
    fn rect_covering_tolerates_frame_rounding() {
        let monitor = RectBounds {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert!(rect_covers_monitor(
            RectBounds {
                left: -1919,
                top: 1,
                right: -1,
                bottom: 1079
            },
            monitor,
            2
        ));
        assert!(!rect_covers_monitor(
            RectBounds {
                left: -1900,
                top: 0,
                right: 0,
                bottom: 1080
            },
            monitor,
            2
        ));
        let from_size = RectBounds::from_origin_size(10, 20, 100, 50);
        assert_eq!(from_size.right, 110);
        assert_eq!(from_size.bottom, 70);
    }

    #[test]
    fn power_unavailable_is_fail_closed() {
        let power = PowerFact::unavailable();
        assert!(!power.available);
        assert!(power.battery_percent.is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_sample_succeeds_or_fails_closed() {
        // Platform test: must never panic; permission denial is acceptable.
        match sample(Duration::from_secs(3)) {
            Ok(env) => {
                assert!(env.observed_at_ms > 0);
                // Titles must never leak.
                assert!(env.windows.iter().all(|window| window.title.is_empty()));
                // Own process windows filtered.
                let own = std::process::id();
                assert!(env.windows.iter().all(|window| window.owner_pid != own));
                // Multi-monitor list is best-effort; when present primary is first.
                if let Some(primary) = env.displays.first() {
                    assert!(primary.width > 0 && primary.height > 0);
                    assert!(primary.work_area.width > 0 && primary.work_area.height > 0);
                    if env.displays.iter().any(|d| d.is_primary) {
                        assert!(primary.is_primary);
                    }
                }
                if let Some(fg) = &env.foreground {
                    assert!(fg.pid > 0);
                }
            }
            Err(error) => {
                assert!(matches!(
                    error,
                    DesktopContextMacosError::PermissionDenied
                        | DesktopContextMacosError::WindowListUnavailable
                        | DesktopContextMacosError::SampleTimeout
                        | DesktopContextMacosError::QueryFailed
                ));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_returns_adapter_unavailable() {
        assert_eq!(
            sample(Duration::from_secs(1)),
            Err(DesktopContextMacosError::AdapterUnavailable)
        );
    }
}
