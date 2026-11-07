//! Every call into C, C++ and Objective-C code lives here, behind small safe Rust types. The rest
//! of the program is plain safe Rust: lib.rs denies `unsafe` everywhere else, so a new unsafe line
//! anywhere outside this folder does not compile.
//!
//! What is still native, and why (none of it can reach the network):
//! - LAME, the MP3 encoder (vendor/lame);
//! - CTranslate2 (translation) through native/ct2_shim.cpp, and ONNX Runtime (the voice, OCR);
//! - Apple Vision (OCR) through native/vision.m, IOKit (keeping the Mac awake) and CoreText (the
//!   translated PDF, native/typeset.c): Apple's own system frameworks, with no Rust versions.
//! (PDFs are read by pdf.js in the window's sandboxed frame, not here.)
//! - On Android: the Java shell's calls into the app (jni.rs, byte arrays in and out only); there
//!   Apple's frameworks are not used (platform.rs says what the shell does instead).
//!
//! Rules kept in this folder: each `unsafe` block has a SAFETY note; every pointer a library hands
//! back is checked for null, copied into Rust memory and freed with that library's own free; every
//! handle is owned by one Rust value that frees it exactly once (Drop). `wipe` overwrites a book's text and pictures before their
//! memory is given back.

#[cfg(not(windows))]
pub mod ct2;
#[cfg(windows)]
#[path = "ct2_stub.rs"]
pub mod ct2;
#[cfg(target_os = "android")]
pub mod jni;
#[cfg(target_os = "ios")]
pub mod cabi;
#[cfg(not(windows))]
pub mod lame;
#[cfg(windows)]
#[path = "lame_stub.rs"]
pub mod lame;
pub mod onnx;
#[cfg(target_os = "macos")]
pub mod power;
#[cfg(windows)]
#[path = "power_win.rs"]
pub mod power;
pub mod typeset;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod vision;
#[cfg(not(any(target_os = "ios", windows)))]
pub mod vosk;
pub mod wipe;

/// Text from a fixed C buffer the native side filled in (stops at the first NUL, or the buffer's
/// end if there is none), read without trusting the native side to terminate it.
fn buffer_text(buf: &[std::ffi::c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Lowers the whole app's scheduling priority (as `nice 10`), so speech, OCR and translation never
/// make the window or sound playback stutter.
#[cfg(not(windows))]
pub fn lower_priority() {
    // SAFETY: setpriority only reads its three integer arguments.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

/// (Windows: the threads that make speech keep their normal priority.)
#[cfg(windows)]
pub fn lower_priority() {}
