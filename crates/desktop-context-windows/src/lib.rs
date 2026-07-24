//! Windows desktop environment sampling for Nimora lifeform context.
//!
//! Mirrors the macOS adapter surface with privacy-preserving window facts.
//! Titles are never retained. Multi-monitor work areas are sampled when
//! `cfg(windows)`; non-Windows targets return clean adapter-unavailable stubs.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// One complete desktop environment sample.
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
    pub observed_at_ms: u64,
}

/// One physical display with work area (taskbar excluded when known).
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFact {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub layer: i32,
    pub z_order: i32,
    pub owner_pid: u32,
    #[serde(default)]
    pub owner_name: String,
    pub onscreen: bool,
    pub is_minimized: bool,
    #[serde(default)]
    pub is_shell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundFact {
    pub app_name: String,
    pub pid: u32,
    #[serde(default)]
    pub degraded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerFact {
    pub available: bool,
    pub on_battery: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    pub charging: bool,
}

impl PowerFact {
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

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DesktopContextWindowsError {
    #[error("Windows desktop-context sample timeout must be non-zero")]
    InvalidTimeout,
    #[error("Windows desktop-context adapter is unavailable")]
    AdapterUnavailable,
    #[error("Windows desktop-context sample timed out")]
    SampleTimeout,
    #[error("Windows desktop-context query failed")]
    QueryFailed,
}

/// Samples desktop environment facts with a hard wall-clock budget.
///
/// # Errors
///
/// Returns a stable adapter error for invalid timeouts or platform failure.
pub fn sample(timeout: Duration) -> Result<EnvironmentSample, DesktopContextWindowsError> {
    if timeout.is_zero() {
        return Err(DesktopContextWindowsError::InvalidTimeout);
    }
    sample_with_budget(timeout)
}

/// Hard wall-clock budget around the platform sample (parity with macOS adapter).
///
/// On Windows, enumeration can hang under hung message pumps; the worker is
/// bounded so lifeform hosts fail closed instead of blocking unattended loops.
#[cfg(target_os = "windows")]
fn sample_with_budget(timeout: Duration) -> Result<EnvironmentSample, DesktopContextWindowsError> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("nimora-desktop-context-windows".into())
        .spawn(move || {
            let result = sample_platform();
            let _ = sender.send(result);
        })
        .map_err(|_| DesktopContextWindowsError::QueryFailed)?;

    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(DesktopContextWindowsError::SampleTimeout)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(DesktopContextWindowsError::QueryFailed)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn sample_with_budget(_timeout: Duration) -> Result<EnvironmentSample, DesktopContextWindowsError> {
    Err(DesktopContextWindowsError::AdapterUnavailable)
}

#[cfg(target_os = "windows")]
fn sample_platform() -> Result<EnvironmentSample, DesktopContextWindowsError> {
    let windows = list_windows().unwrap_or_default();
    let displays = list_displays();
    let foreground = foreground_from_windows(&windows);
    let idle_ms = idle_ms().unwrap_or(0);
    let power = sample_power();
    let mut names: Vec<String> = windows.iter().map(|w| w.owner_name.clone()).collect();
    if let Some(fg) = &foreground {
        names.push(fg.app_name.clone());
    }
    let meeting = meeting_from_owner_names(names.iter().map(String::as_str));
    Ok(EnvironmentSample {
        windows: filter_windows(windows, std::process::id()),
        foreground,
        displays,
        idle_ms,
        power,
        meeting,
        observed_at_ms: now_ms(),
    })
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Privacy-preserving meeting detection from process/app names.
///
/// Priority: Zoom > Teams > Meet > Webex. Bare "meet" alone is not enough
/// (avoids false positives like "Meetings"); requires "google meet" or
/// "chrome.*meet" style markers, matching the macOS heuristics.
#[must_use]
pub fn meeting_from_owner_names<'a, I>(names: I) -> MeetingFact
where
    I: IntoIterator<Item = &'a str>,
{
    let mut hint = MeetingHint::None;
    for name in names {
        let next = meeting_hint_from_process_name(name);
        hint = prefer_meeting_hint(hint, next);
    }
    if hint == MeetingHint::None {
        MeetingFact::inactive()
    } else {
        MeetingFact {
            active: true,
            hint,
        }
    }
}

/// Returns a meeting hint when `name` matches a known conferencing app.
#[must_use]
pub fn meeting_hint_from_process_name(name: &str) -> MeetingHint {
    let lower = name.to_ascii_lowercase();
    if lower.contains("zoom") {
        return MeetingHint::Zoom;
    }
    if lower.contains("teams") || lower.contains("ms-teams") {
        return MeetingHint::Teams;
    }
    if lower.contains("google meet")
        || lower.contains("googlemeet")
        || (lower.contains("meet") && lower.contains("chrome"))
    {
        return MeetingHint::Meet;
    }
    if lower.contains("webex") || lower.contains("meeting center") {
        return MeetingHint::Webex;
    }
    // Soft conferencing signals (unknown): still suppress chatty autonomy.
    // FaceTime may appear via continuity; Discord / Skype / Phone Link are common.
    if lower.contains("discord")
        || lower.contains("skype")
        || lower.contains("facetime")
        || lower.contains("your phone")
        || lower.contains("phoneexperiencehost")
    {
        return MeetingHint::Unknown;
    }
    MeetingHint::None
}

/// Prefer a more specific meeting hint over a weaker one.
#[must_use]
pub const fn prefer_meeting_hint(current: MeetingHint, next: MeetingHint) -> MeetingHint {
    match (current, next) {
        (_, MeetingHint::None) => current,
        (MeetingHint::None, other) => other,
        (MeetingHint::Zoom, _) | (_, MeetingHint::Zoom) => MeetingHint::Zoom,
        (MeetingHint::Teams, _) | (_, MeetingHint::Teams) => MeetingHint::Teams,
        (MeetingHint::Meet, _) | (_, MeetingHint::Meet) => MeetingHint::Meet,
        (MeetingHint::Webex, _) | (_, MeetingHint::Webex) => MeetingHint::Webex,
        (MeetingHint::Unknown, other) => other,
    }
}

/// Drops shell, tiny, and own-process windows; clears titles.
#[must_use]
pub fn filter_windows(windows: Vec<WindowFact>, own_pid: u32) -> Vec<WindowFact> {
    let mut filtered: Vec<WindowFact> = windows
        .into_iter()
        .filter(|window| {
            window.owner_pid != own_pid
                && window.width > 8
                && window.height > 8
                && window.onscreen
                && !window.is_minimized
                && !window.is_shell
                && !is_shell_owner(&window.owner_name)
        })
        .map(|mut window| {
            window.title.clear();
            window
        })
        .collect();
    // Stable front-to-back order for occlusion (lower z = front).
    filtered.sort_by_key(|window| window.z_order);
    // Cap for stable sensory samples on crowded desktops.
    const MAX_WINDOWS: usize = 64;
    if filtered.len() > MAX_WINDOWS {
        filtered.truncate(MAX_WINDOWS);
    }
    filtered
}

fn is_shell_owner(owner: &str) -> bool {
    let lower = owner.to_ascii_lowercase();
    lower.contains("explorer")
        || lower.contains("shell_traywnd")
        || lower.contains("dwm")
        || lower.contains("searchui")
        || lower.contains("startmenuexperiencehost")
        || lower.contains("textinputhost")
        || lower.contains("shellexperiencehost")
        || lower.contains("systemsettings")
        || lower.contains("applicationframehost")
        || lower.contains("lockapp")
        || lower.contains("sihost")
}

#[cfg(target_os = "windows")]
fn list_displays() -> Vec<DisplayFact> {
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOF_PRIMARY,
    };

    struct Collect(Vec<DisplayFact>);
    static RESULT: Mutex<Option<Collect>> = Mutex::new(None);

    unsafe extern "system" fn callback(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        _data: LPARAM,
    ) -> BOOL {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        if unsafe { GetMonitorInfoW(monitor, &raw mut info) } == 0 {
            return 1;
        }
        let width = info.rcMonitor.right.saturating_sub(info.rcMonitor.left).max(0) as u32;
        let height = info.rcMonitor.bottom.saturating_sub(info.rcMonitor.top).max(0) as u32;
        let work_w = info.rcWork.right.saturating_sub(info.rcWork.left).max(0) as u32;
        let work_h = info.rcWork.bottom.saturating_sub(info.rcWork.top).max(0) as u32;
        if width == 0 || height == 0 {
            return 1;
        }
        if let Ok(mut guard) = RESULT.lock() {
            if let Some(collect) = guard.as_mut() {
                let index = collect.0.len();
                collect.0.push(DisplayFact {
                    id: format!("monitor-{index}"),
                    x: info.rcMonitor.left,
                    y: info.rcMonitor.top,
                    width,
                    height,
                    work_area: WorkAreaFact {
                        x: info.rcWork.left,
                        y: info.rcWork.top,
                        width: work_w.max(1),
                        height: work_h.max(1),
                    },
                    scale_factor: monitor_scale_factor(monitor),
                    is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
                });
            }
        }
        1
    }

    {
        let mut guard = match RESULT.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        *guard = Some(Collect(Vec::new()));
    }
    let _ = unsafe { EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(callback), 0) };
    let mut displays = {
        let mut guard = match RESULT.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.take().map(|c| c.0).unwrap_or_default()
    };
    displays.sort_by_key(|d| !d.is_primary);
    displays
}

#[cfg(target_os = "windows")]
fn monitor_scale_factor(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> f64 {
    // Prefer per-monitor DPI (Win8.1+); fall back to 1.0 when unavailable.
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    let mut dpi_x = 0u32;
    let mut dpi_y = 0u32;
    let hr = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y) };
    if hr < 0 || dpi_x == 0 {
        return 1.0;
    }
    let scale = f64::from(dpi_x) / 96.0;
    if scale.is_finite() && scale >= 0.5 {
        scale.clamp(0.5, 4.0)
    } else {
        1.0
    }
}

#[cfg(target_os = "windows")]
fn list_windows() -> Result<Vec<WindowFact>, DesktopContextWindowsError> {
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowRect, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible,
    };

    struct Collect(Vec<WindowFact>);
    static RESULT: Mutex<Option<Collect>> = Mutex::new(None);

    unsafe extern "system" fn callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &raw mut rect) } == 0 {
            return 1;
        }
        let width = rect.right.saturating_sub(rect.left).max(0) as u32;
        let height = rect.bottom.saturating_sub(rect.top).max(0) as u32;
        if width <= 8 || height <= 8 {
            return 1;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &raw mut pid) };
        let mut class_buf = [0u16; 256];
        let class_len =
            unsafe { GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32) };
        let class_name = if class_len > 0 {
            String::from_utf16_lossy(&class_buf[..class_len as usize])
        } else {
            String::new()
        };
        let process_name = process_name_for_pid(pid).unwrap_or_else(|| class_name.clone());
        let is_shell = is_shell_class(&class_name) || is_shell_owner(&process_name);
        let is_minimized = unsafe { IsIconic(hwnd) } != 0;
        if let Ok(mut guard) = RESULT.lock() {
            if let Some(collect) = guard.as_mut() {
                let z_order = i32::try_from(collect.0.len()).unwrap_or(i32::MAX);
                collect.0.push(WindowFact {
                    id: format!("{hwnd:?}"),
                    title: String::new(),
                    x: rect.left,
                    y: rect.top,
                    width,
                    height,
                    layer: 0,
                    z_order,
                    owner_pid: pid,
                    owner_name: process_name,
                    onscreen: true,
                    is_minimized,
                    is_shell,
                });
            }
        }
        1
    }

    fn is_shell_class(class_name: &str) -> bool {
        matches!(
            class_name,
            "Shell_TrayWnd"
                | "Shell_SecondaryTrayWnd"
                | "Progman"
                | "WorkerW"
                | "DV2ControlHost"
                | "Windows.UI.Core.CoreWindow"
        ) || class_name.starts_with("Windows.UI.")
    }

    {
        let mut guard = RESULT
            .lock()
            .map_err(|_| DesktopContextWindowsError::QueryFailed)?;
        *guard = Some(Collect(Vec::new()));
    }
    let ok = unsafe { EnumWindows(Some(callback), 0) };
    let windows = {
        let mut guard = RESULT
            .lock()
            .map_err(|_| DesktopContextWindowsError::QueryFailed)?;
        guard.take().map(|c| c.0).unwrap_or_default()
    };
    if ok == 0 && windows.is_empty() {
        return Err(DesktopContextWindowsError::QueryFailed);
    }
    Ok(windows)
}

#[cfg(target_os = "windows")]
fn process_name_for_pid(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 512];
    let mut size = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &raw mut size) };
    unsafe { CloseHandle(handle) };
    if ok == 0 || size == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buf[..size as usize]);
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path.as_str())
        .to_owned();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(target_os = "windows")]
fn foreground_from_windows(windows: &[WindowFact]) -> Option<ForegroundFact> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return windows.first().map(|window| ForegroundFact {
            app_name: window.owner_name.clone(),
            pid: window.owner_pid,
            degraded: true,
        });
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &raw mut pid) };
    windows
        .iter()
        .find(|window| window.owner_pid == pid)
        .map(|window| ForegroundFact {
            app_name: window.owner_name.clone(),
            pid,
            degraded: false,
        })
        .or_else(|| {
            let name = process_name_for_pid(pid).unwrap_or_default();
            Some(ForegroundFact {
                app_name: name,
                pid,
                degraded: name.is_empty(),
            })
        })
}

#[cfg(target_os = "windows")]
fn idle_ms() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: u32::try_from(std::mem::size_of::<LASTINPUTINFO>()).unwrap_or(u32::MAX),
        dwTime: 0,
    };
    if unsafe { GetLastInputInfo(&raw mut info) } == 0 {
        return None;
    }
    let now = unsafe { GetTickCount() };
    Some(u64::from(now.wrapping_sub(info.dwTime)))
}

#[cfg(target_os = "windows")]
fn sample_power() -> PowerFact {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS {
        ACLineStatus: 255,
        BatteryFlag: 255,
        BatteryLifePercent: 255,
        SystemStatusFlag: 0,
        BatteryLifeTime: u32::MAX,
        BatteryFullLifeTime: u32::MAX,
    };
    if unsafe { GetSystemPowerStatus(&raw mut status) } == 0 {
        return PowerFact::unavailable();
    }
    let on_battery = status.ACLineStatus == 0;
    let charging = status.BatteryFlag & 8 != 0;
    let battery_percent = if status.BatteryLifePercent <= 100 {
        Some(status.BatteryLifePercent)
    } else {
        None
    };
    PowerFact {
        available: true,
        on_battery,
        battery_percent,
        charging,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_timeout() {
        assert_eq!(
            sample(Duration::ZERO),
            Err(DesktopContextWindowsError::InvalidTimeout)
        );
    }

    #[test]
    fn meeting_detects_teams_and_zoom_priority() {
        let fact = meeting_from_owner_names(["chrome.exe", "ms-teams.exe"]);
        assert!(fact.active);
        assert_eq!(fact.hint, MeetingHint::Teams);

        let fact = meeting_from_owner_names(["ms-teams.exe", "Zoom.exe"]);
        assert_eq!(fact.hint, MeetingHint::Zoom);

        let fact = meeting_from_owner_names(["webex.exe"]);
        assert_eq!(fact.hint, MeetingHint::Webex);
    }

    #[test]
    fn meeting_avoids_bare_meet_false_positive() {
        assert_eq!(
            meeting_hint_from_process_name("Meetings.exe"),
            MeetingHint::None
        );
        assert_eq!(
            meeting_hint_from_process_name("Google Meet"),
            MeetingHint::Meet
        );
        assert_eq!(
            meeting_hint_from_process_name("chrome_google_meet"),
            MeetingHint::Meet
        );
    }

    #[test]
    fn filter_clears_titles_and_own_pid() {
        let windows = vec![WindowFact {
            id: "1".into(),
            title: "SECRET".into(),
            x: 0,
            y: 0,
            width: 200,
            height: 200,
            layer: 0,
            z_order: 0,
            owner_pid: 7,
            owner_name: "Code".into(),
            onscreen: true,
            is_minimized: false,
            is_shell: false,
        }];
        let filtered = filter_windows(windows, 7);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_keeps_foreign_and_clears_title() {
        let windows = vec![WindowFact {
            id: "2".into(),
            title: "SECRET".into(),
            x: 10,
            y: 10,
            width: 400,
            height: 300,
            layer: 0,
            z_order: 0,
            owner_pid: 99,
            owner_name: "notepad.exe".into(),
            onscreen: true,
            is_minimized: false,
            is_shell: false,
        }];
        let filtered = filter_windows(windows, 7);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].title.is_empty());
        assert_eq!(filtered[0].owner_name, "notepad.exe");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_returns_adapter_unavailable() {
        assert_eq!(
            sample(Duration::from_secs(1)),
            Err(DesktopContextWindowsError::AdapterUnavailable)
        );
    }

    #[test]
    fn meeting_detects_discord_as_unknown() {
        let fact = meeting_from_owner_names(["Discord", "Chrome"]);
        assert!(fact.active);
        assert_eq!(fact.hint, MeetingHint::Unknown);
    }

    #[test]
    fn meeting_soft_signals_and_priority() {
        assert_eq!(
            meeting_hint_from_process_name("Skype for Desktop"),
            MeetingHint::Unknown
        );
        assert_eq!(
            meeting_hint_from_process_name("FaceTime"),
            MeetingHint::Unknown
        );
        // Zoom still wins over soft signals.
        let fact = meeting_from_owner_names(["Discord", "zoom.exe", "Skype"]);
        assert!(fact.active);
        assert_eq!(fact.hint, MeetingHint::Zoom);
    }

    #[test]
    fn shell_owner_heuristics_cover_common_hosts() {
        assert!(is_shell_owner("explorer.exe"));
        assert!(is_shell_owner("ShellExperienceHost"));
        assert!(is_shell_owner("TextInputHost"));
        assert!(!is_shell_owner("Code.exe"));
        assert!(!is_shell_owner("chrome"));
    }
}
