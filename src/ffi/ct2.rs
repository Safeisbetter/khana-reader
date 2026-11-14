//! CTranslate2 4.5.0 (the translation model) through native/ct2_shim.cpp.

use super::buffer_text;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::ptr::NonNull;

unsafe extern "C" {
    fn ct2_new(path: *const c_char, intra: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn ct2_free(t: *mut c_void);
    fn ct2_translate(t: *mut c_void, tokens: *const c_char, prefix: *const c_char, beam: c_int, max_batch: c_int, max_len: c_int, rep: f32) -> *mut c_char;
    fn ct2_free_str(s: *mut c_char);
    fn wh_new(path: *const c_char, intra: c_int, err: *mut c_char, errlen: c_int) -> *mut c_void;
    fn wh_free(w: *mut c_void);
    fn wh_transcribe(w: *mut c_void, mel: *const f32, n_mels: c_int, frames: c_int, prompt: *const c_char, beam: c_int, max_len: c_int) -> *mut c_char;
}

pub struct Options {
    pub beam: i32,
    pub max_batch: i32,
    pub max_len: i32,
    pub repetition_penalty: f32,
}

/// A loaded translation model. `translate` takes `&mut self`: one translation at a time.
pub struct Translator(NonNull<c_void>);

// SAFETY: the translator is used by one thread at a time (&mut self).
unsafe impl Send for Translator {}

impl Translator {
    pub fn load(dir: &Path, threads: usize) -> Result<Translator, String> {
        let path = CString::new(dir.to_string_lossy().as_bytes()).map_err(|_| "model folder name".to_string())?;
        let mut err = [0 as c_char; 512];
        // SAFETY: `path` lives through the call; `err` is 512 bytes, the length passed.
        let t = unsafe { ct2_new(path.as_ptr(), threads.clamp(1, 64) as c_int, err.as_mut_ptr(), 512) };
        NonNull::new(t).map(Translator).ok_or_else(|| buffer_text(&err))
    }

    /// Token lines (one sentence each, space-separated pieces) -> the best translation of each, in
    /// the same form; None if the model failed.
    pub fn translate(&mut self, lines: &str, target_prefix: &str, o: &Options) -> Option<String> {
        let tokens = CString::new(lines).ok()?;
        let prefix = CString::new(target_prefix).ok()?;
        // SAFETY: self.0 is a loaded model; both strings live through the call.
        let r = unsafe { ct2_translate(self.0.as_ptr(), tokens.as_ptr(), prefix.as_ptr(), o.beam, o.max_batch, o.max_len, o.repetition_penalty) };
        let r = NonNull::new(r)?;
        // SAFETY: the shim returns a NUL-terminated malloc'd string; it is copied, then freed once.
        let out = unsafe { CStr::from_ptr(r.as_ptr()) }.to_string_lossy().into_owned();
        unsafe { ct2_free_str(r.as_ptr()) };
        Some(out)
    }
}

impl Drop for Translator {
    fn drop(&mut self) {
        // SAFETY: owned only by this value, freed exactly once.
        unsafe { ct2_free(self.0.as_ptr()) }
    }
}

fn whisper_shape(len: usize, n_mels: usize, frames: usize) -> Option<(c_int, c_int)> {
    if n_mels == 0 || frames == 0 || n_mels.checked_mul(frames)? != len {
        return None;
    }
    Some((c_int::try_from(n_mels).ok()?, c_int::try_from(frames).ok()?))
}

/// A loaded Whisper model (speech to text). One recognition at a time.
pub struct Whisper(NonNull<c_void>);

// SAFETY: used by one thread at a time (&mut self).
unsafe impl Send for Whisper {}

impl Whisper {
    pub fn load(dir: &Path, threads: usize) -> Result<Whisper, String> {
        let path = CString::new(dir.to_string_lossy().as_bytes()).map_err(|_| "model folder name".to_string())?;
        let mut err = [0 as c_char; 512];
        // SAFETY: `path` lives through the call; `err` is 512 bytes, the length passed.
        let t = unsafe { wh_new(path.as_ptr(), threads.clamp(1, 64) as c_int, err.as_mut_ptr(), 512) };
        NonNull::new(t).map(Whisper).ok_or_else(|| buffer_text(&err))
    }

    /// `mel`: n_mels rows of `frames` values (30 s = 3000 frames); `prompt`: the start tokens separated by spaces.
    /// Returns the best hypothesis' tokens separated by spaces.
    pub fn transcribe(&mut self, mel: &[f32], n_mels: usize, frames: usize, prompt: &str, beam: i32, max_len: i32) -> Option<String> {
        let (n_mels, frames) = whisper_shape(mel.len(), n_mels, frames)?;
        let prompt = CString::new(prompt).ok()?;
        // SAFETY: self.0 is a loaded model; `mel` holds n_mels * frames values, the prompt lives through the call.
        let r = unsafe { wh_transcribe(self.0.as_ptr(), mel.as_ptr(), n_mels, frames, prompt.as_ptr(), beam, max_len) };
        let r = NonNull::new(r)?;
        // SAFETY: the shim returns a NUL-terminated malloc'd string; it is copied, then freed once.
        let out = unsafe { CStr::from_ptr(r.as_ptr()) }.to_string_lossy().into_owned();
        unsafe { ct2_free_str(r.as_ptr()) };
        Some(out)
    }
}

impl Drop for Whisper {
    fn drop(&mut self) {
        // SAFETY: owned only by this value, freed exactly once.
        unsafe { wh_free(self.0.as_ptr()) }
    }
}

