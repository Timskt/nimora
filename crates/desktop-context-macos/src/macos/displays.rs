//! Multi-monitor sampling via CoreGraphics bounds + NSScreen work areas.

use crate::types::{DisplayFact, WorkAreaFact};
use std::ffi::c_void;
use std::os::raw::{c_char, c_double, c_long, c_ulong};

type CGDirectDisplayID = u32;
type CGError = i32;
type CGFloat = c_double;
type NSUInteger = c_ulong;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: CGFloat,
    y: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: CGFloat,
    height: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;
    fn CGMainDisplayID() -> CGDirectDisplayID;
    fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> usize;
    fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> usize;
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

type MsgSend0 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type MsgSendNSUInteger = unsafe extern "C" fn(*mut c_void, *mut c_void) -> NSUInteger;
type MsgSendIdAt = unsafe extern "C" fn(*mut c_void, *mut c_void, NSUInteger) -> *mut c_void;
type MsgSendRect = unsafe extern "C" fn(*mut c_void, *mut c_void) -> CGRect;

fn sel(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sel_registerName(c.as_ptr()) }
}

fn class(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { objc_getClass(c.as_ptr()) }
}

/// Lists active displays. Primary is sorted first. Work area falls back to full
/// bounds when NSScreen is unavailable.
pub(super) fn list_displays() -> Vec<DisplayFact> {
    let mut count = 0u32;
    // SAFETY: null buffer query for count.
    let err = unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &raw mut count) };
    if err != 0 || count == 0 {
        return fallback_main_display();
    }
    let mut ids = vec![0u32; count as usize];
    // SAFETY: buffer sized to count from previous call.
    let err = unsafe { CGGetActiveDisplayList(count, ids.as_mut_ptr(), &raw mut count) };
    if err != 0 || count == 0 {
        return fallback_main_display();
    }
    ids.truncate(count as usize);

    let main_id = unsafe { CGMainDisplayID() };
    let nsscreen_work = nsscreen_work_areas();

    let mut displays = Vec::with_capacity(ids.len());
    for id in ids {
        let bounds = unsafe { CGDisplayBounds(id) };
        let x = bounds.origin.x as i32;
        let y = bounds.origin.y as i32;
        let width = non_negative_u32(bounds.size.width).max(1);
        let height = non_negative_u32(bounds.size.height).max(1);
        let px_w = unsafe { CGDisplayPixelsWide(id) };
        let _px_h = unsafe { CGDisplayPixelsHigh(id) };
        let scale = if width > 0 && px_w > 0 {
            (px_w as f64 / f64::from(width)).clamp(1.0, 4.0)
        } else {
            1.0
        };
        let full = WorkAreaFact {
            x,
            y,
            width,
            height,
        };
        let work_area = match_work_area(x, y, width, height, &nsscreen_work).unwrap_or(full);
        displays.push(DisplayFact {
            id: format!("cg-{id}"),
            x,
            y,
            width,
            height,
            work_area,
            scale_factor: scale,
            is_primary: id == main_id,
        });
    }

    displays.sort_by_key(|d| !d.is_primary);
    if displays.is_empty() {
        fallback_main_display()
    } else {
        displays
    }
}

fn fallback_main_display() -> Vec<DisplayFact> {
    let id = unsafe { CGMainDisplayID() };
    if id == 0 {
        return Vec::new();
    }
    let bounds = unsafe { CGDisplayBounds(id) };
    let width = non_negative_u32(bounds.size.width);
    let height = non_negative_u32(bounds.size.height);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let x = bounds.origin.x as i32;
    let y = bounds.origin.y as i32;
    vec![DisplayFact {
        id: format!("cg-{id}"),
        x,
        y,
        width,
        height,
        work_area: WorkAreaFact {
            x,
            y,
            width,
            height,
        },
        scale_factor: 1.0,
        is_primary: true,
    }]
}

fn match_work_area(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    screens: &[(WorkAreaFact, WorkAreaFact)],
) -> Option<WorkAreaFact> {
    screens
        .iter()
        .find(|(frame, _)| {
            (frame.x - x).abs() <= 2
                && (frame.y - y).abs() <= 2
                && frame.width.abs_diff(width) <= 4
                && frame.height.abs_diff(height) <= 4
        })
        .map(|(_, work)| *work)
}

/// Returns (frame, visibleFrame) pairs from NSScreen.screens when AppKit loads.
fn nsscreen_work_areas() -> Vec<(WorkAreaFact, WorkAreaFact)> {
    let ns_screen = class("NSScreen");
    if ns_screen.is_null() {
        return Vec::new();
    }
    let screens_sel = sel("screens");
    let count_sel = sel("count");
    let object_at_sel = sel("objectAtIndex:");
    let frame_sel = sel("frame");
    let visible_sel = sel("visibleFrame");

    // SAFETY: function pointer cast for objc_msgSend with known signatures.
    let msg0: MsgSend0 = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let msg_count: MsgSendNSUInteger =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let msg_at: MsgSendIdAt = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let msg_rect: MsgSendRect = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };

    let screens = unsafe { msg0(ns_screen, screens_sel) };
    if screens.is_null() {
        return Vec::new();
    }
    let count = unsafe { msg_count(screens, count_sel) };
    if count == 0 {
        return Vec::new();
    }

    let mut frames = Vec::with_capacity(count as usize);
    let mut main_height: f64 = 0.0;
    for index in 0..count {
        let screen = unsafe { msg_at(screens, object_at_sel, index) };
        if screen.is_null() {
            continue;
        }
        let frame = unsafe { msg_rect(screen, frame_sel) };
        let visible = unsafe { msg_rect(screen, visible_sel) };
        main_height = main_height.max(frame.origin.y + frame.size.height);
        frames.push((frame, visible));
    }

    frames
        .into_iter()
        .map(|(frame, visible)| {
            (
                cocoa_to_cg(frame, main_height),
                cocoa_to_cg(visible, main_height),
            )
        })
        .collect()
}

fn cocoa_to_cg(rect: CGRect, main_height: f64) -> WorkAreaFact {
    let width = non_negative_u32(rect.size.width);
    let height = non_negative_u32(rect.size.height);
    let x = rect.origin.x as i32;
    // Cocoa Y is bottom-up; CG Y is top-down from primary top.
    let cg_y = (main_height - (rect.origin.y + rect.size.height)) as i32;
    WorkAreaFact {
        x,
        y: cg_y,
        width: width.max(1),
        height: height.max(1),
    }
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

// Keep NSInteger alias available for future AppKit selectors.
type _NSInteger = c_long;
const _: Option<_NSInteger> = None;
