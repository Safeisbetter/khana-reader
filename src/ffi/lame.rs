//! LAME 3.100, the MP3 encoder (vendor/lame).

use std::ffi::{c_int, c_short, c_uchar, c_void};
use std::ptr::NonNull;

#[allow(non_camel_case_types)]
type lame_global_flags = c_void;
const MONO: c_int = 3;

unsafe extern "C" {
    fn lame_init() -> *mut lame_global_flags;
    fn lame_set_in_samplerate(g: *mut lame_global_flags, v: c_int) -> c_int;
    fn lame_set_num_channels(g: *mut lame_global_flags, v: c_int) -> c_int;
    fn lame_set_mode(g: *mut lame_global_flags, v: c_int) -> c_int;
    fn lame_set_brate(g: *mut lame_global_flags, v: c_int) -> c_int;
    fn lame_set_quality(g: *mut lame_global_flags, v: c_int) -> c_int;
    fn lame_init_params(g: *mut lame_global_flags) -> c_int;
    fn lame_encode_buffer(g: *mut lame_global_flags, l: *const c_short, r: *const c_short, n: c_int, out: *mut c_uchar, size: c_int) -> c_int;
    fn lame_encode_flush(g: *mut lame_global_flags, out: *mut c_uchar, size: c_int) -> c_int;
    fn lame_close(g: *mut lame_global_flags) -> c_int;
}

/// Samples given to LAME at once: keeps every size it is handed far inside a C int.
const BLOCK: usize = 1 << 20;

/// A mono, constant bit rate encoder.
pub struct Encoder(NonNull<lame_global_flags>);

// SAFETY: the encoder state is used by one thread at a time (&mut self).
unsafe impl Send for Encoder {}

impl Encoder {
    pub fn new(rate: u32, bitrate_kbps: u32, quality: i32) -> Result<Encoder, String> {
        let rate = c_int::try_from(rate).map_err(|_| "MP3 sample rate".to_string())?;
        let brate = c_int::try_from(bitrate_kbps).map_err(|_| "MP3 bit rate".to_string())?;
        // SAFETY: lame_init takes nothing and returns a new state or null.
        let g = NonNull::new(unsafe { lame_init() }).ok_or("MP3 encoder")?;
        let enc = Encoder(g);
        let p = enc.0.as_ptr();
        // SAFETY: `p` is a fresh LAME state; the setters only store integers in it.
        let ok = unsafe {
            lame_set_in_samplerate(p, rate);
            lame_set_num_channels(p, 1);
            lame_set_mode(p, MONO);
            lame_set_brate(p, brate);
            lame_set_quality(p, quality);
            lame_init_params(p) >= 0
        };
        if ok { Ok(enc) } else { Err("MP3 encoder settings".into()) }
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        let mut all = Vec::new();
        for block in pcm.chunks(BLOCK) {
            // LAME's worst case for n samples: 1.25 n + 7200 bytes
            let mut out = vec![0u8; block.len() * 5 / 4 + 7200];
            // SAFETY: `block` holds block.len() samples (passed for both channels of a mono
            // stream) and `out` is out.len() bytes; both lengths fit a C int (BLOCK).
            let n = unsafe {
                lame_encode_buffer(self.0.as_ptr(), block.as_ptr(), block.as_ptr(), block.len() as c_int, out.as_mut_ptr(), out.len() as c_int)
            };
            out.truncate(usize::try_from(n).unwrap_or(0));
            all.extend_from_slice(&out);
        }
        all
    }

    pub fn flush(&mut self) -> Vec<u8> {
        let mut out = vec![0u8; 7200];
        // SAFETY: `out` is out.len() bytes, the size passed.
        let n = unsafe { lame_encode_flush(self.0.as_ptr(), out.as_mut_ptr(), out.len() as c_int) };
        out.truncate(usize::try_from(n).unwrap_or(0));
        out
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the state is owned only by this value and closed exactly once.
        unsafe {
            lame_close(self.0.as_ptr());
        }
    }
}
