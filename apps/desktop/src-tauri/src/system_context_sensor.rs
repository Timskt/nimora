#[cfg(target_os = "macos")]
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(target_os = "macos")]
use wait_timeout::ChildExt;

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, IsIconic, IsWindowVisible,
    },
};

#[cfg(target_os = "macos")]
const FULLSCREEN_QUERY: &str = r#"
tell application "System Events"
  set frontProcess to first application process whose frontmost is true
  if (count of windows of frontProcess) is 0 then return "false"
  try
    return value of attribute "AXFullScreen" of front window of frontProcess
  on error
    return "false"
  end try
end tell
"#;

#[cfg(target_os = "macos")]
pub fn sample_fullscreen(timeout: Duration) -> Result<bool, MacOsFullscreenError> {
    if timeout.is_zero() {
        return Err(MacOsFullscreenError::InvalidTimeout);
    }
    let mut child = Command::new("/usr/bin/osascript")
        .args(["-e", FULLSCREEN_QUERY])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| MacOsFullscreenError::AdapterUnavailable)?;
    let Some(status) = child
        .wait_timeout(timeout)
        .map_err(|_| MacOsFullscreenError::AdapterUnavailable)?
    else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(MacOsFullscreenError::SampleTimeout);
    };
    if !status.success() {
        return Err(MacOsFullscreenError::PermissionDeniedOrUnavailable);
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or(MacOsFullscreenError::InvalidResponse)?
        .take(16)
        .read_to_string(&mut output)
        .map_err(|_| MacOsFullscreenError::InvalidResponse)?;
    match output.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(MacOsFullscreenError::InvalidResponse),
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum MacOsFullscreenError {
    #[error("macOS fullscreen sensor timeout must be non-zero")]
    InvalidTimeout,
    #[error("macOS fullscreen sensor adapter is unavailable")]
    AdapterUnavailable,
    #[error("macOS fullscreen sensor timed out")]
    SampleTimeout,
    #[error("macOS fullscreen sensor lacks permission or system support")]
    PermissionDeniedOrUnavailable,
    #[error("macOS fullscreen sensor returned an invalid response")]
    InvalidResponse,
}

#[cfg(target_os = "windows")]
pub fn sample_fullscreen(_timeout: std::time::Duration) -> Result<bool, WindowsFullscreenError> {
    if _timeout.is_zero() {
        return Err(WindowsFullscreenError::InvalidTimeout);
    }
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Ok(false);
    }
    if unsafe { IsWindowVisible(window) } == 0 || unsafe { IsIconic(window) } != 0 {
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
        Rect::from(window_rect),
        Rect::from(monitor_info.rcMonitor),
        2,
    ))
}

#[cfg(target_os = "windows")]
fn is_shell_surface(window: HWND) -> Result<bool, WindowsFullscreenError> {
    let mut class_name = [0_u16; 64];
    let length = unsafe { GetClassNameW(window, class_name.as_mut_ptr(), class_name.len() as i32) };
    if length == 0 {
        return Err(WindowsFullscreenError::WindowClassUnavailable);
    }
    let class_name = String::from_utf16_lossy(&class_name[..length as usize]);
    Ok(matches!(
        class_name.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd"
    ))
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
impl From<RECT> for Rect {
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
fn rect_covers_monitor(window: Rect, monitor: Rect, tolerance: i32) -> bool {
    window.left <= monitor.left.saturating_add(tolerance)
        && window.top <= monitor.top.saturating_add(tolerance)
        && window.right >= monitor.right.saturating_sub(tolerance)
        && window.bottom >= monitor.bottom.saturating_sub(tolerance)
}

#[cfg(target_os = "windows")]
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum WindowsFullscreenError {
    #[error("Windows fullscreen sensor timeout must be non-zero")]
    InvalidTimeout,
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
    #[cfg(target_os = "macos")]
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_zero_timeout_without_spawning_adapter() {
        assert_eq!(
            sample_fullscreen(Duration::ZERO),
            Err(MacOsFullscreenError::InvalidTimeout)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn query_reads_only_the_accessibility_fullscreen_attribute() {
        assert!(FULLSCREEN_QUERY.contains("AXFullScreen"));
        for forbidden in ["AXTitle", "name of", "description of", "contents"] {
            assert!(!FULLSCREEN_QUERY.contains(forbidden));
        }
    }

    #[test]
    fn monitor_coverage_tolerates_native_frame_rounding_only() {
        let monitor = Rect {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert!(rect_covers_monitor(
            Rect {
                left: -1919,
                top: 1,
                right: -1,
                bottom: 1079,
            },
            monitor,
            2,
        ));
        assert!(!rect_covers_monitor(
            Rect {
                left: -1900,
                top: 0,
                right: 0,
                bottom: 1080,
            },
            monitor,
            2,
        ));
    }
}
