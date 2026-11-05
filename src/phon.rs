
use crate::pyre::*;
use crate::text::{HEH, YEH_KAF};

const FATHA: char = '\u{064e}';
const KASRA: char = '\u{0650}';
const DAMMA: char = '\u{064f}';
const SUKUN: char = '\u{0652}';
const SHADDA: char = '\u{0651}';
const TANWIN: char = '\u{064b}';

fn vowel_mark(c: char) -> Option<&'static str> {
    match c {
        FATHA => Some("a"),
        KASRA => Some("e"),
        DAMMA => Some("o"),
        _ => None,
    }
}

fn consonant(c: char) -> Option<&'static str> {
    Some(match c {
        'ب' => "b", 'پ' => "p", 'ت' => "t", 'ث' => "s", 'ج' => "j", 'چ' => "C", 'ح' => "h", 'خ' => "x",
        'د' => "d", 'ذ' => "z", 'ر' => "r", 'ز' => "z", 'ژ' => ";", 'س' => "s", 'ش' => "S", 'ص' => "s",
        'ض' => "z", 'ط' => "t", 'ظ' => "z", 'ع' => "?", 'غ' => "q", 'ف' => "f", 'ق' => "q", 'ک' => "k",
        'ك' => "k", 'گ' => "g", 'ل' => "l", 'م' => "m", 'ن' => "n", 'ء' => "?", 'أ' => "?", 'إ' => "?",
        'ؤ' => "?", 'ئ' => "?",
        _ => return None,
    })
}

pub const PHONEME_WORD: &str = r"^[A-Za-z?;\-]+$";

pub fn pronounce(spelling: &str) -> String {
    let spelling = strip(spelling);
    if search(PHONEME_WORD, &spelling.replace(' ', "")) {
        return spelling.to_string();
    }
    split_ws(&spelling.replace('\u{200c}', "")).iter().map(|w| pronounce_word(w)).collect::<Vec<_>>().join(" ")
}

fn ends_any(s: &str, e: &[&str]) -> bool {
    e.iter().any(|x| s.ends_with(x))
}

pub fn pronounce_word(word: &str) -> String {
    let word = translate_map(&translate_map(&nfkc(word), YEH_KAF), HEH);
    let mut letters: Vec<(char, String)> = Vec::new();
    for ch in word.chars() {
        if vowel_mark(ch).is_some() || ch == SUKUN || ch == SHADDA || ch == TANWIN {
            if let Some(l) = letters.last_mut() {
                l.1.push(ch);
            }
        } else if ('\u{0600}'..='\u{06ff}').contains(&ch) {
            letters.push((ch, String::new()));
        }
    }
    let mut out = String::new();
    let n = letters.len();
    for i in 0..n {
        let (ch, ref marks) = letters[i];
        let vowel = marks.chars().find_map(vowel_mark).unwrap_or("");
        let first = i == 0;
        let last = i == n - 1;
        let prev_marks = if i > 0 { letters[i - 1].1.as_str() } else { "" };
        let prev_has_vowel = prev_marks.chars().any(|m| vowel_mark(m).is_some()) || ends_any(&out, &["A", "i", "u"]);
        let shadda = marks.contains(SHADDA);
        match ch {
            'آ' => out.push_str("?A"),
            'ا' => {
                if marks.contains(TANWIN) {
                    out.push_str("an");
                } else if first {
                    out.push('?');
                    out.push_str(if vowel.is_empty() { "a" } else { vowel });
                } else {
                    out.push('A');
                    out.push_str(vowel);
                }
            }
            'و' => {
                if first || !vowel.is_empty() || shadda {
                    out.push_str(if shadda { "vv" } else { "v" });
                    out.push_str(vowel);
                } else if out.ends_with('a') && (marks.contains(SUKUN) || (!last && !"اآیو".contains(letters[i + 1].0))) {
                    out.pop();
                    out.push('o');
                } else if prev_marks.contains(DAMMA) {
                } else if prev_has_vowel {
                    out.push('v');
                } else {
                    out.push('u');
                }
            }
            'ی' => {
                if first || !vowel.is_empty() || shadda {
                    out.push_str(if shadda { "yy" } else { "y" });
                    out.push_str(vowel);
                } else if out.ends_with('a') && marks.contains(SUKUN) {
                    out.pop();
                    out.push_str("ey");
                } else if prev_has_vowel && !last {
                    out.push('y');
                } else {
                    out.push('i');
                }
            }
            'ه' => {
                if last && marks.is_empty() && !ends_any(&out, &["A", "i", "u"]) && !out.ends_with('h') {
                    if !out.ends_with('e') {
                        out.push('e');
                    }
                } else {
                    out.push('h');
                    out.push_str(vowel);
                }
            }
            _ => {
                if let Some(c) = consonant(ch) {
                    out.push_str(c);
                    if shadda {
                        out.push_str(c);
                    }
                    out.push_str(vowel);
                }
            }
        }
    }
    out
}
