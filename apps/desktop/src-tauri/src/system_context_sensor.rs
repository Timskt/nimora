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
const DO_NOT_DISTURB_NOTIFICATION: &str = "com.apple.donotdisturb.status";

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
pub fn sample_do_not_disturb(timeout: Duration) -> Result<bool, MacOsDoNotDisturbError> {
    if timeout.is_zero() {
        return Err(MacOsDoNotDisturbError::InvalidTimeout);
    }
    let mut child = Command::new("/usr/bin/notifyutil")
        .args(["-g", DO_NOT_DISTURB_NOTIFICATION])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| MacOsDoNotDisturbError::AdapterUnavailable)?;
    let Some(status) = child
        .wait_timeout(timeout)
        .map_err(|_| MacOsDoNotDisturbError::AdapterUnavailable)?
    else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(MacOsDoNotDisturbError::SampleTimeout);
    };
    if !status.success() {
        return Err(MacOsDoNotDisturbError::SystemUnsupported);
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or(MacOsDoNotDisturbError::InvalidResponse)?
        .take(128)
        .read_to_string(&mut output)
        .map_err(|_| MacOsDoNotDisturbError::InvalidResponse)?;
    parse_do_not_disturb_state(&output)
}

#[cfg(target_os = "macos")]
fn parse_do_not_disturb_state(output: &str) -> Result<bool, MacOsDoNotDisturbError> {
    let mut fields = output.split_ascii_whitespace();
    match (fields.next(), fields.next(), fields.next()) {
        (Some(DO_NOT_DISTURB_NOTIFICATION), Some("0"), None) => Ok(false),
        (Some(DO_NOT_DISTURB_NOTIFICATION), Some("1"), None) => Ok(true),
        _ => Err(MacOsDoNotDisturbError::InvalidResponse),
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

#[cfg(target_os = "macos")]
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum MacOsDoNotDisturbError {
    #[error("macOS do-not-disturb sensor timeout must be non-zero")]
    InvalidTimeout,
    #[error("macOS do-not-disturb sensor adapter is unavailable")]
    AdapterUnavailable,
    #[error("macOS do-not-disturb sensor timed out")]
    SampleTimeout,
    #[error("macOS do-not-disturb sensor is unsupported")]
    SystemUnsupported,
    #[error("macOS do-not-disturb sensor returned an invalid response")]
    InvalidResponse,
}

#[cfg(target_os = "windows")]
pub fn sample_fullscreen(
    timeout: std::time::Duration,
) -> Result<bool, nimora_system_context_windows::WindowsFullscreenError> {
    nimora_system_context_windows::sample_fullscreen(timeout)
}

#[cfg(target_os = "windows")]
pub fn sample_activity(
    timeout: std::time::Duration,
) -> Result<
    nimora_system_context_windows::WindowsActivitySnapshot,
    nimora_system_context_windows::WindowsActivityError,
> {
    nimora_system_context_windows::sample_activity(timeout)
}


// ---------------------------------------------------------------------------
// Pure presence debounce / schedule helpers (platform-agnostic, no OS I/O).
// Hosts can stabilize fullscreen / DND before Presence decisions.
// ---------------------------------------------------------------------------

/// Minimum continuous observation before a presence boolean is considered stable.
pub const PRESENCE_BOOLEAN_HOLD_MS: u64 = 1_500;

/// Suggested re-sample interval while the published presence value is stable.
pub const PRESENCE_SAMPLE_CADENCE_MS: u64 = 5_000;

/// Faster re-sample interval while a hold is in progress (pending change).
pub const PRESENCE_SAMPLE_CADENCE_UNSTABLE_MS: u64 = 1_000;

/// Debounce gate for presence booleans (fullscreen / DND).
///
/// Emits only after the new value has been observed continuously for the hold
/// window, preventing flicker into Presence / lifeform suppress paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenceBooleanGate {
    published: Option<bool>,
    pending: Option<bool>,
    pending_since_ms: u64,
    hold_ms: u64,
}

impl Default for PresenceBooleanGate {
    fn default() -> Self {
        Self::new(PRESENCE_BOOLEAN_HOLD_MS)
    }
}

impl PresenceBooleanGate {
    /// Builds a gate with a custom hold interval (milliseconds).
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

    /// True while a candidate value differs from the published one and hold is open.
    #[must_use]
    pub const fn is_holding(self) -> bool {
        self.pending.is_some()
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

    /// Force-publish (tests / fail-closed reset after permission loss).
    #[must_use]
    pub fn force(mut self, value: bool) -> Self {
        self.published = Some(value);
        self.pending = None;
        self
    }
}

/// Maps sample success into an optional raw boolean; errors are ignored (keep last).
#[must_use]
pub fn presence_raw_from_result<E>(result: Result<bool, E>) -> Option<bool> {
    result.ok()
}

/// Next sample delay: faster while any gate is still holding a pending flip.
#[must_use]
pub const fn presence_sample_delay_ms(holding: bool) -> u64 {
    if holding {
        PRESENCE_SAMPLE_CADENCE_UNSTABLE_MS
    } else {
        PRESENCE_SAMPLE_CADENCE_MS
    }
}

/// Whether two successive presence samples agree (single-shot convenience).
#[must_use]
pub const fn presence_values_agree(a: bool, b: bool) -> bool {
    a == b
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
        assert_eq!(
            sample_do_not_disturb(Duration::ZERO),
            Err(MacOsDoNotDisturbError::InvalidTimeout)
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

    #[cfg(target_os = "macos")]
    #[test]
    fn do_not_disturb_parser_accepts_only_the_named_boolean_state() {
        assert_eq!(
            parse_do_not_disturb_state("com.apple.donotdisturb.status 0\n"),
            Ok(false)
        );
        assert_eq!(
            parse_do_not_disturb_state("com.apple.donotdisturb.status 1\n"),
            Ok(true)
        );
        for invalid in [
            "com.apple.donotdisturb.status 2",
            "com.apple.donotdisturb.status 1 extra",
            "other.status 1",
            "1",
        ] {
            assert_eq!(
                parse_do_not_disturb_state(invalid),
                Err(MacOsDoNotDisturbError::InvalidResponse)
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn do_not_disturb_adapter_never_reads_user_databases() {
        assert_eq!(DO_NOT_DISTURB_NOTIFICATION, "com.apple.donotdisturb.status");
        for forbidden in ["Library", "Assertions.json", "defaults", "sqlite"] {
            assert!(!DO_NOT_DISTURB_NOTIFICATION.contains(forbidden));
        }
    }
}


#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn presence_gate_holds_before_publish() {
        let gate = PresenceBooleanGate::new(100);
        let (gate, emit) = gate.observe(true, 0);
        assert_eq!(emit, None);
        assert!(gate.is_holding());
        let (gate, emit) = gate.observe(true, 50);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(true, 100);
        assert_eq!(emit, Some(true));
        assert!(!gate.is_holding());
        assert_eq!(gate.published(), Some(true));
        let (gate, emit) = gate.observe(true, 200);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(false, 200);
        assert_eq!(emit, None);
        assert!(gate.is_holding());
        let (_gate, emit) = gate.observe(false, 300);
        assert_eq!(emit, Some(false));
    }

    #[test]
    fn presence_gate_resets_pending_on_flicker() {
        let gate = PresenceBooleanGate::new(100);
        let (gate, _) = gate.observe(true, 0);
        let (gate, emit) = gate.observe(false, 50);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(false, 149);
        assert_eq!(emit, None);
        let (gate, emit) = gate.observe(false, 150);
        assert_eq!(emit, Some(false));
        assert_eq!(gate.published(), Some(false));
    }

    #[test]
    fn presence_schedule_and_result_helpers() {
        assert_eq!(presence_sample_delay_ms(false), PRESENCE_SAMPLE_CADENCE_MS);
        assert_eq!(
            presence_sample_delay_ms(true),
            PRESENCE_SAMPLE_CADENCE_UNSTABLE_MS
        );
        assert_eq!(presence_raw_from_result::<()>(Ok(true)), Some(true));
        assert_eq!(presence_raw_from_result::<&str>(Err("denied")), None);
        assert!(presence_values_agree(true, true));
        assert!(!presence_values_agree(true, false));
        let forced = PresenceBooleanGate::default().force(true);
        assert_eq!(forced.published(), Some(true));
        assert!(!forced.is_holding());
    }
}
