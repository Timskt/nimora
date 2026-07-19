//! Minimal audited Windows system-context FFI boundary.

use std::time::Duration;

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
    },
};

/// Samples whether the visible foreground window covers its current monitor.
///
/// # Errors
///
/// Returns a stable adapter error for invalid input or unavailable Win32 facts.
pub fn sample_fullscreen(timeout: Duration) -> Result<bool, WindowsFullscreenError> {
    if timeout.is_zero() {
        return Err(WindowsFullscreenError::InvalidTimeout);
    }
    sample_fullscreen_platform()
}

#[cfg(target_os = "windows")]
fn sample_fullscreen_platform() -> Result<bool, WindowsFullscreenError> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null()
        || unsafe { IsWindowVisible(window) } == 0
        || unsafe { IsIconic(window) } != 0
    {
        return Ok(false);
    }
    if is_shell_surface(window)? {
        return Ok(false);
    }
    let mut window_rect = RECT::default();
    if unsafe { GetWindowRect(window, &raw mut window_rect) } == 0 {
        return Err(WindowsFullscreenError::WindowBoundsUnavailable);
    }
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return Err(WindowsFullscreenError::MonitorUnavailable);
    }
    let mut monitor_info = MONITORINFO {
        cbSize: u32::try_from(std::mem::size_of::<MONITORINFO>()).unwrap_or(u32::MAX),
        ..MONITORINFO::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &raw mut monitor_info) } == 0 {
        return Err(WindowsFullscreenError::MonitorBoundsUnavailable);
    }
    Ok(rect_covers_monitor(
        window_rect.into(),
        monitor_info.rcMonitor.into(),
        2,
    ))
}

#[cfg(not(target_os = "windows"))]
fn sample_fullscreen_platform() -> Result<bool, WindowsFullscreenError> {
    Err(WindowsFullscreenError::AdapterUnavailable)
}

#[cfg(target_os = "windows")]
fn is_shell_surface(window: HWND) -> Result<bool, WindowsFullscreenError> {
    let mut class_name = [0_u16; 64];
    let capacity = i32::try_from(class_name.len())
        .map_err(|_| WindowsFullscreenError::WindowClassUnavailable)?;
    let length = unsafe { GetClassNameW(window, class_name.as_mut_ptr(), capacity) };
    if length == 0 {
        return Err(WindowsFullscreenError::WindowClassUnavailable);
    }
    let length =
        usize::try_from(length).map_err(|_| WindowsFullscreenError::WindowClassUnavailable)?;
    let class_name = String::from_utf16_lossy(&class_name[..length]);
    Ok(matches!(
        class_name.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd"
    ))
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
struct RectBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
impl From<RECT> for RectBounds {
    fn from(value: RECT) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

#[cfg(any(test, target_os = "windows"))]
fn rect_covers_monitor(window: RectBounds, monitor: RectBounds, tolerance: i32) -> bool {
    window.left <= monitor.left.saturating_add(tolerance)
        && window.top <= monitor.top.saturating_add(tolerance)
        && window.right >= monitor.right.saturating_sub(tolerance)
        && window.bottom >= monitor.bottom.saturating_sub(tolerance)
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFullscreenError {
    #[error("Windows fullscreen sensor timeout must be non-zero")]
    InvalidTimeout,
    #[error("Windows fullscreen sensor adapter is unavailable")]
    AdapterUnavailable,
    #[error("Windows foreground window bounds are unavailable")]
    WindowBoundsUnavailable,
    #[error("Windows foreground window class is unavailable")]
    WindowClassUnavailable,
    #[error("Windows foreground monitor is unavailable")]
    MonitorUnavailable,
    #[error("Windows foreground monitor bounds are unavailable")]
    MonitorBoundsUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_coverage_tolerates_native_frame_rounding_only() {
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
    }

    #[test]
    fn zero_timeout_fails_before_platform_access() {
        assert_eq!(
            sample_fullscreen(Duration::ZERO),
            Err(WindowsFullscreenError::InvalidTimeout)
        );
    }
}
