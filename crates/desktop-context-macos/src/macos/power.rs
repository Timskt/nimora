//! Battery / AC power via IOKit power-sources APIs (pure FFI, no crates).
//!
//! Uses `IOPSCopyPowerSourcesInfo` / `IOPSCopyPowerSourcesList` /
//! `IOPSGetPowerSourceDescription`. On failure returns
//! [`PowerFact::unavailable`] (fail closed).

use super::cf::{
    cfstring_from_str, dict_bool, dict_i64, dict_string, CFArrayGetCount, CFArrayGetValueAtIndex,
    CFArrayRef, CFDictionaryRef, CFTypeRef, CfRetained,
};
use crate::types::PowerFact;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    /// Snapshot blob of power sources (+1 retain), or null.
    fn IOPSCopyPowerSourcesInfo() -> CFTypeRef;
    /// Array of power-source refs for `blob` (+1 retain), or null.
    fn IOPSCopyPowerSourcesList(blob: CFTypeRef) -> CFArrayRef;
    /// Description dictionary for one power source (unowned).
    fn IOPSGetPowerSourceDescription(blob: CFTypeRef, ps: CFTypeRef) -> CFDictionaryRef;
}

/// Samples internal battery / AC state. Fail-closed when IOKit is unavailable.
pub(super) fn sample_power() -> PowerFact {
    // SAFETY: IOKit Create/Copy APIs; null checked before use; retained via CfRetained.
    let info = unsafe { IOPSCopyPowerSourcesInfo() };
    if info.is_null() {
        return PowerFact::unavailable();
    }
    let info = CfRetained::new(info);

    let list_ptr = unsafe { IOPSCopyPowerSourcesList(info.as_ptr()) };
    if list_ptr.is_null() {
        return PowerFact::unavailable();
    }
    let list = CfRetained::new(list_ptr);

    let count = unsafe { CFArrayGetCount(list.as_ptr()) };
    if count <= 0 {
        return PowerFact::unavailable();
    }

    // Prefer the first present source that looks like a battery (or any present source).
    for index in 0..count {
        let ps = unsafe { CFArrayGetValueAtIndex(list.as_ptr(), index) };
        if ps.is_null() {
            continue;
        }
        let desc = unsafe { IOPSGetPowerSourceDescription(info.as_ptr(), ps) };
        if desc.is_null() {
            continue;
        }
        if let Some(fact) = parse_power_source(desc) {
            return fact;
        }
    }

    PowerFact::unavailable()
}

fn parse_power_source(desc: CFDictionaryRef) -> Option<PowerFact> {
    let is_present_key = cfstring_from_str("Is Present")?;
    // Skip sources that are not present (empty external battery slots, etc.).
    if let Some(false) = dict_bool(desc, is_present_key.as_ptr()) {
        return None;
    }

    let state_key = cfstring_from_str("Power Source State")?;
    let state = dict_string(desc, state_key.as_ptr()).unwrap_or_default();
    // "Battery Power" means drawing from battery; "AC Power" means wall power.
    let on_battery = state.eq_ignore_ascii_case("Battery Power");

    let charging_key = cfstring_from_str("Is Charging")?;
    let charging = dict_bool(desc, charging_key.as_ptr()).unwrap_or(false);

    let current_key = cfstring_from_str("Current Capacity")?;
    let max_key = cfstring_from_str("Max Capacity")?;
    let current = dict_i64(desc, current_key.as_ptr());
    let max = dict_i64(desc, max_key.as_ptr());

    let battery_percent = match (current, max) {
        (Some(cur), Some(max_cap)) if max_cap > 0 => {
            let pct = (cur.saturating_mul(100)) / max_cap;
            let clamped = pct.clamp(0, 100);
            u8::try_from(clamped).ok()
        }
        (Some(cur), None) if (0..=100).contains(&cur) => u8::try_from(cur).ok(),
        _ => None,
    };

    Some(PowerFact {
        available: true,
        on_battery,
        battery_percent,
        charging,
    })
}
