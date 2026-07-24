//! Minimal CoreFoundation FFI used by window and power sampling.
//!
//! Links the system CoreFoundation framework only — no crates.io bindings.

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

pub type CFTypeRef = *const c_void;
pub type CFIndex = isize;
pub type CFStringRef = *const c_void;
pub type CFArrayRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFNumberRef = *const c_void;
pub type CFBooleanRef = *const c_void;
pub type CFAllocatorRef = *const c_void;
pub type CFTypeID = usize;
pub type CFStringEncoding = u32;
pub type CFNumberType = i32;
pub type Boolean = u8;

pub const kCFStringEncodingUTF8: CFStringEncoding = 0x0800_0100;
pub const kCFNumberSInt32Type: CFNumberType = 3;
pub const kCFNumberSInt64Type: CFNumberType = 4;
pub const kCFNumberDoubleType: CFNumberType = 13;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;

    pub fn CFArrayGetCount(the_array: CFArrayRef) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(the_array: CFArrayRef, idx: CFIndex) -> *const c_void;

    pub fn CFDictionaryGetValueIfPresent(
        the_dict: CFDictionaryRef,
        key: *const c_void,
        value: *mut *const c_void,
    ) -> Boolean;

    pub fn CFNumberGetTypeID() -> CFTypeID;
    pub fn CFNumberGetValue(
        number: CFNumberRef,
        the_type: CFNumberType,
        value_ptr: *mut c_void,
    ) -> Boolean;

    pub fn CFBooleanGetTypeID() -> CFTypeID;
    pub fn CFBooleanGetValue(boolean: CFBooleanRef) -> Boolean;

    pub fn CFStringGetTypeID() -> CFTypeID;
    pub fn CFStringGetLength(the_string: CFStringRef) -> CFIndex;
    pub fn CFStringGetMaximumSizeForEncoding(
        length: CFIndex,
        encoding: CFStringEncoding,
    ) -> CFIndex;
    pub fn CFStringGetCString(
        the_string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: CFStringEncoding,
    ) -> Boolean;
    pub fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: CFStringEncoding,
    ) -> CFStringRef;
}

/// RAII release for a +1 CF object.
pub struct CfRetained(CFTypeRef);

impl CfRetained {
    /// Takes ownership of a Create/Copy return (+1 retain). Null is allowed.
    pub const fn new(ptr: CFTypeRef) -> Self {
        Self(ptr)
    }

    pub const fn as_ptr(&self) -> CFTypeRef {
        self.0
    }

    pub const fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

impl Drop for CfRetained {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointer came from a Create/Copy API and is released once.
            unsafe { CFRelease(self.0) };
            self.0 = ptr::null();
        }
    }
}

/// Creates a temporary UTF-8 CFString (+1). Caller owns via [`CfRetained`].
pub fn cfstring_from_str(s: &str) -> Option<CfRetained> {
    let c = CString::new(s).ok()?;
    // SAFETY: null allocator uses default; c is null-terminated UTF-8.
    let ptr = unsafe {
        CFStringCreateWithCString(ptr::null(), c.as_ptr(), kCFStringEncodingUTF8)
    };
    if ptr.is_null() {
        None
    } else {
        Some(CfRetained::new(ptr))
    }
}

pub fn cfstring_to_string(s: CFStringRef) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: s is a live CFStringRef for the duration of this call.
    unsafe {
        if CFGetTypeID(s) != CFStringGetTypeID() {
            return String::new();
        }
        let len = CFStringGetLength(s);
        if len == 0 {
            return String::new();
        }
        let max = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8);
        if max <= 0 {
            return String::new();
        }
        let mut buf = vec![0u8; (max as usize).saturating_add(1)];
        let ok = CFStringGetCString(
            s,
            buf.as_mut_ptr().cast::<c_char>(),
            buf.len() as CFIndex,
            kCFStringEncodingUTF8,
        );
        if ok == 0 {
            return String::new();
        }
        CStr::from_ptr(buf.as_ptr().cast::<c_char>())
            .to_string_lossy()
            .into_owned()
    }
}

pub fn dict_get(dict: CFDictionaryRef, key: CFStringRef) -> Option<*const c_void> {
    if dict.is_null() || key.is_null() {
        return None;
    }
    let mut value: *const c_void = ptr::null();
    // SAFETY: dict/key are live CF objects; value out-param is stack storage.
    let present = unsafe {
        CFDictionaryGetValueIfPresent(dict, key.cast(), &mut value)
    };
    if present == 0 || value.is_null() {
        None
    } else {
        Some(value)
    }
}

pub fn dict_i64(dict: CFDictionaryRef, key: CFStringRef) -> Option<i64> {
    let value = dict_get(dict, key)?;
    // SAFETY: value obtained from CF dictionary; type-checked before use.
    unsafe {
        if CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let number = value.cast();
        let mut out: i64 = 0;
        if CFNumberGetValue(
            number,
            kCFNumberSInt64Type,
            (&raw mut out).cast::<c_void>(),
        ) != 0
        {
            return Some(out);
        }
        let mut out32: i32 = 0;
        if CFNumberGetValue(
            number,
            kCFNumberSInt32Type,
            (&raw mut out32).cast::<c_void>(),
        ) != 0
        {
            return Some(i64::from(out32));
        }
        let mut out_f: f64 = 0.0;
        if CFNumberGetValue(
            number,
            kCFNumberDoubleType,
            (&raw mut out_f).cast::<c_void>(),
        ) != 0
            && out_f.is_finite()
        {
            return Some(out_f as i64);
        }
        None
    }
}

pub fn dict_bool(dict: CFDictionaryRef, key: CFStringRef) -> Option<bool> {
    let value = dict_get(dict, key)?;
    // SAFETY: value from CF dictionary; type-checked.
    unsafe {
        if CFGetTypeID(value) != CFBooleanGetTypeID() {
            return None;
        }
        Some(CFBooleanGetValue(value.cast()) != 0)
    }
}

pub fn dict_string(dict: CFDictionaryRef, key: CFStringRef) -> Option<String> {
    let value = dict_get(dict, key)?;
    // SAFETY: value from CF dictionary; type-checked.
    unsafe {
        if CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        Some(cfstring_to_string(value.cast()))
    }
}
