
use crate::consts::*;
use crate::lexicon::LexiconG2P;
use crate::normfa::normalize_for_model;
use crate::g2p::transliterate_text;
use crate::pyre::*;
use crate::spm::SentencePiece;
use std::collections::HashMap;

pub const STRONG_GAP: f64 = 0.3;
pub const PHRASE_GAP: f64 = 0.2;
pub const SENTENCE_GAP: f64 = 0.45;
pub const CHUNK_GAP: f64 = 0.12;
const CHUNK_TOKENS: usize = 18;
const CHUNK_HEADROOM: usize = 2;
const TAIL_MERGE_CAP: usize = 19;
const MIN_CHUNK_WORDS: usize = 3;
const MIN_CHUNK_TOKENS: usize = 6;
const RUNT_TAIL_TOKENS: usize = 2;
const LOOKBACK_LIGHTVERB: usize = 5;
const LOOKBACK_PREP: usize = 3;

pub fn split_sentences(text: &str) -> Vec<String> {
    split(SENTENCE_SPLIT, strip(text)).iter().map(|t| strip(t).to_string()).filter(|t| !t.is_empty()).collect()
}

pub fn letter_words(tp: &str) -> Vec<&str> {
    split_ws(tp).into_iter().filter(|w| w.chars().any(is_alnum)).collect()
}

fn split_phrases(sentence: &str) -> Vec<String> {
    split(PHRASE_SPLIT, strip(sentence)).into_iter().filter(|t| !strip(t).is_empty() && !letter_words(t).is_empty()).collect()
}

fn strong_end(p: &str) -> bool {
    STRONG_LEADIN.iter().any(|e| rstrip(p).ends_with(e))
}

fn merge_short_phrases(phrases: Vec<String>, min_words: usize, keep: &mut dyn FnMut(&str) -> bool) -> Vec<String> {
    let mut protected = |p: &str, keep: &mut dyn FnMut(&str) -> bool| strong_end(p) && keep(p);
    let mut out: Vec<String> = Vec::new();
    for p in phrases {
        let merge = match out.last() {
            Some(last) => letter_words(last).len() < min_words && !protected(&last.clone(), keep),
            None => false,
        };
        if merge {
            let l = out.last_mut().unwrap();
            l.push(' ');
            l.push_str(&p);
        } else {
            out.push(p);
        }
    }
    if out.len() >= 2 && letter_words(out.last().unwrap()).len() < min_words && !protected(&out[out.len() - 2].clone(), keep) {
        let last = out.pop().unwrap();
        let l = out.last_mut().unwrap();
        l.push(' ');
        l.push_str(&last);
    }
    out
}

fn split_long_phrase(tp: &str, max_words: usize) -> Vec<String> {
    let words = split_ws(tp);
    if words.len() <= max_words {
        return vec![tp.to_string()];
    }
    let mut cut = None;
    for (i, w) in words.iter().enumerate() {
        if TEXT_PREPS.contains(w) && i >= 4 && words.len() - i >= 3 {
            cut = Some(i);
        }
    }
    match cut {
        None => vec![tp.to_string()],
        Some(c) => {
            let mut out = split_long_phrase(&words[..c].join(" "), max_words);
            out.extend(split_long_phrase(&words[c..].join(" "), max_words));
            out
        }
    }
}

fn merge_leading_light_verbs(phrases: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in phrases {
        let words = letter_words(&p);
        let first = words.first().map(|w| strip_chars(w, "«»()\"'.,;:!?،؛:-").to_string()).unwrap_or_default();
        let key = first.replace('\u{200c}', "");
        if !out.is_empty() && (TEXT_LIGHT_VERBS.contains(&key.as_str()) || key == "را") {
            let l = out.last_mut().unwrap();
            l.push(' ');
            l.push_str(&p);
        } else {
            out.push(p);
        }
    }
    out
}

pub fn plan_phrases(sentence: &str, g2p: &LexiconG2P, tok: &SentencePiece) -> Result<Vec<(String, f64)>, String> {
    let mut cache: HashMap<String, bool> = HashMap::new();
    let mut err: Option<String> = None;
    let mut clean = |tp: &str| -> bool {
        if let Some(v) = cache.get(tp) {
            return *v;
        }
        let want = match normalize_for_model(&transliterate_text(tp)) {
            Ok(n) => letter_words(&n.replace('؟', "").replace('?', "").replace(':', "")).len(),
            Err(e) => {
                err.get_or_insert(e);
                0
            }
        };
        let ph = match g2p.phonemise(tp, true) {
            Ok(p) => p,
            Err(e) => {
                err.get_or_insert(e);
                String::new()
            }
        };
        let v = !ph.is_empty() && split_ws(&ph).len() == want;
        cache.insert(tp.to_string(), v);
        v
    };
    let merged = merge_short_phrases(split_phrases(sentence), 2, &mut clean);
    if let Some(e) = err {
        return Err(e);
    }
    let merged = merge_leading_light_verbs(merged);
    let mut out = Vec::new();
    let mut prev_tp: Option<String> = None;
    for tp in merged {
        let ph_tp = g2p.phonemise(&tp, true)?;
        let parts = if !ph_tp.is_empty() && tok.encode(&ph_tp).len() <= CHUNK_TOKENS + CHUNK_HEADROOM {
            vec![tp.clone()]
        } else {
            split_long_phrase(&tp, 9)
        };
        for q in parts {
            let ph = if q == tp { ph_tp.clone() } else { g2p.phonemise(&q, true)? };
            if ph.is_empty() {
                continue;
            }
            let ph = number_rhythm(&ph);
            let strong = prev_tp.as_deref().map_or(false, strong_end);
            out.push((ph, if strong { STRONG_GAP } else { PHRASE_GAP }));
            prev_tp = Some(q);
        }
    }
    Ok(out)
}

pub fn pack_phrases(plan: Vec<(String, f64)>) -> Vec<(String, f64)> {
    let mut groups: Vec<(String, f64)> = Vec::new();
    for (ph, gap) in plan {
        if !groups.is_empty() && gap != STRONG_GAP {
            let g = groups.last_mut().unwrap();
            g.0 = format!("{} {ph}", g.0);
        } else {
            groups.push((ph, gap));
        }
    }
    groups
}


fn function_word(w: &str) -> bool {
    TTS_FUNCTION_WORDS.contains(&w) || ["Ce", "ham", "na", "?agar", "?amA", "Con", "zirA", "hattA", "?ammA"].contains(&w)
}

fn conjunction(w: &str) -> bool {
    TTS_CONJUNCTIONS.contains(&w) || ["?amA", "?ammA"].contains(&w)
}

fn light_verb(w: &str) -> bool {
    TTS_LIGHT_VERBS.contains(&w)
}

fn number_word(w: &str) -> bool {
    const MORE: &[&str] = &["hejdah", "hijdah", "hefdah", "SAnzdah", "pAnzdah", "yAzdah", "davAzdah", "sizdah", "nuzdah", "CAhArdah", "CahArdah", "CArdah", "devist", "sisad", "CArsad", "CAhArsad", "pAnsad", "SeSsad", "haftsad", "haStsad", "nohsad", "CAr", "CAhAr"];
    w != "?o" && (NUMBER_PH.contains(&w) || MORE.contains(&w))
}

fn number_with_e(w: &str) -> Option<&str> {
    let base = w.strip_suffix('e')?;
    (base.len() >= 3 && base != "dah" && number_word(base)).then_some(base)
}

pub fn number_rhythm(ph: &str) -> String {
    let mut w: Vec<String> = split_ws(ph).iter().map(|s| s.to_string()).collect();
    for i in 0..w.len() {
        if let Some(base) = w[i].strip_suffix("e1") {
            if number_word(base) {
                w[i] = base.to_string();
                continue;
            }
        }
        let after_number = i >= 2 && w[i - 1] == "?o" && number_word(&w[i - 2]);
        if let Some(base) = number_with_e(&w[i]) {
            if base != "CAr" || after_number {
                w[i] = base.to_string();
            }
        }
    }
    for i in 1..w.len() {
        if w[i] == "?o" && number_word(&w[i - 1]) && !w.get(i + 1).is_some_and(|n| number_word(n)) {
            w[i] = "va".to_string();
        }
    }
    w.join(" ")
}

pub fn number_atoms(words: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if number_word(&words[i]) {
            let mut j = i + 1;
            while j + 1 < words.len() && words[j] == "?o" && number_word(&words[j + 1]) {
                j += 2;
            }
            if words.get(j).is_some_and(|n| ["sAl", "sAle", "sAle1", "darsad", "darsade", "darsade1", "safhe", "nafar", "mAh", "ruz", "qarn"].contains(&n.as_str())) {
                j += 1;
            }
            out.push(words[i..j].join(" "));
            i = j;
        } else {
            out.push(words[i].clone());
            i += 1;
        }
    }
    out
}

pub struct Chunker<'a> {
    pub tok: &'a SentencePiece,
}

impl Chunker<'_> {
    fn n(&self, s: &str) -> usize {
        self.tok.encode(s).len()
    }

    fn pack_words(&self, words: &[String], max_tokens: usize, slack: usize) -> Vec<String> {
        let mut chunks: Vec<String> = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        for w in words {
            let mut cand = cur.clone();
            cand.push(w.clone());
            let n = self.n(&cand.join(" "));
            let over = !cur.is_empty() && n > max_tokens + slack;
            let bound = (!cur.is_empty() && cur.last().unwrap().ends_with('1')) || light_verb(w) || w == "rA";
            if over && !(bound && n <= max_tokens + CHUNK_HEADROOM) {
                let mut cut = cur.len();
                let lo = cur.len() as isize - LOOKBACK_LIGHTVERB as isize;
                let mut k = cur.len() as isize - 1;
                while k > lo.max(-1) {
                    if light_verb(&cur[k as usize]) || cur[k as usize] == "rA" {
                        cut = k as usize + 1;
                        break;
                    }
                    k -= 1;
                }
                if cut == cur.len() {
                    let lo = cur.len() as isize - LOOKBACK_PREP as isize;
                    let mut k = cur.len() as isize - 1;
                    while k > lo.max(-1) {
                        if TTS_PREPS.contains(&cur[k as usize].as_str()) && cur.len() - k as usize <= 2 {
                            cut = k as usize;
                            break;
                        }
                        k -= 1;
                    }
                }
                if cut >= MIN_CHUNK_WORDS && self.n(&cur[..cut].join(" ")) >= MIN_CHUNK_TOKENS {
                    chunks.push(cur[..cut].join(" "));
                    cur = cur[cut..].to_vec();
                } else {
                    chunks.push(cur.join(" "));
                    cur = Vec::new();
                }
                cur.push(w.clone());
            } else {
                cur.push(w.clone());
            }
        }
        if !cur.is_empty() {
            chunks.push(cur.join(" "));
        }
        chunks
    }

    fn fix_boundaries(&self, mut chunks: Vec<String>) -> Vec<String> {
        let min_words = 3;
        let mut i = 1;
        while i < chunks.len() {
            let prev: Vec<String> = split_ws(&chunks[i - 1]).iter().map(|s| s.to_string()).collect();
            let nxt: Vec<String> = split_ws(&chunks[i]).iter().map(|s| s.to_string()).collect();
            let last = prev.last().map(|s| s.as_str()).unwrap_or("");
            let left_bound = prev.len() >= 2 && (prev[prev.len() - 2].ends_with('1') || light_verb(last) || last == "rA");
            let first = nxt.first().map(|s| s.as_str());
            let mut bad = false;
            if prev.len() > min_words {
                if last.ends_with('1') {
                    bad = true;
                } else if !left_bound && first.map_or(false, |f| f.ends_with('1')) {
                    bad = true;
                } else if !left_bound && function_word(last) {
                    bad = true;
                } else if !left_bound && first.map_or(false, conjunction) {
                    bad = true;
                } else if !left_bound && last.ends_with('e') && first.map_or(false, |f| f.starts_with('?')) {
                    bad = true;
                }
            }
            if bad && !number_word(last) {
                chunks[i - 1] = prev[..prev.len() - 1].join(" ");
                chunks[i] = format!("{} {}", prev[prev.len() - 1], chunks[i]);
            } else {
                i += 1;
            }
        }
        chunks
    }

    fn enforce_budget(&self, mut chunks: Vec<String>, max_tokens: usize) -> Vec<String> {
        for _ in 0..3 {
            if chunks.iter().all(|c| self.n(c) <= max_tokens + CHUNK_HEADROOM) {
                return chunks;
            }
            let mut repacked = Vec::new();
            for c in &chunks {
                if self.n(c) <= max_tokens + CHUNK_HEADROOM {
                    repacked.push(c.clone());
                } else {
                    let ws = number_atoms(&split_ws(c).iter().map(|s| s.to_string()).collect::<Vec<_>>());
                    repacked.extend(self.pack_words(&ws, max_tokens, CHUNK_HEADROOM));
                }
            }
            chunks = self.fix_boundaries(repacked);
        }
        chunks
    }

    pub fn chunk_phonemes(&self, phonemes: &str) -> Vec<String> {
        let max_tokens = CHUNK_TOKENS;
        let words: Vec<String> = number_atoms(&split_ws(phonemes).iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let pack = |slack: usize| -> Vec<String> {
            let c = self.pack_words(&words, max_tokens, slack);
            let c = self.fix_boundaries(c);
            let mut c = self.enforce_budget(c, max_tokens);
            if c.len() >= 2 {
                let tail = self.n(&c[c.len() - 1]);
                let merged = self.n(&format!("{} {}", c[c.len() - 2], c[c.len() - 1]));
                if tail <= RUNT_TAIL_TOKENS && merged <= TAIL_MERGE_CAP {
                    let last = c.pop().unwrap();
                    let l = c.last_mut().unwrap();
                    *l = format!("{l} {last}");
                }
            }
            c
        };
        let mut chunks = pack(CHUNK_HEADROOM);
        if chunks.len() >= 2 {
            let last = chunks.last().unwrap();
            if letter_words(last).len() < MIN_CHUNK_WORDS || self.n(last) < MIN_CHUNK_TOKENS {
                chunks = pack(0);
            }
        }
        chunks.into_iter().map(|c| c.replace('1', "")).collect()
    }
}

