
use crate::ffi::lame::Encoder;

pub struct Mp3(Encoder);

impl Mp3 {
    pub fn new(rate: u32, bitrate_kbps: u32) -> Result<Mp3, String> {
        Encoder::new(rate, bitrate_kbps, 2).map(Mp3)
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Vec<u8> {
        self.0.encode(pcm)
    }

    pub fn flush(&mut self) -> Vec<u8> {
        self.0.flush()
    }
}
