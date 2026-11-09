//! The Windows app has no translator yet: the same names as ffi/ct2.rs, and a model that never loads (the page then offers no
//! translation, as in the light Android version).

use std::path::Path;

pub struct Options {
    pub beam: i32,
    pub max_batch: i32,
    pub max_len: i32,
    pub repetition_penalty: f32,
}

pub struct Translator(());

impl Translator {
    pub fn load(_dir: &Path, _threads: usize) -> Result<Translator, String> {
        Err("translation is not in the Windows app yet".into())
    }

    pub fn translate(&mut self, _lines: &str, _target_prefix: &str, _o: &Options) -> Option<String> {
        None
    }
}

pub struct Whisper(());

impl Whisper {
    pub fn load(_dir: &Path, _threads: usize) -> Result<Whisper, String> {
        Err("speech to text is not in this app yet".into())
    }

    pub fn transcribe(&mut self, _mel: &[f32], _n_mels: usize, _frames: usize, _prompt: &str, _beam: i32, _max_len: i32) -> Option<String> {
        None
    }
}
