//! The Java shell's calls into the app (android/java/org/khana/reader/Native.java). Only byte arrays
//! cross: each is copied into Rust memory at once, and every answer is a new Java byte array. What
//! the calls do is safe Rust in crate::android; this file only moves bytes and keeps a panic from
//! crossing into Java.
//!
//! JNI is called through its function table by index (the order fixed by the JNI specification,
//! jni.h's JNINativeInterface), so no binding library is needed.

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// JNIEnv*: a pointer to the pointer to the function table.
type Env = *mut *const *const c_void;
type JObject = *mut c_void;
type JByteArray = JObject;
type JSize = i32;
type JBoolean = u8;

const GET_ARRAY_LENGTH: usize = 171;
const NEW_BYTE_ARRAY: usize = 176;
const GET_BYTE_ARRAY_REGION: usize = 200;
const SET_BYTE_ARRAY_REGION: usize = 208;
const EXCEPTION_CHECK: usize = 228;

/// The table entry `i` of `env`.
///
/// SAFETY (for callers): `env` is the JNIEnv this native method was called with, on its thread.
unsafe fn func(env: Env, i: usize) -> *const c_void {
    // SAFETY: a valid JNIEnv points to its function table, which has more than 228 entries.
    unsafe { *(*env).add(i) }
}

/// A Java byte array's bytes (none for null).
fn bytes(env: Env, a: JByteArray) -> Vec<u8> {
    if a.is_null() {
        return Vec::new();
    }
    // SAFETY: entries 171 and 200 of the table have these types (jni.h); `a` is a live local
    // reference to a byte[] passed by the JVM; the buffer holds exactly `n` bytes.
    unsafe {
        let len: extern "C" fn(Env, JObject) -> JSize = std::mem::transmute(func(env, GET_ARRAY_LENGTH));
        let n = len(env, a).max(0);
        let mut v = vec![0u8; n as usize];
        let get: extern "C" fn(Env, JByteArray, JSize, JSize, *mut u8) = std::mem::transmute(func(env, GET_BYTE_ARRAY_REGION));
        get(env, a, 0, n, v.as_mut_ptr());
        v
    }
}

fn text(env: Env, a: JByteArray) -> String {
    String::from_utf8_lossy(&bytes(env, a)).into_owned()
}

/// A new Java byte array holding `v` (null if it is too large for Java or the JVM is out of memory,
/// with the JVM's exception pending).
fn new_array(env: Env, v: &[u8]) -> JByteArray {
    let Ok(n) = JSize::try_from(v.len()) else { return std::ptr::null_mut() };
    // SAFETY: entries 176, 208 and 228 have these types (jni.h); the new array is `n` bytes long
    // and `v` holds `n` bytes.
    unsafe {
        let new: extern "C" fn(Env, JSize) -> JByteArray = std::mem::transmute(func(env, NEW_BYTE_ARRAY));
        let a = new(env, n);
        let failed: extern "C" fn(Env) -> JBoolean = std::mem::transmute(func(env, EXCEPTION_CHECK));
        if a.is_null() || failed(env) != 0 {
            return std::ptr::null_mut();
        }
        let set: extern "C" fn(Env, JByteArray, JSize, JSize, *const u8) = std::mem::transmute(func(env, SET_BYTE_ARRAY_REGION));
        set(env, a, 0, n, v.as_ptr());
        a
    }
}

/// Runs `f`, turning a panic into `dflt` (a panic must not unwind into the JVM).
fn guard<R>(dflt: R, f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(dflt)
}

/// Native.init(dataDir, saveDir, webDir, modelsDir, onnxRuntimeLibrary), paths as UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_init(env: Env, _cls: JObject, data: JByteArray, save: JByteArray, web: JByteArray, models: JByteArray, ort: JByteArray) {
    let (data, save, web, models, ort) = (text(env, data), text(env, save), text(env, web), text(env, models), text(env, ort));
    guard((), || crate::android::init(data, save, web, models, ort));
}

/// Native.token(): the page's request token.
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_token(env: Env, _cls: JObject) -> JByteArray {
    let t = guard(String::new(), crate::android::token);
    new_array(env, t.as_bytes())
}

/// Native.handle(method, path, query, token or null, body or null) -> the reply (crate::android::handle).
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_handle(env: Env, _cls: JObject, method: JByteArray, path: JByteArray, query: JByteArray, token: JByteArray, body: JByteArray) -> JByteArray {
    let (method, path, query) = (text(env, method), text(env, path), text(env, query));
    let token = if token.is_null() { None } else { Some(text(env, token)) };
    let mut body = bytes(env, body);
    let mut r = guard(crate::android::failure(), || crate::android::handle(&method, &path, &query, token.as_deref(), &body));
    let out = new_array(env, &r);
    // (the document's bytes and the page's text went through these two buffers: overwritten, not only freed)
    crate::ffi::wipe::vec_u8(&mut body);
    crate::ffi::wipe::vec_u8(&mut r);
    out
}

/// Native.takeEvents(): crate::platform::take_events, as UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_takeEvents(env: Env, _cls: JObject) -> JByteArray {
    let e = guard(String::new(), crate::platform::take_events);
    new_array(env, e.as_bytes())
}

/// Native.busy(): whether an MP3 or PDF job is running.
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_busy(_env: Env, _cls: JObject) -> JBoolean {
    guard(0, || crate::platform::busy() as JBoolean)
}

/// Native.shutdown(): stops the jobs (the app is closing).
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_khana_reader_Native_shutdown(_env: Env, _cls: JObject) {
    guard((), crate::android::shutdown);
}
