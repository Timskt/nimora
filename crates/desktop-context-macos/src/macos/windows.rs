//! CGWindowListCopyWindowInfo sampling via pure CoreGraphics / CoreFoundation FFI.
//!
//! Window titles are never read (privacy). Geometry and owner metadata only.

use super::cf::{
    dict_get, dict_i64, dict_string, CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef,
    CFDictionaryRef, CFStringRef, CfRetained,
};
use crate::filter::{is_shell_layer, is_shell_owner_name, is_zero_size};
use crate::types::WindowFact;

/// `kCGNullWindowID`
const CG_NULL_WINDOW_ID: u32 = 0;
/// `kCGWindowListOptionAll`
const CG_WINDOW_LIST_OPTION_ALL: u32 = 0;
/// `kCGWindowListExcludeDesktopElements`
const CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGWindowNumber: CFStringRef;
    static kCGWindowOwnerPID: CFStringRef;
    static kCGWindowLayer: CFStringRef;
    static kCGWindowOwnerName: CFStringRef;
    static kCGWindowIsOnscreen: CFStringRef;
    static kCGWindowBounds: CFStringRef;

    /// Returns a retained CFArray of CFDictionary window info, or null.
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;

    /// Fills `rect` from a bounds CFDictionary `{X,Y,Width,Height}`.
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowListError {
    Unavailable,
    PermissionDenied,
}

/// Lists CG windows (including off-screen), excluding desktop elements.
pub(super) fn list_windows() -> Result<Vec<WindowFact>, WindowListError> {
    let option = CG_WINDOW_LIST_OPTION_ALL | CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
    // SAFETY: system API; option/relative IDs are documented constants.
    let array_ptr = unsafe { CGWindowListCopyWindowInfo(option, CG_NULL_WINDOW_ID) };
    if array_ptr.is_null() {
        // Null result is treated as permission denial or API failure.
        return Err(WindowListError::PermissionDenied);
    }
    let array = CfRetained::new(array_ptr);

    // SAFETY: array is a live retained CFArray from CGWindowListCopyWindowInfo.
    let count = unsafe { CFArrayGetCount(array.as_ptr()) };
    if count < 0 {
        return Err(WindowListError::Unavailable);
    }
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut windows = Vec::with_capacity(count as usize);
    let mut parse_failures = 0usize;
    for z_order in 0..count {
        // SAFETY: index is within CFArrayGetCount bounds.
        let value = unsafe { CFArrayGetValueAtIndex(array.as_ptr(), z_order) };
        if value.is_null() {
            parse_failures += 1;
            continue;
        }
        let dict: CFDictionaryRef = value;
        if let Some(window) = parse_window_dict(dict, z_order as usize) {
            windows.push(window);
        } else {
            parse_failures += 1;
        }
    }

    if windows.is_empty() && parse_failures > 0 {
        // All entries failed to parse — treat as unavailable rather than empty success.
        return Err(WindowListError::Unavailable);
    }

    Ok(windows)
}

fn parse_window_dict(dict: CFDictionaryRef, z_order: usize) -> Option<WindowFact> {
    // SAFETY: kCGWindow* keys are process-lifetime CFString constants from CoreGraphics.
    let id = dict_i64(dict, unsafe { kCGWindowNumber }).unwrap_or(0);
    let owner_pid =
        u32::try_from(dict_i64(dict, unsafe { kCGWindowOwnerPID }).unwrap_or(0)).unwrap_or(0);
    let layer =
        i32::try_from(dict_i64(dict, unsafe { kCGWindowLayer }).unwrap_or(0)).unwrap_or(0);
    let owner_name = dict_string(dict, unsafe { kCGWindowOwnerName }).unwrap_or_default();
    let onscreen = dict_i64(dict, unsafe { kCGWindowIsOnscreen }).unwrap_or(0) != 0;

    let (x, y, width, height) = dict_bounds(dict)?;

    // Pre-mark shell so callers can also inspect before filtering.
    let is_shell = is_shell_layer(layer) || is_shell_owner_name(&owner_name);
    // Off-screen normal windows are treated as minimized best-effort
    // (CGWindowList does not expose a dedicated minimized flag).
    let is_minimized = !onscreen && !is_shell && !is_zero_size(width, height);

    Some(WindowFact {
        id: id.to_string(),
        // Privacy: never populate titles (do not read kCGWindowName).
        title: String::new(),
        x,
        y,
        width,
        height,
        layer,
        z_order: i32::try_from(z_order).unwrap_or(i32::MAX),
        owner_pid,
        owner_name,
        onscreen,
        is_minimized,
        is_shell,
    })
}

fn dict_bounds(dict: CFDictionaryRef) -> Option<(i32, i32, u32, u32)> {
    // SAFETY: kCGWindowBounds is a process-lifetime CFString constant.
    let bounds_key = unsafe { kCGWindowBounds };
    let bounds_value = dict_get(dict, bounds_key)?;
    let bounds_dict: CFDictionaryRef = bounds_value;
    let mut rect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 0.0,
            height: 0.0,
        },
    };
    // SAFETY: bounds_dict is a CFDictionary from the window info entry.
    let ok = unsafe { CGRectMakeWithDictionaryRepresentation(bounds_dict, &mut rect) };
    if ok == 0 {
        return None;
    }
    Some((
        rect.origin.x as i32,
        rect.origin.y as i32,
        non_negative_u32(rect.size.width),
        non_negative_u32(rect.size.height),
    ))
}

fn non_negative_u32(value: f64) -> u32 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        value as u32
    }
}
