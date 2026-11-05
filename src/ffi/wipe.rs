//! Overwriting text and pictures before their memory is given back. Freeing memory (Drop) does not erase it: the bytes stay
//! until the memory is used again. These writes are volatile, so the compiler cannot drop them as dead stores.
//!
//! What this covers, and what it cannot: copies the app's own code holds (the open book's text, its translation, a request's
//! bytes). Not the copies in the WebView's page (JavaScript strings, the picture of the page) or in the Java shell's strings,
//! which nothing but their owners can reach. Copies a function made on the way (a clone, a formatted string) are overwritten
//! only where the code says so.

use std::sync::atomic::{compiler_fence, Ordering};

/// Zeros every byte of `buf`.
pub fn bytes(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        // SAFETY: `b` is a valid, aligned, exclusive reference to one initialised byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

/// Zeros the bytes of a text and empties it (its memory is given back with the String).
pub fn string(s: &mut String) {
    // SAFETY: zero bytes are valid UTF-8, so the String stays valid.
    bytes(unsafe { s.as_bytes_mut() });
    s.clear();
}

/// Zeros a buffer and empties it.
pub fn vec_u8(v: &mut Vec<u8>) {
    bytes(v.as_mut_slice());
    v.clear();
}

/// Zeros every text of a list and empties it.
pub fn strings(v: &mut Vec<String>) {
    for s in v.iter_mut() {
        string(s);
    }
    v.clear();
}

/// Zeros every text of a set, which hands out its elements only by moving them out.
pub fn set(s: &mut std::collections::HashSet<String>) {
    for mut w in s.drain() {
        string(&mut w);
    }
}

