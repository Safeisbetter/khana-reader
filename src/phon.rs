
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

/// Returns true if `spelling` is a user-typed Finglish pronunciation (all ASCII letters/spaces/hyphens,
/// and NOT an internal Parsigo uppercase-phoneme code containing `A`, `S`, `C`, `?`, `;`).
pub fn is_plain_finglish_spelling(spelling: &str) -> bool {
    let s = strip(spelling);
    if s.is_empty() {
        return false;
    }
    let has_alpha = s.chars().any(|c| c.is_ascii_alphabetic());
    let all_latin = s.chars().all(|c| c.is_ascii_alphabetic() || matches!(c, ' ' | '-' | '\'' | '’'));
    let has_parsigo_caps = s.chars().any(|c| matches!(c, 'A' | 'S' | 'C' | '?' | ';'));
    has_alpha && all_latin && !has_parsigo_caps
}

/// Heuristic: returns true when a typed Latin text looks like Persian written in Finglish
/// (e.g. "salam chetori man khobam") rather than standard English prose.
pub fn looks_like_finglish(text: &str) -> bool {
    if text.chars().any(|c| ('\u{0600}'..='\u{06ff}').contains(&c)) {
        return false;
    }
    let words: Vec<String> = text
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();
    if words.is_empty() {
        return false;
    }
    const FINGLISH_CUES: &[&str] = &[
        "salam", "chetori", "chetor", "khob", "khoob", "khub", "khobam", "khoobam", "khubam",
        "kheyli", "kheili", "mikham", "mikhaham", "khana", "ketab", "mersi", "mamnoon", "mamnun",
        "khoda", "khodahafez", "emruz", "emrooz", "farda", "dirooz", "diruz", "inja", "onja",
        "oonja", "koja", "chera", "chi", "hast", "hastam", "hasti", "hastim", "nist", "nistam",
        "baraye", "bale", "areh", "doost", "dust", "daram", "dari", "darim", "irani", "farsi",
        "zaban", "zendegi", "omid", "azadi", "mardom", "shab", "rooz", "ruz", "sobh", "bekheir",
        "bekheyr", "lotfan", "bebakhshid", "moshkel", "komak", "khane", "khoone", "madar", "pedar",
        "baradar", "khahar", "bache", "madrese", "daneshgah", "moalem", "ostad", "refigh", "aziz",
        "delam", "shode", "shod", "mikonam", "mikoni", "mikone", "mikonim", "miram", "miri", "mire",
        "miam", "miyam", "behet", "behem", "vali", "chon", "bood", "bud", "shoma", "hame", "yek",
        "chahar", "panj", "shesh", "haft", "hasht", "hezar", "goft", "goftam", "raft", "raftam",
        "kard", "kardam", "khod", "khodam", "dige", "digar", "alan", "hala", "faghat", "hatman",
        "dorost", "ghalat", "bozorg", "koochik", "kuchik", "ziba", "ghashang", "khoobe", "khube",
    ];
    let mut hits = 0usize;
    let mut digraphs = 0usize;
    for w in &words {
        if FINGLISH_CUES.contains(&w.as_str()) {
            hits += 1;
        }
        if w.contains("kh") || w.contains("gh") || w.contains("zh") || w.ends_with("am") || w.ends_with("im") || w.ends_with("id") {
            digraphs += 1;
        }
    }
    hits >= 1 && (hits * 3 >= words.len() || hits >= 2 || digraphs * 2 >= words.len())
}

/// Converts a single Finglish word (Persian written in Latin letters) into Parsigo phonemes.
pub fn finglish_word_to_phonemes(raw: &str) -> String {
    let w = raw.trim().to_lowercase();
    let w: String = w.chars().filter(|c| c.is_ascii_alphabetic() || *c == '\'' || *c == '’').collect();
    if w.is_empty() {
        return String::new();
    }
    const COMMON: &[(&str, &str)] = &[
        ("salam", "salAm"), ("chetori", "Cetori"), ("chetor", "Cetor"),
        ("khob", "xub"), ("khoob", "xub"), ("khub", "xub"),
        ("khobam", "xubam"), ("khoobam", "xubam"), ("khubam", "xubam"),
        ("khobe", "xube"), ("khoobe", "xube"), ("khube", "xube"),
        ("kheyli", "xeyli"), ("kheili", "xeyli"), ("mikham", "mixAm"), ("mikhaham", "mixAham"),
        ("mikhay", "mixAy"), ("mikhastam", "mixAstam"), ("khana", "xAnA"), ("ketab", "ketAb"),
        ("mersi", "mersi"), ("mamnoon", "mamnun"), ("mamnun", "mamnun"),
        ("khoda", "xodA"), ("hafez", "hAfez"), ("khodahafez", "xodAhAfez"),
        ("emruz", "?emruz"), ("emrooz", "?emruz"), ("farda", "fardA"),
        ("dirooz", "diruz"), ("diruz", "diruz"), ("inja", "?injA"),
        ("onja", "?unjA"), ("oonja", "?unjA"), ("anja", "?AnjA"),
        ("koja", "kojA"), ("chera", "CerA"), ("chi", "Ci"), ("che", "Ce"),
        ("hast", "hast"), ("hastam", "hastam"), ("hasti", "hasti"), ("hastim", "hastim"), ("hastand", "hastand"),
        ("nist", "nist"), ("nistam", "nistam"), ("ast", "?ast"),
        ("baraye", "barAye"), ("baray", "barAye"), ("bale", "bale"), ("na", "na"),
        ("are", "?Are"), ("areh", "?Are"), ("doost", "dust"), ("dust", "dust"),
        ("daram", "dAram"), ("dari", "dAri"), ("dare", "dAre"), ("darim", "dArim"), ("darand", "dArand"),
        ("iran", "?irAn"), ("irani", "?irAni"), ("farsi", "fArsi"), ("zaban", "zabAn"),
        ("zendegi", "zendegi"), ("omid", "?omid"), ("azadi", "?AzAdi"), ("mardom", "mardom"),
        ("shab", "Sab"), ("rooz", "ruz"), ("ruz", "ruz"), ("sobh", "sobh"),
        ("bekheir", "bexeyr"), ("bekheyr", "bexeyr"), ("lotfan", "lotfan"),
        ("bebakhshid", "bebaxSid"), ("moshkel", "moSkel"), ("komak", "komak"),
        ("kar", "kAr"), ("khane", "xAne"), ("khoone", "xune"), ("khune", "xune"),
        ("madar", "mAdar"), ("pedar", "pedar"), ("baradar", "barAdar"), ("khahar", "xAhar"),
        ("bache", "baCCe"), ("madrese", "madrese"), ("daneshgah", "dAneSgAh"),
        ("moalem", "mo?allem"), ("ostad", "?ostAd"), ("refigh", "refiq"), ("aziz", "?aziz"),
        ("delam", "delam"), ("tang", "tang"), ("shode", "Sode"), ("shod", "Sod"),
        ("mikonam", "mikonam"), ("mikoni", "mikoni"), ("mikone", "mikone"), ("mikonim", "mikonim"),
        ("miram", "miram"), ("miri", "miri"), ("mire", "mire"), ("miam", "miyAm"), ("miyam", "miyAm"),
        ("behet", "behet"), ("behem", "behem"), ("ba", "bA"), ("be", "be"),
        ("az", "?az"), ("dar", "dar"), ("ke", "ke"), ("va", "va"), ("ya", "yA"),
        ("ama", "?ammA"), ("amma", "?ammA"), ("vali", "vali"), ("agar", "?agar"), ("age", "?age"),
        ("chon", "Con"), ("ta", "tA"), ("ro", "ro"), ("ra", "rA"),
        ("in", "?in"), ("on", "?un"), ("oon", "?un"), ("an", "?An"),
        ("man", "man"), ("to", "to"), ("oo", "?u"), ("u", "?u"), ("ma", "mA"), ("shoma", "SomA"), ("onha", "?unhA"), ("anha", "?AnhA"),
        ("hame", "hame"), ("yek", "yek"), ("ye", "ye"), ("do", "do"), ("se", "se"),
        ("chahar", "CahAr"), ("char", "CAr"), ("panj", "panj"), ("shesh", "SeS"), ("shish", "SiS"),
        ("haft", "haft"), ("hasht", "haSt"), ("noh", "noh"), ("dah", "dah"), ("sad", "sad"), ("hezar", "hezAr"),
        ("bood", "bud"), ("bud", "bud"), ("boodam", "budam"), ("budam", "budam"),
        ("goft", "goft"), ("goftam", "goftam"), ("raft", "raft"), ("raftam", "raftam"),
        ("kard", "kard"), ("kardam", "kardam"), ("khod", "xod"), ("khodam", "xodam"),
        ("dige", "dige"), ("digar", "digar"), ("alan", "?al?An"), ("hala", "hAlA"),
        ("faghat", "faqat"), ("hatman", "hatman"), ("dorost", "dorost"), ("ghalat", "qalat"),
        ("bozorg", "bozorg"), ("koochik", "kuCik"), ("kuchik", "kuCik"), ("ziba", "zibA"),
        ("ghashang", "qaSang"), ("tehran", "tehrAn"), ("shiraz", "SirAz"), ("tabriz", "tabriz"),
        ("esfahan", "?esfahAn"), ("mashhad", "maShad"), ("sistan", "sistAn"), ("baluchestan", "baluCestAn"),
        ("kordestan", "kordestAn"), ("kurdistan", "kordestAn"),
        ("bekhonam", "bexunam"), ("bekhunam", "bexunam"), ("bekhanam", "bexAnam"),
        ("khondan", "xundan"), ("khandan", "xAndan"), ("neveshtan", "neveStan"),
        ("matn", "matn"), ("seda", "sedA"), ("talafoz", "talaffoz"),
    ];
    if let Some((_, ph)) = COMMON.iter().find(|(k, _)| *k == w.as_str()) {
        return (*ph).to_string();
    }
    let b: Vec<char> = w.chars().collect();
    let n = b.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        let c = b[i];
        let next = b.get(i + 1).copied();
        let next2 = b.get(i + 2).copied();
        if i == 0 && matches!(c, 'a' | 'e' | 'i' | 'o' | 'u') {
            out.push('?');
        }
        match (c, next) {
            ('k', Some('h')) => { out.push('x'); i += 2; }
            ('g', Some('h')) => { out.push('q'); i += 2; }
            ('s', Some('h')) => { out.push('S'); i += 2; }
            ('c', Some('h')) => { out.push('C'); i += 2; }
            ('z', Some('h')) => { out.push(';'); i += 2; }
            ('p', Some('h')) => { out.push('f'); i += 2; }
            ('o', Some('o')) | ('o', Some('u')) => { out.push('u'); i += 2; }
            ('e', Some('e')) => { out.push('i'); i += 2; }
            ('a', Some('a')) => { out.push('A'); i += 2; }
            ('e', Some('y')) | ('e', Some('i')) => { out.push_str("ey"); i += 2; }
            ('o', Some('w')) => { out.push('o'); i += 2; }
            ('a', Some('y')) if next2.is_none() || !matches!(next2, Some('a' | 'e' | 'i' | 'o' | 'u')) => {
                out.push_str("Ay"); i += 2;
            }
            ('\'' | '’', _) => { out.push('?'); i += 1; }
            ('a', _) => {
                // Word-final 'a' in Persian Finglish is almost always long /A/ (e.g. "baba", "khana", "farda")
                if i + 1 == n { out.push('A'); } else { out.push('a'); }
                i += 1;
            }
            ('e', _) => { out.push('e'); i += 1; }
            ('i', _) => { out.push('i'); i += 1; }
            ('o', _) => { out.push('o'); i += 1; }
            ('u', _) => { out.push('u'); i += 1; }
            ('w', _) => { out.push('v'); i += 1; }
            ('c', _) => {
                if matches!(next, Some('e' | 'i' | 'y')) { out.push('s'); } else { out.push('k'); }
                i += 1;
            }
            ('x', _) => { out.push_str("ks"); i += 1; }
            (ch, _) if ch.is_ascii_alphabetic() => { out.push(ch); i += 1; }
            _ => { i += 1; }
        }
    }
    out
}

/// Converts a Finglish phrase (one or more words) to space-separated Parsigo phonemes.
pub fn finglish_to_phonemes(text: &str) -> String {
    text.split_whitespace()
        .map(finglish_word_to_phonemes)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Merges user-placed Persian vowel marks (`َ ِ ُ ْ ّ`) onto a base G2P phoneme string so that
/// marking just 1–2 letters (e.g. "بُرش" or "مُبتنی" or "اَشکال") overrides ONLY those letters'
/// vowels/gemination while keeping all unmarked letters' natural Persian vowels intact!
pub fn apply_user_marks(marked_word: &str, base_ph: &str) -> String {
    let word = translate_map(&translate_map(&nfkc(marked_word), YEH_KAF), HEH);
    let mut letters: Vec<(char, String)> = Vec::new();
    for ch in word.chars() {
        if vowel_mark(ch).is_some() || ch == SUKUN || ch == SHADDA || ch == TANWIN {
            if let Some(l) = letters.last_mut() {
                l.1.push(ch);
            }
        } else if ('\u{0600}'..='\u{06ff}').contains(&ch) && ch != '\u{200c}' {
            letters.push((ch, String::new()));
        }
    }
    if letters.is_empty() || base_ph.is_empty() {
        return pronounce_word(marked_word);
    }
    // Count how many consonants carry explicit marks; if user marked almost every consonant, trust pronounce_word directly.
    let cons_count = letters.iter().filter(|(c, _)| consonant(*c).is_some() || *c == 'ا').count();
    let marked_count = letters.iter().filter(|(_, m)| !m.is_empty()).count();
    if cons_count > 0 && marked_count * 4 >= cons_count * 3 && cons_count <= 4 {
        return pronounce_word(marked_word);
    }
    if letters.len() == 3
        && letters[0].1.chars().any(|m| vowel_mark(m).is_some())
        && letters[1].1.is_empty()
        && letters[2].1.is_empty()
        && !letters.iter().skip(1).any(|(c, _)| "اآوی".contains(*c))
        && letters[2].0 != 'ه'
    {
        return pronounce_word(marked_word);
    }
    // Walk through `base_ph`, matching each consonant slot to `letters` and applying explicit marks!
    let ph_chars: Vec<char> = base_ph.chars().collect();
    let is_v = |c: char| matches!(c, 'a' | 'e' | 'o' | 'i' | 'u' | 'A');
    let mut out = String::new();
    let mut p = 0usize;
    for (idx, (ch, marks)) in letters.iter().enumerate() {
        let want_v = marks.chars().find_map(vowel_mark);
        let want_sukun = marks.contains(SUKUN);
        let want_shadda = marks.contains(SHADDA);
        let want_tanwin = marks.contains(TANWIN);
        // Handle initial alef ('ا') with explicit vowel mark (e.g. اَشکال -> ?aSkAl, اِشکال -> ?eSkAl)
        if idx == 0 && *ch == 'ا' {
            if p < ph_chars.len() && ph_chars[p] == '?' {
                out.push('?');
                p += 1;
            } else {
                out.push('?');
            }
            if p < ph_chars.len() && is_v(ph_chars[p]) {
                if let Some(v) = want_v {
                    out.push_str(v);
                } else {
                    out.push(ph_chars[p]);
                }
                p += 1;
            } else if let Some(v) = want_v {
                out.push_str(v);
            }
            continue;
        }
        if *ch == 'ا' && want_tanwin {
            // Skip any existing vowel/an in base_ph and emit "an"
            while p < ph_chars.len() && (is_v(ph_chars[p]) || ph_chars[p] == 'n') {
                p += 1;
            }
            out.push_str("an");
            continue;
        }
        let Some(target_c) = consonant(*ch).or(match *ch { 'ه' => Some("h"), 'و' => Some("v"), 'ی' => Some("y"), _ => None }) else {
            continue;
        };
        let tc = target_c.chars().next().unwrap();
        // Advance in base_ph until we reach this consonant (copying intervening vowels/glides untouched)
        if let Some(rel) = ph_chars[p..].iter().position(|&x| x == tc) {
            if rel <= 3 {
                for &x in &ph_chars[p..p + rel] {
                    out.push(x);
                }
                p += rel;
                // Consume consonant (and existing geminate if present)
                out.push(ph_chars[p]);
                p += 1;
                let had_gem = p < ph_chars.len() && ph_chars[p] == tc;
                if had_gem {
                    out.push(ph_chars[p]);
                    p += 1;
                } else if want_shadda {
                    out.push(tc);
                }
                if want_v.is_some() || want_sukun {
                    // Replace/remove short vowel (a/e/o) right after this consonant
                    if p < ph_chars.len() && matches!(ph_chars[p], 'a' | 'e' | 'o') {
                        p += 1;
                    }
                    if let Some(v) = want_v {
                        out.push_str(v);
                    }
                }
            }
        }
    }
    while p < ph_chars.len() {
        out.push(ph_chars[p]);
        p += 1;
    }
    if out.is_empty() { pronounce_word(marked_word) } else { out }
}
