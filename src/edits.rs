
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Edits {
    pub paras: Vec<(String, Vec<String>)>,
    pub words: Vec<(String, String)>,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\u{200c}'
}

pub fn replace_word(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return text.to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    for (i, m) in text.match_indices(from) {
        let before = text[..i].chars().next_back();
        let after = text[i + m.len()..].chars().next();
        if i < last || before.is_some_and(is_word_char) || after.is_some_and(is_word_char) {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(to);
        last = i + m.len();
    }
    out.push_str(&text[last..]);
    if to.is_empty() {
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        out
    }
}

pub fn count_word(text: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    text.match_indices(word)
        .filter(|(i, m)| {
            !text[..*i].chars().next_back().is_some_and(is_word_char) && !text[i + m.len()..].chars().next().is_some_and(is_word_char)
        })
        .count()
}

pub fn fingerprint(pages: &[Vec<String>]) -> String {
    pages.iter().flatten().filter(|p| !p.trim().is_empty()).flat_map(|p| p.chars()).take(1000).collect()
}

pub fn key_of(name: &str, n_pages: usize, fingerprint: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.bytes().chain(n_pages.to_string().bytes()).chain(fingerprint.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub fn file_for(name: &str, n_pages: usize, fingerprint: &str) -> PathBuf {
    crate::app::data_dir().join("edits").join(format!("{}.json", key_of(name, n_pages, fingerprint)))
}

pub const MAX_PARAGRAPH_CHARS: usize = 100_000;
pub const MAX_WORD_CHARS: usize = 200;
pub const MAX_EDITS: usize = 5_000;
const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;

pub fn global_words_file() -> PathBuf {
    crate::app::data_dir().join("edits").join("all-books-words.json")
}

impl Edits {
    pub fn load_book(path: &Path) -> Edits {
        let mut e = Edits::load(path);
        let g = Edits::load(&global_words_file());
        for (a, b) in g.words {
            e.words.retain(|(x, _)| *x != a);
            e.words.push((a, b));
        }
        e
    }

    pub fn save_book(&self, path: &Path) -> Result<(), String> {
        Edits { paras: self.paras.clone(), words: Vec::new() }.save(path)?;
        Edits { paras: Vec::new(), words: self.words.clone() }.save(&global_words_file())
    }

    pub fn is_empty(&self) -> bool {
        self.paras.is_empty() && self.words.is_empty()
    }

    pub fn load(path: &Path) -> Edits {
        if std::fs::metadata(path).map_or(false, |m| m.len() > MAX_FILE_BYTES) {
            return Edits::default();
        }
        let v: Value = match std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            Some(v) => v,
            None => return Edits::default(),
        };
        let strs = |x: &Value| x.as_str().unwrap_or("").to_string();
        let paras = v["paras"].as_array().map(|a| {
            a.iter().map(|e| (strs(&e[0]), e[1].as_array().map(|l| l.iter().map(strs).collect()).unwrap_or_default())).collect()
        });
        let words = v["words"].as_array().map(|a| a.iter().map(|e| (strs(&e[0]), strs(&e[1]))).collect());
        Edits { paras: paras.unwrap_or_default(), words: words.unwrap_or_default() }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if self.is_empty() {
            let _ = std::fs::remove_file(path);
            return Ok(());
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let v = json!({"paras": self.paras.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
                       "words": self.words.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>()});
        crate::private_fs::atomic_write(path, serde_json::to_string_pretty(&v).unwrap().as_bytes()).map_err(|e| e.to_string())
    }

    pub fn words_applied(&self, p: &str) -> String {
        let mut t = p.to_string();
        for (a, b) in &self.words {
            t = replace_word(&t, a, b);
        }
        t
    }

    pub fn apply(&self, paras: Vec<String>) -> Vec<String> {
        if self.is_empty() {
            return paras;
        }
        let mut out = Vec::new();
        for p in paras {
            if let Some((_, to)) = self.paras.iter().find(|(a, _)| *a == p) {
                out.extend(to.iter().cloned());
            } else {
                let t = self.words_applied(&p);
                if !t.trim().is_empty() {
                    out.push(t);
                }
            }
        }
        out
    }

    pub fn edit_paragraph(&mut self, base: &[String], shown: &str, new: &str) -> bool {
        if new.chars().count() > MAX_PARAGRAPH_CHARS || self.paras.len() >= MAX_EDITS {
            return false;
        }
        let lines: Vec<String> = new.split('\n').map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
        if let Some(e) = self.paras.iter_mut().find(|(_, to)| to.iter().any(|l| l == shown)) {
            let at = e.1.iter().position(|l| l == shown).unwrap();
            e.1.splice(at..=at, lines);
            return true;
        }
        match base.iter().find(|b| self.words_applied(b) == shown) {
            Some(b) => {
                let b = b.clone();
                self.paras.retain(|(a, _)| *a != b);
                self.paras.push((b, lines));
                true
            }
            None => false,
        }
    }

    pub fn set_word(&mut self, from: &str, to: &str) {
        let from = from.trim();
        if from.is_empty() || from.chars().count() > MAX_WORD_CHARS || to.chars().count() > MAX_WORD_CHARS || self.words.len() >= MAX_EDITS {
            return;
        }
        self.words.retain(|(a, _)| a != from);
        self.words.push((from.to_string(), to.trim().to_string()));
    }
}

