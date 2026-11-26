
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

#[cfg(test)]
mod tests {
    use super::Mp3;

    #[test]
    fn encodes_valid_mp3_at_tts_rate() {
        let rate = 24_000u32;
        let mut enc = Mp3::new(rate, 64).expect("lame init at 24kHz 64kbps");
        let pcm: Vec<i16> = (0..rate as usize)
            .map(|i| {
                let t = i as f32 / rate as f32;
                ((t * 440.0 * std::f32::consts::TAU).sin() * 12_000.0) as i16
            })
            .collect();
        let mut out = enc.encode(&pcm);
        out.extend_from_slice(&enc.flush());
        assert!(out.len() >= 6_000 && out.len() <= 14_000, "unexpected mp3 size: {}", out.len());
        let has_sync = out.windows(2).any(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0);
        assert!(has_sync, "encoded output missing MPEG frame sync header");
    }
}
