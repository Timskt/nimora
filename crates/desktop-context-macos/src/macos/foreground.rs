//! Foreground app via NSWorkspace.frontmostApplication (pure objc runtime FFI).

use crate::types::ForegroundFact;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_msgSend();
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

type MsgSendId = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type MsgSendI32 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;

fn sel(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sel_registerName(c.as_ptr()) }
}

fn class(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { objc_getClass(c.as_ptr()) }
}

/// Returns the frontmost application via NSWorkspace when available.
pub(super) fn frontmost_application() -> Option<ForegroundFact> {
    let ns_workspace = class("NSWorkspace");
    if ns_workspace.is_null() {
        return None;
    }
    // SAFETY: function pointer cast for objc_msgSend with known signatures.
    let msg_id: MsgSendId = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let msg_i32: MsgSendI32 = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };

    let shared = unsafe { msg_id(ns_workspace, sel("sharedWorkspace")) };
    if shared.is_null() {
        return None;
    }
    let app = unsafe { msg_id(shared, sel("frontmostApplication")) };
    if app.is_null() {
        return None;
    }
    let pid_raw = unsafe { msg_i32(app, sel("processIdentifier")) };
    if pid_raw <= 0 {
        return None;
    }
    let pid = pid_raw as u32;
    let name = nsstring_to_rust(unsafe { msg_id(app, sel("localizedName")) })
        .or_else(|| nsstring_to_rust(unsafe { msg_id(app, sel("bundleIdentifier")) }))
        .unwrap_or_default();
    Some(ForegroundFact {
        app_name: name,
        pid,
        degraded: false,
    })
}

fn nsstring_to_rust(ns: *mut c_void) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    let msg_id: MsgSendId = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    let cstr_ptr = unsafe { msg_id(ns, sel("UTF8String")) } as *const c_char;
    if cstr_ptr.is_null() {
        return None;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(cstr_ptr) }
        .to_string_lossy()
        .into_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
