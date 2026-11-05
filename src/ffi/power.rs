//! Keeping the Mac awake (not the screen) while an MP3 is made: the same power assertion macOS's
//! `caffeinate -i` makes.

use std::ffi::{c_char, c_void, CStr};

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(t: *const c_void, level: u32, name: *const c_void, id: *mut u32) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, enc: u32) -> *const c_void;
    fn CFRelease(p: *const c_void);
}

const UTF8: u32 = 0x0800_0100;
const ASSERTION_ON: u32 = 255;

/// A CoreFoundation string, released once when dropped.
struct CfString(*const c_void);

impl CfString {
    fn new(s: &CStr) -> Option<CfString> {
        // SAFETY: `s` is NUL-terminated and lives through the call; null means failure.
        let p = unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), UTF8) };
        if p.is_null() { None } else { Some(CfString(p)) }
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        // SAFETY: created above (not null), released exactly once.
        unsafe { CFRelease(self.0) }
    }
}

/// While this value lives, the Mac does not go to sleep on its own.
pub struct Awake(u32);

impl Awake {
    pub fn new(reason: &CStr) -> Option<Awake> {
        let kind = CfString::new(c"PreventUserIdleSystemSleep")?;
        let name = CfString::new(reason)?;
        let mut id = 0u32;
        // SAFETY: both strings are valid CFStrings for the call (IOKit retains what it keeps); `id`
        // is valid for writing.
        let r = unsafe { IOPMAssertionCreateWithName(kind.0, ASSERTION_ON, name.0, &mut id) };
        if r == 0 { Some(Awake(id)) } else { None }
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        // SAFETY: `self.0` is an assertion this value created; released exactly once.
        unsafe {
            IOPMAssertionRelease(self.0);
        }
    }
}
