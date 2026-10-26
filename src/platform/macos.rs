//! macOS: IOKit power assertions + CoreGraphics idle time, via plain FFI.

use crate::config::NudgeMethod;
use std::ffi::{c_char, c_void, CString};
use std::time::Duration;

type CFTypeRef = *const c_void;
type IOPMAssertionID = u32;
type IOReturn = i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;
const K_IOPM_USER_ACTIVE_LOCAL: u32 = 0;
const K_IO_RETURN_SUCCESS: IOReturn = 0;
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION: i32 = 0;
const K_CG_EVENT_SOURCE_STATE_HID_SYSTEM: i32 = 1;
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0;
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
const K_CG_HID_EVENT_TAP: u32 = 0;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(alloc: CFTypeRef, s: *const c_char, encoding: u32) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(b: CFTypeRef) -> u8;
    fn CFNumberGetValue(n: CFTypeRef, kind: isize, out: *mut c_void) -> u8;
    fn CFDictionaryCreate(
        alloc: CFTypeRef,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFTypeRef;
    static kCFBooleanTrue: CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: CFTypeRef,
        level: u32,
        name: CFTypeRef,
        id: *mut IOPMAssertionID,
    ) -> IOReturn;
    fn IOPMAssertionRelease(id: IOPMAssertionID) -> IOReturn;
    fn IOPMAssertionDeclareUserActivity(
        name: CFTypeRef,
        user_type: u32,
        id: *mut IOPMAssertionID,
    ) -> IOReturn;
    fn IOServiceMatching(name: *const c_char) -> CFTypeRef;
    fn IOServiceGetMatchingService(main_port: u32, matching: CFTypeRef) -> u32;
    fn IORegistryEntryCreateCFProperty(entry: u32, key: CFTypeRef, alloc: CFTypeRef, options: u32) -> CFTypeRef;
    fn IOObjectRelease(object: u32) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGSessionCopyCurrentDictionary() -> CFTypeRef;
    fn CGEventCreate(source: CFTypeRef) -> CFTypeRef;
    fn CGEventGetLocation(event: CFTypeRef) -> CGPoint;
    fn CGEventCreateMouseEvent(source: CFTypeRef, kind: u32, at: CGPoint, button: u32) -> CFTypeRef;
    fn CGEventPost(tap: u32, event: CFTypeRef);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> u8;
}

const K_CF_NUMBER_SINT64_TYPE: isize = 4;

/// Owned CFString.
struct CfString(CFTypeRef);

impl CfString {
    fn new(s: &str) -> Self {
        let c = CString::new(s).expect("no interior NUL");
        CfString(unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) })
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

const ASSERTION_NAME: &str = "Nudge keeps you available";

pub struct Platform {
    /// Power assertion preventing idle sleep (system or display).
    awake: Option<IOPMAssertionID>,
    /// Re-used user-activity assertion.
    activity: IOPMAssertionID,
}

fn idle_secs(state: i32) -> Duration {
    let s = unsafe { CGEventSourceSecondsSinceLastEventType(state, K_CG_ANY_INPUT_EVENT_TYPE) };
    Duration::from_secs_f64(if s.is_finite() && s > 0.0 { s } else { 0.0 })
}

/// IOHIDSystem's `HIDIdleTime` property (nanoseconds).
fn hid_idle_time() -> Option<Duration> {
    unsafe {
        let service = IOServiceGetMatchingService(0, IOServiceMatching(c"IOHIDSystem".as_ptr()));
        if service == 0 {
            return None;
        }
        let key = CfString::new("HIDIdleTime");
        let value = IORegistryEntryCreateCFProperty(service, key.0, std::ptr::null(), 0);
        IOObjectRelease(service);
        if value.is_null() {
            return None;
        }
        let mut ns: i64 = 0;
        let ok = CFNumberGetValue(value, K_CF_NUMBER_SINT64_TYPE, (&mut ns as *mut i64).cast());
        CFRelease(value);
        (ok != 0 && ns >= 0).then(|| Duration::from_nanos(ns as u64))
    }
}

/// Posts a mouse-moved event at the cursor's current position: zero distance.
fn post_zero_move() -> Result<(), String> {
    unsafe {
        let here = CGEventCreate(std::ptr::null());
        if here.is_null() {
            return Err("CGEventCreate failed".into());
        }
        let at = CGEventGetLocation(here);
        CFRelease(here);
        let ev = CGEventCreateMouseEvent(std::ptr::null(), K_CG_EVENT_MOUSE_MOVED, at, 0);
        if ev.is_null() {
            return Err("CGEventCreateMouseEvent failed".into());
        }
        CGEventPost(K_CG_HID_EVENT_TAP, ev);
        CFRelease(ev);
    }
    Ok(())
}

impl Platform {
    pub fn new() -> Self {
        Platform { awake: None, activity: 0 }
    }

    /// Time since the last *hardware* input — what the user actually did.
    pub fn idle_time(&self) -> Duration {
        idle_secs(K_CG_EVENT_SOURCE_STATE_HID_SYSTEM)
    }

    /// Every idle clock we can read. The flag marks the one chat apps (Chromium/Electron
    /// idle detection, which Teams uses) read.
    pub fn idle_report(&self) -> Vec<(&'static str, Duration, bool)> {
        let mut out = vec![
            ("CoreGraphics HID state", idle_secs(K_CG_EVENT_SOURCE_STATE_HID_SYSTEM), false),
            ("CoreGraphics session (apps read)", idle_secs(K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION), true),
        ];
        if let Some(d) = hid_idle_time() {
            out.push(("IOKit HIDIdleTime", d, false));
        }
        out
    }

    pub fn screen_locked(&self) -> bool {
        unsafe {
            let dict = CGSessionCopyCurrentDictionary();
            if dict.is_null() {
                return false;
            }
            let key = CfString::new("CGSSessionScreenIsLocked");
            let v = CFDictionaryGetValue(dict, key.0);
            let locked = !v.is_null() && CFGetTypeID(v) == CFBooleanGetTypeID() && CFBooleanGetValue(v) != 0;
            CFRelease(dict);
            locked
        }
    }

    /// Returns a short description of what was done.
    ///
    /// Measured on macOS 26: declaring user activity keeps the display awake but resets
    /// no idle clock; a posted zero-distance mouse move resets IOKit's HIDIdleTime always,
    /// and the CoreGraphics clock apps read only when Accessibility access is granted.
    pub fn nudge(&mut self, method: NudgeMethod) -> Result<&'static str, String> {
        let declare = |id: &mut IOPMAssertionID| {
            let name = CfString::new(ASSERTION_NAME);
            match unsafe { IOPMAssertionDeclareUserActivity(name.0, K_IOPM_USER_ACTIVE_LOCAL, id) } {
                K_IO_RETURN_SUCCESS => Ok(()),
                r => Err(format!("IOPMAssertionDeclareUserActivity failed: {r:#x}")),
            }
        };
        match method {
            NudgeMethod::Activity => declare(&mut self.activity).map(|_| "declared user activity"),
            NudgeMethod::Input => post_zero_move().map(|_| "posted zero-distance mouse move"),
            NudgeMethod::Auto => {
                post_zero_move()?;
                declare(&mut self.activity)?;
                Ok("posted zero-distance mouse move + declared user activity")
            }
        }
    }

    pub fn hold_awake(&mut self, display: bool) {
        self.release_awake();
        let kind = CfString::new(if display { "PreventUserIdleDisplaySleep" } else { "PreventUserIdleSystemSleep" });
        let name = CfString::new(ASSERTION_NAME);
        let mut id = 0;
        let r = unsafe { IOPMAssertionCreateWithName(kind.0, K_IOPM_ASSERTION_LEVEL_ON, name.0, &mut id) };
        if r == K_IO_RETURN_SUCCESS {
            self.awake = Some(id);
        } else {
            eprintln!("nudge: IOPMAssertionCreateWithName failed: {r:#x}");
        }
    }

    pub fn release_awake(&mut self) {
        if let Some(id) = self.awake.take() {
            unsafe { IOPMAssertionRelease(id) };
        }
    }

    /// Whether posted input reaches the idle clock apps read (Accessibility access).
    pub fn input_permission(&self) -> Option<bool> {
        Some(unsafe { AXIsProcessTrusted() } != 0)
    }

    /// Shows the system prompt asking the user to grant Accessibility access to Nudge.
    pub fn request_input_permission(&self) {
        unsafe {
            let key = CfString::new("AXTrustedCheckOptionPrompt");
            let keys = [key.0];
            let values = [kCFBooleanTrue];
            let opts = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                (&raw const kCFTypeDictionaryValueCallBacks).cast(),
            );
            AXIsProcessTrustedWithOptions(opts);
            if !opts.is_null() {
                CFRelease(opts);
            }
        }
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        self.release_awake();
        if self.activity != 0 {
            unsafe { IOPMAssertionRelease(self.activity) };
        }
    }
}
