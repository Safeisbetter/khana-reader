//! The translated PDF, laid out by CoreText (native/typeset.c) on the Mac.

#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn kt_farsi_pdf(recs: *const u8, len: usize, font: *const u8, fontlen: usize, outlen: *mut usize) -> *mut u8;
    fn kt_free(p: *mut c_void);
}

/// One block of the PDF.
pub enum Block<'a> {
    Title(&'a str),
    /// the line under the title
    Subtitle(&'a str),
    Heading(&'a str),
    Paragraph(&'a str),
}

/// The blocks as records: each a kind byte ('T', 'S', 'H', 'P') and its UTF-8 text, separated by
/// 0x1E (on Android the Java shell lays these out, android/java/.../FarsiPdf.java).
pub fn records(blocks: &[Block]) -> Vec<u8> {
    let mut recs = Vec::new();
    for b in blocks {
        let (k, t) = match b {
            Block::Title(t) => (b'T', t),
            Block::Subtitle(t) => (b'S', t),
            Block::Heading(t) => (b'H', t),
            Block::Paragraph(t) => (b'P', t),
        };
        recs.push(k);
        recs.extend(t.bytes().filter(|c| *c != 0x1E));
        recs.push(0x1E);
    }
    recs
}

/// A PDF of the blocks, right to left, in the given font (a font file's bytes; empty: the system's).
#[cfg(target_os = "macos")]
pub fn farsi_pdf(blocks: &[Block], font: &[u8]) -> Result<Vec<u8>, String> {
    let recs = records(blocks);
    let mut len = 0usize;
    // SAFETY: `recs` and `font` live through the call (both read only); `len` is valid for writing.
    let p = unsafe { kt_farsi_pdf(recs.as_ptr(), recs.len(), font.as_ptr(), font.len(), &mut len) };
    let p = NonNull::new(p).ok_or("the PDF was not made")?;
    // SAFETY: the shim returned `len` readable bytes at `p` (malloc'd), copied before the free.
    let v = unsafe { std::slice::from_raw_parts(p.as_ptr(), len) }.to_vec();
    // SAFETY: allocated by the shim with malloc and freed once, by its own free.
    unsafe { kt_free(p.as_ptr().cast()) };
    Ok(v)
}
