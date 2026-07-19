#[cfg(target_os = "macos")]
use std::{
    io::Read,
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(target_os = "macos")]
use wait_timeout::ChildExt;

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
pub fn sample_fullscreen(
    timeout: std::time::Duration,
) -> Result<bool, nimora_system_context_windows::WindowsFullscreenError> {
    nimora_system_context_windows::sample_fullscreen(timeout)
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
}
