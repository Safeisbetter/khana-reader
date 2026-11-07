
#![deny(unsafe_code)]

pub mod consts;
pub mod difflib;
pub mod doc;
pub mod edits;
pub mod ebook;
pub mod phon;
pub mod pyre;
pub mod spm;
pub mod text;
pub mod vocab;
pub mod g2p;
pub mod normfa;
pub mod onnx;
pub mod audio;
pub mod prosody;
pub mod lexicon;
pub mod asr;
pub mod planner;
pub mod tts;
pub mod mp3;
pub mod stretch;
pub mod app;
#[cfg(target_os = "macos")]
pub mod mac;
pub mod platform;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub mod android;
pub mod engines;
#[allow(unsafe_code)]
pub mod ffi;
pub mod translate;
pub mod ocr;
