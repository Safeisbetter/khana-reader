
use crate::consts::{NAMES, NUMBER_PH};
use crate::difflib::{Op, SequenceMatcher};
use crate::phon::pronounce_word;
use crate::pyre::*;
use crate::vocab::{asr_dir, english_ph};
use std::collections::HashMap;
#[cfg(not(target_os = "ios"))]
use crate::ffi::vosk::SharedModel;
use std::sync::OnceLock;

pub const HEARD_MIN: f64 = 0.75;

#[cfg(not(target_os = "ios"))]
fn model() -> Option<&'static vosk::Model> {
    static M: OnceLock<Option<SharedModel>> = OnceLock::new();
    M.get_or_init(|| {
        let d = asr_dir();
        if !d.is_dir() {
            return None;
        }
        vosk::set_log_level(vosk::LogLevel::Error);
        vosk::Model::new(d.to_string_lossy()).map(SharedModel)
    })
    .as_ref()
    .map(|m| &m.0)
}

#[cfg(target_os = "ios")]
fn model() -> Option<&'static ()> {
    None
}

pub fn ready() -> bool {
    model().is_some()
}

fn skeleton_fa(text: &str) -> String {
    split_ws(text).iter().map(|w| sub(r"[aeoiuAy?\-]", "", &pronounce_word(w))).collect()
}

#[cfg(target_os = "ios")]
pub fn transcribe16(_pcm: &[i16]) -> String {
    String::new()
}

#[cfg(not(target_os = "ios"))]
pub fn transcribe16(pcm: &[i16]) -> String {
    let Some(m) = model() else { return String::new() };
    let Some(mut rec) = vosk::Recognizer::new(m, 16000.0) else { return String::new() };
    let _ = rec.accept_waveform(pcm);
    match rec.final_result() {
        vosk::CompleteResult::Single(s) => s.text.to_string(),
        vosk::CompleteResult::Multiple(m) => m.alternatives.first().map(|a| a.text.to_string()).unwrap_or_default(),
    }
}

pub fn heard_score(phonemes: &str, speech: &[f32], rate: usize) -> (Option<f64>, Vec<String>) {
    let raw: Vec<String> = split_ws(phonemes).iter().map(|w| w.replace('1', "")).collect();
    let eng = english_ph().lock().unwrap().clone();
    let mut pairs: Vec<(String, bool)> = Vec::new();
    for w in &raw {
        let skip = NUMBER_PH.contains(&w.as_str()) || NAMES.contains(&w.as_str()) || eng.contains(w);
        let s = sub(r"[aeoiuAy?1\-]", "", &w.replace("zh", ";"));
        if !s.is_empty() {
            pairs.push((s, skip));
        }
    }
    let words: Vec<&String> = pairs.iter().map(|p| &p.0).collect();
    let want: String = words.iter().map(|s| s.as_str()).collect();
    if model().is_none() || len(&want) < 4 || (speech.len() as f64) < rate as f64 * 0.3 {
        return (None, Vec::new());
    }
    let pcm16 = if rate == 24000 { crate::audio::resample_poly(speech, 2, 3) } else { speech.to_vec() };
    let heard = transcribe16(&crate::audio::to_pcm16(&pcm16));
    let got = skeleton_fa(&heard);
    let wa: Vec<char> = want.chars().collect();
    let ga: Vec<char> = got.chars().collect();
    let mut sm = SequenceMatcher::new(&wa, &ga, false);
    let mut hit = vec![false; wa.len()];
    for (a, _, size) in sm.get_matching_blocks() {
        for h in hit.iter_mut().skip(a).take(size) {
            *h = true;
        }
    }
    let mut problems = Vec::new();
    let mut pos = 0;
    for (k, (w, unchecked)) in pairs.iter().enumerate() {
        let n = len(w);
        let h = &hit[pos..pos + n];
        pos += n;
        if *unchecked {
            continue;
        }
        let mean = h.iter().filter(|x| **x).count() as f64 / n as f64;
        if n >= 2 && mean < 0.34 {
            problems.push(format!("-{w}{}", if k == words.len() - 1 { "$" } else { "" }));
        } else if n >= 3 && (h.iter().filter(|x| !**x).count() >= 2 || !h[0]) {
            problems.push(format!("~{w}"));
        }
    }
    let extra: usize = sm.get_opcodes().iter().filter(|o| o.0 == Op::Insert).map(|o| o.4 - o.3).sum();
    if extra >= 4 {
        problems.push(format!("+{extra}"));
    }
    let mut said: HashMap<&str, usize> = HashMap::new();
    for w in &words {
        *said.entry(w.as_str()).or_default() += 1;
    }
    let mut heard_counts: Vec<(String, usize)> = Vec::new();
    for h in split_ws(&heard) {
        let s = skeleton_fa(h);
        match heard_counts.iter_mut().find(|x| x.0 == s) {
            Some(x) => x.1 += 1,
            None => heard_counts.push((s, 1)),
        }
    }
    for (w, n) in heard_counts {
        let s = *said.get(w.as_str()).unwrap_or(&0);
        if len(&w) >= 2 && 0 < s && s < n {
            problems.push(format!("+{w}"));
        }
    }
    (Some(sm.ratio()), problems)
}

pub fn heard_ok(heard: Option<f64>, problems: &[String]) -> bool {
    let Some(h) = heard else { return true };
    let missing: Vec<&String> = problems.iter().filter(|p| p.starts_with('-') || p.starts_with('~')).collect();
    h >= HEARD_MIN
        && problems.len() == missing.len()
        && missing.len() < 2
        && !missing.iter().any(|p| len(strip_chars(p, "-~$")) >= 3 || p.ends_with('$'))
}

pub fn heard_bad(heard: Option<f64>, problems: &[String]) -> bool {
    let Some(h) = heard else { return false };
    h < 0.6 || problems.iter().any(|p| p.starts_with('+') || p.ends_with('$')) || problems.iter().filter(|p| p.starts_with('-')).count() >= 2
}
