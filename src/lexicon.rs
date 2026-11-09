
use crate::consts::*;
use crate::difflib::{Op, SequenceMatcher};
use crate::g2p::OnnxG2P;
use crate::phon::{pronounce, pronounce_word, PHONEME_WORD};
use crate::pyre::*;
use crate::text::repair_default;
use crate::vocab::{latin_phonemes, source_is_dutch};
use std::collections::HashMap;
use std::sync::Mutex;

type R = Result<String, String>;

pub fn collapse_loops(ph: &str) -> String {
    let mut words: Vec<String> = split_ws(ph).iter().map(|s| s.to_string()).collect();
    for n in 1..5 {
        let mut i = 0;
        let mut out: Vec<String> = Vec::new();
        while i < words.len() {
            let unit: Vec<String> = words[i..(i + n).min(words.len())].to_vec();
            let mut k = 1;
            while unit.len() == n && words.get(i + k * n..(i + (k + 1) * n).min(words.len())).map_or(false, |s| s == unit.as_slice()) {
                k += 1;
            }
            if k >= 3 && unit.len() == n {
                out.extend(unit);
                i += k * n;
            } else {
                out.push(words[i].clone());
                i += 1;
            }
        }
        words = out;
    }
    words.join(" ")
}

fn skeleton(ph: &str) -> String {
    sub(r"[aeoiuAy?1;\-]", "", &ph.to_lowercase())
}

pub fn core(word: &str) -> String {
    sub(r"^[^\w]+|[^\w‌]+$", "", word).replace('\u{064b}', "")
}

pub fn align_words(words: &[String], phw: &[String]) -> Vec<(usize, usize)> {
    if words.len() == phw.len() {
        return (0..words.len()).map(|i| (i, i)).collect();
    }
    let a: Vec<String> = words.iter().map(|w| skeleton(&pronounce_word(&sub(r"[^\w‌]", "", w)))).collect();
    let b: Vec<String> = phw.iter().map(|p| skeleton(p)).collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut set = |i: usize, j: usize, out: &mut Vec<(usize, usize)>| match out.iter_mut().find(|x| x.0 == i) {
        Some(x) => x.1 = j,
        None => out.push((i, j)),
    };
    for (op, i1, i2, j1, j2) in SequenceMatcher::new(&a, &b, false).get_opcodes() {
        match op {
            Op::Equal => {
                for k in 0..i2 - i1 {
                    set(i1 + k, j1 + k, &mut out);
                }
            }
            Op::Replace => {
                for k in 0..(i2 - i1).min(j2 - j1) {
                    if crate::difflib::ratio_str(&a[i1 + k], &b[j1 + k], true) >= 0.7 {
                        set(i1 + k, j1 + k, &mut out);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn lookup<'a>(m: &'a [(&str, &str)], k: &str) -> Option<&'a str> {
    m.iter().find(|(a, _)| *a == k).map(|(_, v)| *v)
}

const GENERAL_LEXICON: &[(&str, &str)] = &[
    ("قویا", "qaviyyan"),
    ("کلاهبردار", "kolAhbardAr"),
    ("آنلاین", "?Anlayn"),
    ("گنگ", "gong"),
    ("شق", "Seqq"),
    ("سنخ", "senx"),
    ("ترکه", "tarke"),
    ("مترسک", "matarsak"),
    ("کلنگ", "kolang"),
    ("ولگرد", "velgard"),
    ("سنگفرش", "sangfarS"),
    ("سوءظن", "su?ezan"),
    ("چکش", "CakoS"),
    ("انبر", "?anbor"),
    ("عالمه", "?Aleme"),
    ("وازلین", "vAzelin"),
    ("مثل", "mesle"),
    ("باربر", "bArbar"),
    ("شش", "SeS"),
    ("پرتره", "portre"),
    ("شیء", "Sey?"),
    ("منافع", "manAfe?e"),
    ("درباره", "darbAreye"),
    ("نکنند", "nakonand"),
    ("رای", "ra?y"),
    ("کریکت", "kriket"),
    ("قایقرانی", "qAyeqrAni"),
    ("رویارویی", "ruyAruyi"),
    ("مونتاژ", "montA;"),
    ("پروژکتور", "porojektor"),
    ("تألیفی", "ta?lifi"),
    ("تالیفی", "ta?lifi"),
    ("چاره", "CAre"),
    ("انهدام", "?enhedAm"),
    ("وانگهی", "vAngahi"),
    ("درکی", "darki"),
    ("بدوی", "badavi"),
    ("مضحک", "mozahek"),
    ("حول", "havl"),
    ("کردار", "kardAr"),
    ("موطن", "mavtan"),
    ("بنگریم", "bengarim"),
    ("ممارست", "momArast"),
    ("جبلی", "jebelli"),
    ("ادعیه", "?ad?iye"),
    ("اینده", "?Ayande"),
    ("گوتیک", "gotik"),
    ("پژوهش‌گران", "pa;uheSgarAn"),
    ("ماکیان", "mAkiyAn"),
    ("علیرغم", "?alA raqme"),
];

const RUST_LEXICON: &[(&str, &str)] = &[
];

const EZAFE_PRONE: &[&str] = &["ستم", "نوع", "ضلع", "فرع"];
const EZAFE_STOP: &[&str] = &[
    "و", "را", "که", "از", "در", "به", "با", "تا", "یا", "است", "نیست", "بود", "شد", "شده", "می", "نمی", "هم", "نیز", "بر", "بی", "چون", "اما", "ولی", "اگر", "آنکه", "هیچ", "نه", "هر", "چه", "پس", "بلکه",
];

fn ezafe_after_ain(w: &str) -> String {
    sub(r"^(.+?[uoAi])(?:ye1?|e)\?e(1?)$", r"\1?e\2", w)
}


fn mikon_fixed(w: &str) -> String {
    sub(r"^(ne)?mikan(am|i|ad|im|id|and)$", r"\1mikon\2", w)
}


fn has_vowel(p: &str) -> bool {
    p.chars().any(|c| "aeoiuA".contains(c))
}

fn ends_any(s: &str, e: &[&str]) -> bool {
    e.iter().any(|x| s.ends_with(x))
}

pub struct LexiconG2P {
    pub base: OnnxG2P,
    lexicon: Mutex<(Vec<(String, String)>, Option<&'static fancy_regex::Regex>)>,
    plain: Mutex<HashMap<String, String>>,
    memo: Mutex<HashMap<(String, bool), String>>,
}

impl LexiconG2P {
    pub fn new(base: OnnxG2P) -> Self {
        LexiconG2P { base, lexicon: Mutex::new((Vec::new(), None)), plain: Mutex::new(HashMap::new()), memo: Mutex::new(HashMap::new()) }
    }

    fn spelling_phonemes(&self, spelling: &str) -> R {
        let spelling = strip(spelling);
        if spelling.is_empty() {
            return Ok(String::new());
        }
        if search(PHONEME_WORD, &spelling.replace(' ', "")) || search(r"[ً-ْ]", spelling) {
            return Ok(pronounce(spelling));
        }
        if let Some(p) = self.plain.lock().unwrap().get(spelling) {
            return Ok(p.clone());
        }
        let p = self.phonemise_marked(&repair_default(spelling), true)?;
        self.plain.lock().unwrap().insert(spelling.to_string(), p.clone());
        Ok(p)
    }

    pub fn set_lexicon(&self, user: &[(String, String)]) {
        let mut all: Vec<(String, String)> =
            BUILTIN_LEXICON.iter().chain(GENERAL_LEXICON).chain(RUST_LEXICON).map(|(a, b)| (a.to_string(), b.to_string())).collect();
        for (w, s) in user {
            match all.iter_mut().find(|x| &x.0 == w) {
                Some(x) => x.1 = s.clone(),
                None => all.push((w.clone(), s.clone())),
            }
        }
        let mut lex: Vec<(String, String)> = Vec::new();
        for (word, spelling) in all {
            let word = repair_default(&word);
            let ph = self.spelling_phonemes(&spelling).unwrap_or_default();
            if !word.is_empty() && !ph.is_empty() {
                match lex.iter_mut().find(|x| x.0 == word) {
                    Some(x) => x.1 = ph,
                    None => lex.push((word, ph)),
                }
            }
        }
        let mut cur = self.lexicon.lock().unwrap();
        if lex != cur.0 {
            let mut words: Vec<&String> = lex.iter().map(|x| &x.0).collect();
            words.sort_by(|a, b| len(b).cmp(&len(a)));
            let re = if words.is_empty() {
                None
            } else {
                let alt = words.iter().map(|w| fancy_regex::escape(w).into_owned()).collect::<Vec<_>>().join("|");
                Some(rx(&format!("(?<![\\w\u{200c}])({alt})(?![\\w\u{200c}])")))
            };
            *cur = (lex, re);
            self.memo.lock().unwrap().clear();
        }
    }

    pub fn phonemise(&self, text: &str, keep_ezafe: bool) -> R {
        let key = (if source_is_dutch() { format!("nl|{text}") } else { text.to_string() }, keep_ezafe);
        if let Some(p) = self.memo.lock().unwrap().get(&key) {
            return Ok(p.clone());
        }
        let r = self.phonemise_uncached(text, keep_ezafe)?;
        let mut m = self.memo.lock().unwrap();
        if m.len() > 20000 {
            m.clear();
        }
        m.insert(key, r.clone());
        Ok(r)
    }

    fn phonemise_uncached(&self, text: &str, keep_ezafe: bool) -> R {
        let mut text = sub(r"ق\.\s?م\.?", "قبل از میلاد", text);
        text = sub(r"(?<=[\d۰-۹٠-٩]) قم(?![\w])", " قبل از میلاد", &text);
        text = sub(r"(?<=[\d۰-۹٠-٩])\s*ص\.?(?![\w])", " صفحه", &text);
        text = sub(r"(?<![\d۰-۹٠-٩])([\d۰-۹٠-٩]{2,4})\s*[-–]\s*([\d۰-۹٠-٩]{2,4})(?![\d۰-۹٠-٩])", r"\1 تا \2", &text);
        let mut found: Vec<(usize, usize, String)> = Vec::new();
        {
            let lex = self.lexicon.lock().unwrap();
            if let Some(re) = lex.1 {
                for c in re.captures_iter(text.as_str()).flatten() {
                    let m = c.get(0).unwrap();
                    let ph = lex.0.iter().find(|x| x.0 == &c[1]).map(|x| x.1.clone()).unwrap_or_default();
                    found.push((m.start(), m.end(), ph));
                }
            }
        }
        for m in rx(r"(?<![\d۰-۹٠-٩٫٬.,/])[9۹٩](?![\d۰-۹٠-٩٫٬]|[.,/][\d۰-۹٠-٩])").find_iter(text.as_str()).flatten() {
            if !found.iter().any(|(a, b, _)| *a < m.end() && m.start() < *b) {
                found.push((m.start(), m.end(), "noh".into()));
            }
        }
        for m in rx(LATIN_WORD).find_iter(text.as_str()).flatten() {
            if let Some(ph) = latin_phonemes(m.as_str()) {
                if !found.iter().any(|(a, b, _)| *a < m.end() && m.start() < *b) {
                    found.push((m.start(), m.end(), ph));
                }
            }
        }
        if found.is_empty() {
            return self.phonemise_marked(&text, keep_ezafe);
        }
        found.sort();
        let mut parts: Vec<String> = Vec::new();
        let mut pos = 0;
        for (start, end, ph) in found {
            let seg = &text[pos..start];
            if search(r"^[\s،,]*و[\s،,]*$", seg) {
                parts.push("va".into());
            } else if search(r"\w", seg) {
                let span = &text[start..end];
                let mut nxt: Vec<String> = split_ws(&self.phonemise_marked(span, keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                let mut cut: Vec<String> = split_ws(&self.phonemise_marked(&format!("{} {span}", rstrip(seg)), keep_ezafe)?)
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let n_seg = if search(r"[\d۰-۹٠-٩A-Za-z]", seg) { 1 } else { split_ws(seg).iter().filter(|w| search(r"\w", w)).count() };
                if nxt.is_empty() || cut.len() < nxt.len() + n_seg {
                    cut = split_ws(&self.phonemise_marked(&format!("{} او", rstrip(seg)), keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                    nxt = cut.last().cloned().into_iter().collect();
                }
                if span.contains('\u{200c}') && cut.len() > nxt.len() + n_seg {
                    cut = split_ws(&self.phonemise_marked(&format!("{} او", rstrip(seg)), keep_ezafe)?).iter().map(|s| s.to_string()).collect();
                    nxt = cut.last().cloned().into_iter().collect();
                }
                let k = nxt.len();
                if cut.len() >= k + n_seg {
                    parts.push(cut[..cut.len() - k].join(" "));
                } else {
                    parts.push(self.phonemise_marked(seg, keep_ezafe)?);
                }
            }
            parts.push(ph);
            pos = end;
        }
        let tail = &text[pos..];
        if search(r"\w", tail) {
            let lone = split_ws(tail).iter().filter(|w| search(r"\w", w)).count() == 1;
            let after: Vec<String> = if lone {
                split_ws(&self.phonemise_marked(&format!("او {}", tail.trim_start()), keep_ezafe)?).iter().skip(1).map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            };
            if after.len() == 1 {
                parts.push(after[0].clone());
            } else {
                parts.push(self.phonemise_marked(tail, keep_ezafe)?);
            }
        }
        Ok(parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" "))
    }

    fn base_checked(&self, text: &str) -> R {
        let ph = collapse_loops(&self.base.phonemise(text, true)?);
        let words: Vec<&str> = split_ws(text).into_iter().filter(|w| search(r"\w", w)).collect();
        let n = split_ws(&ph).len();
        let nw = words.len();
        if (nw as isize - 1 <= n as isize && n <= nw) || nw < 4 || (n > nw && search(r"[0-9۰-۹٠-٩A-Za-z]", text)) {
            return Ok(ph);
        }
        let mid = nw / 2;
        let cuts: Vec<usize> = (2..nw.saturating_sub(1)).filter(|&k| words[k] == "و").collect();
        let h = cuts.iter().copied().min_by_key(|&k| (k as isize - mid as isize).abs()).unwrap_or(mid);
        let mut left: Vec<String> = if !cuts.is_empty() {
            split_ws(&self.base_checked(&words[..h].join(" "))?).iter().map(|s| s.to_string()).collect()
        } else {
            let mut l: Vec<String> = split_ws(&self.base_checked(&(words[..h].join(" ") + " او"))?).iter().map(|s| s.to_string()).collect();
            l.pop();
            l
        };
        left.extend(split_ws(&self.base_checked(&words[h..].join(" "))?).iter().map(|s| s.to_string()));
        Ok(left.join(" "))
    }

    fn phonemise_marked(&self, text: &str, keep_ezafe: bool) -> R {
        let mut ph = self.base_checked(text)?;
        let words: Vec<String> = split_ws(text).into_iter().filter(|w| search(&format!("[{FA_LETTER}]"), w)).map(|s| s.to_string()).collect();
        let mut phw: Vec<String> = split_ws(&ph).iter().map(|s| s.to_string()).collect();
        if phw.iter().any(|p| len(p) >= 3 && !has_vowel(p)) {
            for (i, j) in align_words(&words, &phw) {
                if len(&phw[j]) >= 3 && !has_vowel(&phw[j]) {
                    let alone: Vec<String> = split_ws(&self.base.phonemise(&words[i], true)?).iter().map(|s| s.to_string()).collect();
                    if alone.len() == 1 && has_vowel(&alone[0]) {
                        let one = if phw[j].ends_with('1') && !alone[0].ends_with('1') { "1" } else { "" };
                        phw[j] = format!("{}{one}", alone[0]);
                    }
                }
            }
            ph = phw.join(" ");
        }
        let in_fix = |w: &str| lookup(WORD_FIX, w).is_some();
        let pair_fix = |a: &str, b: &str| WORD_FIX_PAIRS.iter().find(|((x, y), _)| *x == a && *y == b).map(|(_, v)| *v);
        if words.iter().any(|w| EZAFE_PRONE.contains(&core(w).as_str())) || words.iter().any(|w| search(KASRA_END, w) || w.contains("ساخت") || w.contains("جستار") || in_fix(&core(w))) {
            let aligned = align_words(&words, &phw);
            let has = |k: usize| aligned.iter().find(|x| x.0 == k).map(|x| x.1);
            for &(i, j) in &aligned {
                let key = core(&words[i]);
                let pair = if i + 1 < words.len() { pair_fix(&key, &core(&words[i + 1])) } else { None };
                if let (Some((a, b)), Some(j2)) = (pair, has(i + 1)) {
                    phw[j] = a.to_string();
                    phw[j2] = b.to_string();
                } else if let Some(fix) = lookup(WORD_FIX, &key) {
                    if i > 0 && pair_fix(&core(&words[i - 1]), &key).is_some() {
                        continue;
                    }
                    let ezafe = phw[j].contains('1') && !NAMES.contains(&fix);
                    phw[j] = if ezafe {
                        match lookup(EZAFE_FORM, fix) {
                            Some(e) => e.to_string(),
                            None => format!("{fix}{}", if ends_any(fix, &["A", "u", "i", "o", "e"]) { "ye1" } else { "e1" }),
                        }
                    } else {
                        fix.to_string()
                    };
                }
            }
            for (i, j) in align_words(&words, &phw) {
                if EZAFE_PRONE.contains(&core(&words[i]).as_str()) && !phw[j].ends_with('1') && !phw[j].ends_with('e') {
                    let punct_after = sub(r"[^\w\u200c]+$", "", &words[i]) != words[i];
                    let next_ok = words.get(i + 1).map_or(false, |n| !EZAFE_STOP.contains(&core(n).as_str()) && !core(n).is_empty());
                    if !punct_after && next_ok {
                        phw[j] = format!("{}e1", phw[j]);
                    }
                }
                if search(KASRA_END, &words[i]) && !phw[j].ends_with('1') {
                    let stripped = sub(r"[^\w]|[ً-ْ]", "", &words[i]);
                    let last = stripped.chars().last();
                    let sound = if matches!(last, Some('ا') | Some('و') | Some('ی') | Some('ه')) { "ye" } else { "e" };
                    let p = phw[j].clone();
                    phw[j] = format!("{}1", if p.ends_with(sound) { p } else { format!("{p}{sound}") });
                }
                if words[i].contains("ساخت") {
                    phw[j] = phw[j].replace("soxt", "sAxt");
                }
                if words[i].contains("جستار") {
                    phw[j] = phw[j].replace("jastAr", "jostAr");
                }
            }
            ph = phw.join(" ");
        }
        ph = split_ws(&ph).iter().map(|w| lookup(PHONEME_FIX, w).unwrap_or(w).to_string()).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| mikon_fixed(w)).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| sub(r"^(.+?)e?ye1?\?e?ye(1?)$", r"\1e?e\2", w)).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| sub(r"^(.+e)\?(aS|at|am)$", r"\1\2", &sub(r"^(.+e)\?i$", r"\1yi", w))).collect::<Vec<_>>().join(" ");
        ph = split_ws(&ph).iter().map(|w| ezafe_after_ain(w)).collect::<Vec<_>>().join(" ");
        ph = sub(r"(?<=[^\s?])allAh(?=e?1?\b)", "ollAh", &ph);
        if !text.replace('\u{200c}', "").contains("سه") {
            ph = ph.replace("sh", "S");
        }
        if !search("[زذضظ]ه", &text.replace('\u{200c}', "")) {
            ph = ph.replace("zh", ";");
        }
        Ok(if keep_ezafe { ph } else { ph.replace('1', "") })
    }
}
