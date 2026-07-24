//! Idle time via CGEventSourceSecondsSinceLastEventType (pure CoreGraphics FFI).

/// `kCGAnyInputEventType` — any input event (keyboard, mouse, tablet, …).
const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

/// `kCGEventSourceStateCombinedSessionState`
const COMBINED_SESSION_STATE: i32 = 0;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// Seconds since the last input event of the given type.
    ///
    /// # Safety
    ///
    /// Safe to call with the documented CoreGraphics state IDs; returns a
    /// non-negative `CFTimeInterval` (double) on success.
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

/// Returns system idle time in milliseconds, or `None` if the query fails.
pub(super) fn idle_ms() -> Option<u64> {
    // CombinedSessionState matches user session input, not HID system-wide.
    // SAFETY: CoreGraphics system call with fixed constants; no pointers.
    let seconds =
        unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT_TYPE) };
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let millis = seconds * 1000.0;
    if !millis.is_finite() || millis < 0.0 {
        return None;
    }
    // Cap before cast: u64::MAX cannot convert losslessly into f64 on all platforms.
    if millis >= 9_007_199_254_740_992.0 {
        Some(u64::MAX)
    } else {
        Some(millis as u64)
    }
}
