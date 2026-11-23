
use crate::consts::{NAMES, NUMBER_PH};
use crate::difflib::{Op, SequenceMatcher};
use crate::phon::pronounce_word;
use crate::pyre::*;
use crate::vocab::{asr_dir, english_ph};
use std::collections::HashMap;
#[cfg(not(any(target_os = "ios", windows)))]
use crate::ffi::vosk::SharedModel;
use std::sync::OnceLock;

pub const HEARD_MIN: f64 = 0.75;

#[cfg(not(any(target_os = "ios", windows)))]
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

#[cfg(any(target_os = "ios", windows))]
fn model() -> Option<&'static ()> {
    None
}

pub fn ready() -> bool {
    model().is_some()
}

fn skeleton_fa(text: &str) -> String {
    split_ws(text).iter().map(|w| sub(r"[aeoiuAy?\-]", "", &pronounce_word(w))).collect()
}

#[cfg(any(target_os = "ios", windows))]
pub fn transcribe16(_pcm: &[i16], _grammar: Option<&[String]>) -> String {
    String::new()
}

#[cfg(not(any(target_os = "ios", windows)))]
pub fn transcribe16(pcm: &[i16], grammar: Option<&[String]>) -> String {
    let Some(m) = model() else { return String::new() };
    let rec = match grammar {
        Some(g) => vosk::Recognizer::new_with_grammar(m, 16000.0, g),
        None => vosk::Recognizer::new(m, 16000.0),
    };
    let Some(mut rec) = rec else { return String::new() };
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
    let heard = transcribe16(&crate::audio::to_pcm16(&pcm16), None);
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
            problems.push(format!("-{w}{}", if k == words.len() - 1 { "$" } else if k == 0 { "^" } else { "" }));
        } else if n >= 3 && (h.iter().filter(|x| !**x).count() >= 2 || !h[0]) {
            problems.push(format!("~{w}"));
        }
    }
    if let Some((w0, unchecked)) = pairs.first() {
        if !*unchecked && len(w0) == 1 && !hit[0] && !problems.iter().any(|p| p.ends_with('^')) {
            problems.push(format!("-{w0}^"));
        }
    }
    let extra: usize = sm.get_opcodes().iter().filter(|o| o.0 == Op::Insert).map(|o| o.4 - o.3).sum();
    if extra >= 4 {
        problems.push(format!("+{extra}"));
    }
    if sm.get_opcodes().iter().any(|o| o.0 == Op::Insert && o.1 == 0 && o.4 - o.3 >= 2) {
        problems.push("+^".into());
    }
    let mut said: HashMap<&str, usize> = HashMap::new();
    for w in &words {
        *said.entry(w.as_str()).or_default() += 1;
    }
    let mut heard_counts: Vec<(String, usize)> = Vec::new();
    let heard_words: Vec<String> = split_ws(&heard).iter().map(|word| skeleton_fa(word)).collect();
    for s in &heard_words {
        match heard_counts.iter_mut().find(|x| x.0 == *s) {
            Some(x) => x.1 += 1,
            None => heard_counts.push((s.clone(), 1)),
        }
    }
    for (w, n) in heard_counts {
        let s = *said.get(w.as_str()).unwrap_or(&0);
        if len(&w) >= 2 && 0 < s && s < n {
            problems.push(format!("+{w}"));
        }
    }
    for pair in heard_words.windows(2) {
        let whole = pair[0].as_str();
        let ending = pair[1].as_str();
        if looks_like_repeated_ending(whole, ending)
            && said.contains_key(whole)
            && !said.contains_key(ending)
        {
            problems.push(format!("+{ending}"));
        }
    }
    // The recognizer can split a compound into two tokens, then emit the ending again.
    for triple in heard_words.windows(3) {
        let prefix = triple[0].as_str();
        let ending = triple[1].as_str();
        if split_compound_repeat(prefix, ending, &triple[2], &said) {
            problems.push(format!("+{ending}"));
        }
    }
    (Some(sm.ratio()), problems)
}

fn looks_like_repeated_ending(whole: &str, ending: &str) -> bool {
    let ending_len = ending.chars().count();
    ending_len >= 3 && whole.chars().count() >= ending_len + 2 && whole.ends_with(ending)
}

fn split_compound_repeat(prefix: &str, ending: &str, extra: &str, expected: &HashMap<&str, usize>) -> bool {
    prefix.chars().count() >= 2
        && ending.chars().count() >= 3
        && ending == extra
        && !expected.contains_key(ending)
        && expected.contains_key(format!("{prefix}{ending}").as_str())
}

pub fn heard_ok(heard: Option<f64>, problems: &[String]) -> bool {
    let Some(h) = heard else { return true };
    let missing: Vec<&String> = problems.iter().filter(|p| p.starts_with('-') || p.starts_with('~')).collect();
    h >= HEARD_MIN
        && problems.len() == missing.len()
        && missing.len() < 2
        && !missing.iter().any(|p| len(strip_chars(p, "-~$^")) >= 3 || p.ends_with('$') || p.ends_with('^'))
}

pub fn repeated_word_count(problems: &[String]) -> usize {
    problems.iter().filter(|problem| {
        problem.strip_prefix('+').is_some_and(|word| {
            word.chars().count() >= 2 && !word.bytes().all(|byte| byte.is_ascii_digit())
        })
    }).count()
}

pub fn heard_clear_fault(heard: Option<f64>, problems: &[String]) -> bool {
    let Some(h) = heard else { return false };
    h < 0.6
        || problems.iter().any(|p| p.ends_with('^'))
        || repeated_word_count(problems) > 0
        || problems.iter().filter(|p| p.starts_with('-')).count() >= 2
}

pub fn heard_bad(heard: Option<f64>, problems: &[String]) -> bool {
    let Some(h) = heard else { return false };
    h < 0.6 || problems.iter().any(|p| p.starts_with('+') || p.ends_with('$') || p.ends_with('^')) || problems.iter().filter(|p| p.starts_with('-')).count() >= 2
}

#[cfg(test)]
mod tests {
    use super::{heard_clear_fault, looks_like_repeated_ending, split_compound_repeat};

    fn problems(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn repeated_ending_requires_a_distinct_word_prefix() {
        assert!(looks_like_repeated_ending("bndgyr", "gyr"));
        assert!(!looks_like_repeated_ending("gyr", "gyr"));
        assert!(!looks_like_repeated_ending("bndgyr", "yr"));
    }

    #[test]
    fn detects_repeated_ending_when_recognizer_splits_a_compound() {
        let mut expected = std::collections::HashMap::from([("bndgyr", 1)]);
        assert!(split_compound_repeat("bnd", "gyr", "gyr", &expected));
        assert!(!split_compound_repeat("bnd", "gyr", "krd", &expected));
        expected.insert("gyr", 1);
        assert!(!split_compound_repeat("bnd", "gyr", "gyr", &expected));
    }

    #[test]
    fn light_check_rejects_a_recognized_repeated_word() {
        assert!(heard_clear_fault(Some(0.95), &problems(&["+sys"])));
    }

    #[test]
    fn light_check_preserves_existing_extra_audio_tolerance() {
        assert!(!heard_clear_fault(Some(0.95), &problems(&["+4"])));
    }

    #[test]
    fn light_check_still_tolerates_one_small_pronunciation_mismatch() {
        assert!(!heard_clear_fault(Some(0.95), &problems(&["~shklgyr"])));
    }
}
