//! The Vosk speech recognizer's model (the `vosk` crate wraps the C library).

/// The recognizer's model, loaded once and shared by every check.
pub struct SharedModel(pub ::vosk::Model);

// SAFETY: Vosk only reads a loaded model (each check makes its own Recognizer), and its C library
// documents models as safe to share between threads.
unsafe impl Send for SharedModel {}
unsafe impl Sync for SharedModel {}
