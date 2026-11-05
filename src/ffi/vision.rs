//! Apple Vision text recognition (part of macOS, works offline) through native/vision.m.

use std::ffi::{c_char, c_int, CStr};
use std::ptr::NonNull;

unsafe extern "C" {
    fn k_vision_ocr(gray: *const u8, w: c_int, h: c_int) -> *mut c_char;
}

/// Grey page picture (w*h bytes) -> one line per recognized line, "x\ty\tw\th\ttext" with Vision's
/// normalized box (origin bottom left). None if the picture is not w*h or Vision failed.
pub fn recognize(gray: &[u8], w: usize, h: usize) -> Option<String> {
    if w == 0 || h == 0 || w.checked_mul(h)? != gray.len() {
        return None;
    }
    let (cw, ch) = (c_int::try_from(w).ok()?, c_int::try_from(h).ok()?);
    // SAFETY: `gray` holds exactly w*h bytes (checked above), which is all the shim reads.
    let p = NonNull::new(unsafe { k_vision_ocr(gray.as_ptr(), cw, ch) })?;
    // SAFETY: the shim returns a NUL-terminated malloc'd string; it is copied, then freed once.
    let s = unsafe { CStr::from_ptr(p.as_ptr()) }.to_string_lossy().into_owned();
    unsafe { libc::free(p.as_ptr().cast()) };
    Some(s)
}
