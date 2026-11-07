//! The Windows app has no MP3 maker yet (WAV is saved instead): the same names as ffi/lame.rs.

pub struct Encoder(());

impl Encoder {
    pub fn new(_rate: u32, _bitrate_kbps: u32, _quality: i32) -> Result<Encoder, String> {
        Err("ساختن MP3 در ویندوز هنوز ممکن نیست؛ WAV را انتخاب کنید.".into())
    }

    pub fn encode(&mut self, _pcm: &[i16]) -> Vec<u8> {
        Vec::new()
    }

    pub fn flush(&mut self) -> Vec<u8> {
        Vec::new()
    }
}
