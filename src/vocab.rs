
use crate::pyre::*;
use crate::spm::SentencePiece;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static MODELS: OnceLock<PathBuf> = OnceLock::new();

pub fn set_models_dir(p: PathBuf) {
    let _ = MODELS.set(p);
}

#[cfg(not(shell))]
pub fn web_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    if let Some(d) = exe.parent() {
        let w = d.join("web");
        if w.is_dir() {
            return w;
        }
    }
    if let Some(c) = exe.parent().and_then(|p| p.parent()) {
        let w = c.join("Resources/web");
        if w.is_dir() {
            return w;
        }
    }
    PathBuf::from(".").join("web")
}

#[cfg(shell)]
pub fn web_dir() -> PathBuf {
    crate::platform::web_dir()
}

pub fn models_dir() -> &'static PathBuf {
    MODELS.get_or_init(|| {
        if let Ok(p) = std::env::var("KHANA_MODELS") {
            return PathBuf::from(p);
        }
        let exe = std::env::current_exe().unwrap_or_default();
        if let Some(d) = exe.parent() {
            let m = d.join("models");
            if m.is_dir() {
                return m;
            }
        }
        if let Some(c) = exe.parent().and_then(|p| p.parent()) {
            let r = c.join("Resources/models");
            if r.is_dir() {
                return r;
            }
        }
        #[cfg(not(shell))]
        return PathBuf::from(".").join("models");
        #[cfg(shell)]
        PathBuf::from("models")
    })
}

pub fn asr_dir() -> PathBuf {
    models_dir().join("asr/vosk-model-small-fa-0.5")
}

pub fn translate_dir() -> PathBuf {
    let dir = models_dir().join("translate");
    if let Ok(name) = std::env::var("KHANA_TRANSLATE_MODEL") {
        return dir.join(name);
    }
    if cfg!(not(shell)) {
        for name in ["nllb-200-3.3B", "nllb-200-1.3B"] {
            if dir.join(name).join("model.bin").is_file() {
                return dir.join(name);
            }
        }
    }
    dir.join("nllb-200-600M")
}

pub fn farsi_words() -> &'static HashSet<String> {
    static W: OnceLock<HashSet<String>> = OnceLock::new();
    W.get_or_init(|| read_fst_symbols(&asr_dir().join("graph/Gr.fst")).unwrap_or_default())
}

fn read_fst_symbols(path: &std::path::Path) -> Option<HashSet<String>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).ok()?;
    let mut d = Vec::new();
    f.by_ref().take(12_000_000).read_to_end(&mut d).ok()?;
    let i32_at = |p: usize| -> Option<i32> { Some(i32::from_le_bytes(d.get(p..p + 4)?.try_into().ok()?)) };
    let i64_at = |p: usize| -> Option<i64> { Some(i64::from_le_bytes(d.get(p..p + 8)?.try_into().ok()?)) };
    let mut p = 4usize;
    for _ in 0..2 {
        p += 4 + i32_at(p)? as usize;
    }
    p += 4 + 4 + 8 + 8 * 3;
    p += 4;
    p += 4 + i32_at(p)? as usize + 8;
    let size = i64_at(p)?;
    p += 8;
    let mut words = HashSet::with_capacity(size as usize);
    for _ in 0..size {
        let n = i32_at(p)? as usize;
        let raw = d.get(p + 4..p + 4 + n)?;
        let s = String::from_utf8_lossy(raw).replace('\u{fffd}', "").replace('\u{200c}', "");
        words.insert(s);
        p += 4 + n + 8;
    }
    Some(words)
}

pub struct WordCost {
    pub cost: f32,
    pub spelling: Option<Box<str>>,
}

pub fn word_costs() -> &'static HashMap<Box<str>, WordCost> {
    static W: OnceLock<HashMap<Box<str>, WordCost>> = OnceLock::new();
    W.get_or_init(|| read_unigram_costs(&asr_dir().join("graph/Gr.fst")).unwrap_or_default())
}

fn read_unigram_costs(path: &std::path::Path) -> Option<HashMap<Box<str>, WordCost>> {
    let d = std::fs::read(path).ok()?;
    let bytes = |p: usize, n: usize| d.get(p..p.checked_add(n)?);
    let i32_at = |p: usize| -> Option<i32> { Some(i32::from_le_bytes(bytes(p, 4)?.try_into().ok()?)) };
    let i64_at = |p: usize| -> Option<i64> { Some(i64::from_le_bytes(bytes(p, 8)?.try_into().ok()?)) };
    let u64_at = |p: usize| -> Option<u64> { Some(u64::from_le_bytes(bytes(p, 8)?.try_into().ok()?)) };
    let f32_at = |p: usize| -> Option<f32> { Some(f32::from_le_bytes(bytes(p, 4)?.try_into().ok()?)) };
    let len_at = |p: usize| -> Option<usize> { usize::try_from(i32_at(p)?).ok() };
    let mut p = 4usize;
    let fst_type = std::str::from_utf8(bytes(p + 4, len_at(p)?)?).ok()?;
    if fst_type != "ngram" {
        return None;
    }
    for _ in 0..2 {
        p += 4 + len_at(p)?;
    }
    let flags = i32_at(p + 4)?;
    p += 4 + 4 + 8 + 8 * 3;
    let mut names: HashMap<i64, String> = HashMap::new();
    for table in 0..2 {
        if flags & (1 << table) == 0 {
            continue;
        }
        p += 4;
        p += 4 + len_at(p)? + 8;
        let size = i64_at(p)?;
        p += 8;
        for _ in 0..size {
            let n = len_at(p)?;
            let key = i64_at(p + 4 + n)?;
            if table == 0 {
                names.insert(key, String::from_utf8_lossy(bytes(p + 4, n)?).replace('\u{fffd}', ""));
            }
            p += 4 + n + 8;
        }
    }
    let ns = usize::try_from(u64_at(p)?).ok()?;
    let nf = usize::try_from(u64_at(p + 8)?).ok()?;
    let nfin = usize::try_from(u64_at(p + 16)?).ok()?;
    p += 24;
    let words64 = |bits: usize| bits.div_ceil(64) * 8;
    p += words64(ns * 2 + 1);
    let future = p;
    p += words64(nf + ns + 1) + words64(ns);
    p += (ns + 1) * 4;
    let future_words = p;
    p += nf * 4 + (ns + 1) * 4 + nfin * 4;
    let future_probs = p;
    if future_probs + (nf + 1) * 4 != d.len() {
        return None;
    }
    let bit = |i: usize| -> Option<bool> { Some((u64_at(future + i / 64 * 8)? >> (i % 64)) & 1 == 1) };
    if bit(0)? {
        return None;
    }
    let mut out: HashMap<Box<str>, WordCost> = HashMap::new();
    let mut i = 0usize;
    while i < nf && bit(1 + i)? {
        let name = names.get(&(i32_at(future_words + i * 4)? as i64));
        let cost = f32_at(future_probs + i * 4)?;
        if let Some(name) = name.filter(|n| !n.is_empty() && !n.starts_with('<') && !n.starts_with('#')) {
            let key = name.replace('\u{200c}', "");
            let spelling = (key != *name).then(|| name.as_str().into());
            let e = out.entry(key.into()).or_insert(WordCost { cost: f32::INFINITY, spelling: None });
            if cost < e.cost {
                *e = WordCost { cost, spelling };
            }
        }
        i += 1;
    }
    Some(out)
}

pub fn in_vocab(w: &str) -> bool {
    farsi_words().contains(&w.replace('\u{200c}', ""))
}


pub fn cmu() -> &'static HashMap<String, Vec<String>> {
    static C: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    C.get_or_init(|| {
        let mut m: HashMap<String, Vec<String>> = HashMap::new();
        let Ok(text) = std::fs::read_to_string(models_dir().join("cmudict.dict")) else { return m };
        for line in text.lines() {
            let line = strip(line);
            let line = line.split('#').next().unwrap_or("");
            let parts = split_ws(line);
            if parts.is_empty() {
                continue;
            }
            let word = sub(r"\(\d+\)$", "", parts[0]);
            m.entry(word).or_insert_with(|| parts[1..].iter().map(|s| s.to_string()).collect());
        }
        m
    })
}

const ARPA: &[(&str, &str)] = &[
    ("AA", "A"), ("AE", "a"), ("AH", "a"), ("AO", "o"), ("AW", "Av"), ("AY", "Ay"), ("EH", "e"), ("ER", "er"),
    ("EY", "ey"), ("IH", "i"), ("IY", "i"), ("OW", "o"), ("OY", "oy"), ("UH", "u"), ("UW", "u"),
    ("B", "b"), ("CH", "C"), ("D", "d"), ("DH", "z"), ("F", "f"), ("G", "g"), ("HH", "h"), ("JH", "j"),
    ("K", "k"), ("L", "l"), ("M", "m"), ("N", "n"), ("NG", "ng"), ("P", "p"), ("R", "r"), ("S", "s"),
    ("SH", "S"), ("T", "t"), ("TH", "s"), ("V", "v"), ("W", "v"), ("Y", "y"), ("Z", "z"), ("ZH", ";"),
];

pub fn english_ph() -> &'static Mutex<HashSet<String>> {
    static E: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    E.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn english_phonemes(word: &str) -> Option<String> {
    let w = word.to_lowercase().replace('’', "'");
    let c = cmu();
    let prons = c.get(&w).or_else(|| c.get(&sub(r"'s$", "", &w)))?;
    let mut out = String::new();
    for ph in prons {
        let base = ph.trim_end_matches(|c| "012".contains(c));
        let stress = &ph[base.len()..];
        let mut sound: String = if base == "AH" && stress == "0" {
            "e".into()
        } else {
            ARPA.iter().find(|(k, _)| *k == base).map(|(_, v)| v.to_string()).unwrap_or_default()
        };
        if out.is_empty() && ["a", "e", "o", "i", "u", "A"].iter().any(|v| sound.starts_with(v)) {
            sound = format!("?{sound}");
        }
        out.push_str(&sound);
    }
    if out.is_empty() {
        return None;
    }
    english_ph().lock().unwrap().insert(out.clone());
    Some(out)
}

pub fn set_source_lang(lang: &str) {
    SOURCE_DUTCH.store(lang == "nl", std::sync::atomic::Ordering::Relaxed);
}
static SOURCE_DUTCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn source_is_dutch() -> bool {
    SOURCE_DUTCH.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn latin_phonemes(word: &str) -> Option<String> {
    if !source_is_dutch() || (word.chars().count() <= 6 && word.chars().all(|c| c.is_uppercase())) {
        return english_phonemes(word);
    }
    let out = dutch_phonemes(word)?;
    english_ph().lock().unwrap().insert(out.clone());
    Some(out)
}

pub fn dutch_phonemes(word: &str) -> Option<String> {
    let w: String = word
        .to_lowercase()
        .replace('’', "'")
        .replace("oû", "oe")
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 's',
            c => c,
        })
        .filter(|c| c.is_ascii_lowercase())
        .collect();
    if w.is_empty() {
        return None;
    }
    match w.as_str() {
        "van" => return Some("fAn".into()),
        "de" => return Some("de".into()),
        "het" => return Some("het".into()),
        "en" => return Some("?en".into()),
        _ => {}
    }
    const VOWELS: &str = "aeiouy";
    const SOUNDS: &[(&str, &str)] = &[
        ("ieuw", "iv"), ("eeuw", "ev"), ("ouw", "Av"), ("auw", "Av"), ("aai", "Ay"), ("ooi", "oy"), ("oei", "uy"), ("sch", "sx"), ("tie", "tsi"),
        ("aa", "A"), ("ae", "A"), ("au", "Av"), ("ou", "Av"), ("ee", "e"), ("ei", "ey"), ("ey", "ey"), ("ij", "ey"), ("eu", "o"), ("ie", "i"), ("oe", "u"),
        ("oo", "o"), ("ui", "oy"), ("uu", "u"), ("ch", "x"), ("ng", "ng"), ("gh", "x"), ("ph", "f"), ("th", "t"), ("tj", "C"), ("sj", "S"), ("dj", "j"), ("qu", "kv"),
        ("ts", "ts"), ("ck", "k"), ("a", "A"), ("b", "b"), ("d", "d"), ("e", "e"), ("f", "f"), ("g", "x"), ("h", "h"), ("i", "i"), ("j", "y"), ("k", "k"),
        ("l", "l"), ("m", "m"), ("n", "n"), ("o", "o"), ("p", "p"), ("q", "k"), ("r", "r"), ("s", "s"), ("t", "t"), ("v", "v"), ("w", "v"), ("x", "ks"), ("z", "z"),
    ];
    let c: Vec<char> = w.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let rest: String = c[i..].iter().collect();
        let n = c.len();
        if rest == "sch" && i > 0 {
            out.push('s');
            break;
        }
        if rest == "lijk" && i > 0 {
            out.push_str("lek");
            break;
        }
        if i > 0 && c[i] == c[i - 1] && !VOWELS.contains(c[i]) {
            i += 1;
            continue;
        }
        let sound = match c[i] {
            'c' => if matches!(c.get(i + 1), Some('e' | 'i' | 'y')) { Some(("c", "s")) } else if rest.starts_with("ch") { None } else { Some(("c", "k")) },
            'y' => Some(("y", if c.get(i + 1).is_some_and(|x| VOWELS.contains(*x)) { "y" } else { "i" })),
            'b' if i + 1 == n => Some(("b", "p")),
            'd' if i + 1 == n && !rest.starts_with("dj") => Some(("d", "t")),
            'u' if !SOUNDS.iter().any(|(s, _)| s.len() > 1 && s.starts_with('u') && rest.starts_with(s)) => {
                let closed = i > 0 && match (c.get(i + 1), c.get(i + 2)) {
                    (Some(x), Some(y)) => !VOWELS.contains(*x) && !VOWELS.contains(*y),
                    (Some(x), None) => !VOWELS.contains(*x),
                    _ => false,
                };
                Some(("u", if closed { "o" } else { "u" }))
            }
            _ => None,
        };
        let (take, ph) = sound.or_else(|| SOUNDS.iter().find(|(s, _)| rest.starts_with(s)).copied())?;
        if out.is_empty() && "aeiouA".contains(&ph[..1]) {
            out.push('?');
        }
        out.push_str(ph);
        i += take.len();
    }
    Some(farsi_onset(&out))
}

pub fn farsi_onset(ph: &str) -> String {
    let c: Vec<char> = ph.chars().collect();
    let consonant = |x: char| !"aeiouAy?".contains(x);
    if c.len() < 3 || !consonant(c[0]) || !consonant(c[1]) {
        return ph.to_string();
    }
    let rest: String = c[1..].iter().collect();
    if c[0] == 's' || c[0] == 'S' {
        return format!("?e{ph}");
    }
    format!("{}e{rest}", c[0])
}


pub fn translator_ready() -> bool {
    translate_dir().join("model.bin").is_file()
}

pub fn nllb_sp() -> Option<&'static SentencePiece> {
    static S: OnceLock<Option<SentencePiece>> = OnceLock::new();
    S.get_or_init(|| {
        if !translator_ready() {
            return None;
        }
        SentencePiece::load(&translate_dir().join("sentencepiece.bpe.model")).ok()
    })
    .as_ref()
}

pub fn pieces(x: &str) -> usize {
    nllb_sp().map(|s| s.encode(x).len()).unwrap_or(0)
}

