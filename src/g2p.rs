
use crate::consts::*;
use crate::normfa::normalize_for_model;
use crate::onnx;
use crate::pyre::*;
use ort::Session;
use std::path::Path;

pub struct OnnxG2P {
    enc: Session,
    dec: Session,
    max_len: usize,
}

fn lookup<'a>(m: &'a [(&str, &str)], k: &str) -> Option<&'a str> {
    m.iter().find(|(a, _)| *a == k).map(|(_, v)| *v)
}

pub fn transliterate_word(w: &str) -> String {
    if strip_chars(w, "-").contains('-') {
        let parts: Vec<&str> = w.split('-').filter(|p| !p.is_empty()).collect();
        if parts.len() > 1 {
            return parts.iter().map(|p| transliterate_word(p)).collect::<Vec<_>>().join(" ");
        }
    }
    let lw = w.to_lowercase();
    if let Some(e) = lookup(G2P_LATIN_EXCEPTIONS, &lw) {
        return e.to_string();
    }
    let n = len(w);
    if is_upper(w) && (2..=5).contains(&n) && w.chars().all(is_alpha) {
        return lw.chars().map(|c| lookup(G2P_LETTER_NAMES, &c.to_string()).unwrap_or("").to_string()).collect::<Vec<_>>().join("\u{200c}");
    }
    let cs: Vec<char> = lw.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let two: String = cs[i..(i + 2).min(cs.len())].iter().collect();
        if let Some(d) = lookup(G2P_LATIN_DIGRAPHS, &two) {
            out.push_str(d);
            i += 2;
        } else {
            out.push_str(lookup(G2P_LATIN_CHAR, &cs[i].to_string()).unwrap_or(""));
            i += 1;
        }
    }
    let sc: Vec<char> = out.chars().collect();
    if sc.len() >= 2 && sc[0] == 'س' && CLUSTER_START.contains(sc[1]) {
        out = format!("ا{out}");
    }
    if out.is_empty() { w.to_string() } else { out }
}

pub fn transliterate_text(text: &str) -> String {
    sub_fn(G2P_LATIN_WORD, text, |c| transliterate_word(&c[0]))
}

fn g2p_windows(text: &str) -> Vec<String> {
    let words = split_ws(text);
    const WINDOW: usize = 30;
    if words.len() <= WINDOW {
        return vec![text.to_string()];
    }
    let mut cut = None;
    for i in WINDOW / 2..(words.len() as isize - 4).min(WINDOW as isize - 3).max(0) as usize {
        if G2P_G2P_CUT_WORDS.contains(&words[i]) {
            cut = Some(i);
        }
    }
    let cut = cut.unwrap_or(WINDOW / 2);
    let mut out = g2p_windows(&words[..cut].join(" "));
    out.extend(g2p_windows(&words[cut..].join(" ")));
    out
}

const SHORT_WINDOW_WORDS: usize = 3;

fn input_pieces(w: &str) -> usize {
    w.split(|c: char| c.is_whitespace() || c == '\u{200c}').filter(|p| p.chars().any(|c| c.is_alphabetic())).count()
}

fn repeated_short(w: &str, out: &str) -> bool {
    let n = input_pieces(w);
    (1..=SHORT_WINDOW_WORDS).contains(&n) && !w.chars().any(|c| c.is_numeric()) && split_ws(out).len() > n
}

fn without_final_marks(w: &str) -> Option<&str> {
    let b = w.trim_end_matches(|c: char| c == '.' || c == '!' || c == '…' || c.is_whitespace());
    (b.len() < w.len() && !b.is_empty()).then_some(b)
}

impl OnnxG2P {
    pub fn new(dir: &Path) -> ort::Result<Self> {
        Ok(OnnxG2P {
            enc: onnx::session(&dir.join("g2p_encoder.onnx"), None)?,
            dec: onnx::session(&dir.join("g2p_decoder.onnx"), None)?,
            max_len: 512,
        })
    }

    fn decode_greedy(&self, text: &str) -> String {
        let ids: Vec<i64> = text.bytes().map(|b| b as i64 + 3).collect();
        if ids.is_empty() {
            return String::new();
        }
        let n = ids.len();
        let enc_out = self.enc.run(ort::inputs!["input_ids" => ort::Tensor::from_array(([1usize, n], ids)).unwrap()].unwrap()).unwrap();
        let (eshape, edata) = enc_out[0].try_extract_raw_tensor::<f32>().unwrap();
        let eshape: Vec<usize> = eshape.iter().map(|x| *x as usize).collect();
        let edata = edata.to_vec();
        let mut dec: Vec<i64> = vec![0];
        for _ in 0..self.max_len {
            let out = self
                .dec
                .run(ort::inputs![
                    "decoder_input_ids" => ort::Tensor::from_array(([1usize, dec.len()], dec.clone())).unwrap(),
                    "encoder_hidden" => ort::Tensor::from_array((eshape.clone(), edata.clone())).unwrap(),
                ].unwrap())
                .unwrap();
            let (shape, logits) = out[0].try_extract_raw_tensor::<f32>().unwrap();
            let v = *shape.last().unwrap() as usize;
            let last = &logits[logits.len() - v..];
            let mut best = 0usize;
            for (i, x) in last.iter().enumerate() {
                if *x > last[best] {
                    best = i;
                }
            }
            if best == 1 {
                break;
            }
            dec.push(best as i64);
        }
        let bytes: Vec<u8> = dec[1..].iter().filter(|&&i| (3..=258).contains(&i)).map(|&i| (i - 3) as u8).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn decode_window(&self, w: &str) -> String {
        let out = self.decode_greedy(w);
        if repeated_short(w, &out) {
            if let Some(bare) = without_final_marks(w) {
                let again = self.decode_greedy(bare);
                if split_ws(&again).len() < split_ws(&out).len() {
                    return again;
                }
            }
        }
        out
    }

    pub fn phonemise(&self, text: &str, keep_ezafe: bool) -> Result<String, String> {
        let t = transliterate_text(text);
        let t = normalize_for_model(&t)?;
        let t = t.replace('؟', "").replace('?', "").replace(':', "");
        let raw = g2p_windows(&t).iter().map(|w| self.decode_window(w)).collect::<Vec<_>>().join(" ");
        let out = translate_map(&raw, TO_PHONEMES);
        let out = split_ws(&out).iter().map(|w| lookup(G2P_PRON_FIX, w).unwrap_or(w).to_string()).collect::<Vec<_>>().join(" ");
        Ok(if keep_ezafe { out } else { out.replace('1', "") })
    }
}
