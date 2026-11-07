//! The iPhone shell's calls into the app (ios/KhanaShell.swift, through ios/khana.h). Only C strings
//! and byte buffers cross: each is copied into Rust memory at once, and every answer is a buffer
//! the shell frees with `khana_free`. What the calls do is safe Rust in crate::android (the shell
//! design is the same as the Android app's); this file only moves bytes and keeps a panic from
//! crossing into Swift.

use std::ffi::{c_char, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Runs `f`, turning a panic into `dflt` (a panic must not unwind into Swift).
fn guard<R>(dflt: R, f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(dflt)
}

/// A C string (UTF-8) copied; null is the empty text.
///
/// SAFETY (for callers): `p` is null or a NUL-terminated string valid for the call.
unsafe fn text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: promised by the caller.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// A Rust byte vector handed to the shell: its pointer and length (freed by `khana_free`).
fn give(v: Vec<u8>, len: *mut usize) -> *mut u8 {
    let b = v.into_boxed_slice();
    let n = b.len();
    if !len.is_null() {
        // SAFETY: the shell passes a valid place for the length.
        unsafe { *len = n };
    }
    Box::into_raw(b) as *mut u8
}

/// khana_init(data, save, web, models, ort): the app's folders (UTF-8 paths).
///
/// # Safety
/// Each pointer is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn khana_init(data: *const c_char, save: *const c_char, web: *const c_char, models: *const c_char, ort: *const c_char) {
    // SAFETY: the shell passes C strings.
    let (data, save, web, models, ort) = unsafe { (text(data), text(save), text(web), text(models), text(ort)) };
    guard((), || crate::android::init(data, save, web, models, ort));
}

/// khana_token(&len): the page's request token (UTF-8, not NUL-terminated).
///
/// # Safety
/// `len` is a valid place for a length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn khana_token(len: *mut usize) -> *mut u8 {
    give(guard(String::new(), crate::android::token).into_bytes(), len)
}

/// khana_handle(method, path, query, token or null, body, body_len, &len): one request -> the reply
/// (crate::android::handle: status, headers, body).
///
/// # Safety
/// The strings are NUL-terminated (the token may be null); `body` holds `body_len` bytes (or is null
/// when `body_len` is 0); `len` is a valid place for a length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn khana_handle(method: *const c_char, path: *const c_char, query: *const c_char, token: *const c_char, body: *const u8, body_len: usize, len: *mut usize) -> *mut u8 {
    // SAFETY: promised by the caller.
    let (method, path, query) = unsafe { (text(method), text(path), text(query)) };
    let token = if token.is_null() { None } else { Some(unsafe { text(token) }) };
    // SAFETY: `body` holds `body_len` bytes.
    let mut body: Vec<u8> = if body.is_null() || body_len == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(body, body_len) }.to_vec() };
    let mut r = guard(crate::android::failure(), || crate::android::handle(&method, &path, &query, token.as_deref(), &body));
    // (the document's bytes and the page's text went through these two buffers: overwritten, not only freed)
    super::wipe::vec_u8(&mut body);
    let out = give(r.clone(), len);
    super::wipe::vec_u8(&mut r);
    out
}

/// khana_take_events(&len): crate::platform::take_events, as UTF-8.
///
/// # Safety
/// `len` is a valid place for a length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn khana_take_events(len: *mut usize) -> *mut u8 {
    give(guard(String::new(), crate::platform::take_events).into_bytes(), len)
}

/// khana_busy(): whether an MP3 or PDF job is running.
#[unsafe(no_mangle)]
pub extern "C" fn khana_busy() -> i32 {
    guard(0, || crate::platform::busy() as i32)
}

/// khana_shutdown(): stops the jobs (the app is closing).
#[unsafe(no_mangle)]
pub extern "C" fn khana_shutdown() {
    guard((), crate::android::shutdown);
}

/// khana_free(pointer, length): gives back a buffer an answer above returned.
///
/// # Safety
/// `p` and `len` are exactly what one of the calls above returned, not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn khana_free(p: *mut u8, len: usize) {
    if p.is_null() {
        return;
    }
    // SAFETY: `give` made this pointer from a boxed slice of this length.
    let mut b = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, len)) };
    super::wipe::bytes(&mut b);
}
