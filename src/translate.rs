
use crate::consts::ABBREV;
use crate::pyre::*;
use crate::vocab::{nllb_sp, translate_dir, translator_ready};
use crate::ffi::ct2::{Options, Translator};
use std::sync::{Mutex, OnceLock};

struct Loaded {
    t: Option<Translator>,
    used: std::time::Instant,
    failed: bool,
}

fn slot() -> &'static Mutex<Loaded> {
    static M: OnceLock<Mutex<Loaded>> = OnceLock::new();
    M.get_or_init(|| {
        let idle = std::env::var("KHANA_MT_IDLE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(300u64);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(idle.min(60)));
            let mut l = slot().lock().unwrap_or_else(|e| e.into_inner());
            if l.t.is_some() && l.used.elapsed() > std::time::Duration::from_secs(idle) {
                l.t = None;
            }
        });
        Mutex::new(Loaded { t: None, used: std::time::Instant::now(), failed: false })
    })
}

fn with_model<R>(f: impl FnOnce(&mut Translator) -> R) -> Option<R> {
    if !translator_ready() {
        return None;
    }
    let mut l = slot().lock().unwrap_or_else(|e| e.into_inner());
    if l.t.is_none() && !l.failed {
        let threads = (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) * 3 / 5).clamp(1, 6);
        match Translator::load(&translate_dir(), threads) {
            Ok(t) => l.t = Some(t),
            Err(e) => {
                eprintln!("translation model: {e}");
                l.failed = true;
            }
        }
    }
    let r = l.t.as_mut().map(f);
    l.used = std::time::Instant::now();
    r
}

pub fn ready() -> bool {
    translator_ready()
}

pub fn split_source_sentences(text: &str) -> Vec<String> {
    let parts: Vec<String> = split(r#"(?<=[.!?])\s+(?=["“‘«„(\[]?[\p{Lu}0-9])"#, text)
        .iter()
        .flat_map(|p| split(r#"(?<=[.!?]["”’»)\]])\s+(?=["“‘«„(\[]?[\p{Lu}0-9])"#, p))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for p in parts {
        if out.last().map_or(false, |l| search(ABBREV, l) || search(r"(?:(?:^|\s)[A-Z]|\.[A-Z][a-z]?)\.$", l)) {
            let l = out.last_mut().unwrap();
            l.push(' ');
            l.push_str(&p);
        } else {
            out.push(p);
        }
    }
    let mut fin = Vec::new();
    for sent in out {
        let mut sent: Vec<char> = sent.chars().collect();
        while sent.len() > 600 {
            let head: String = sent[..600].iter().collect();
            let rfind = |pat: &str| head.rfind(pat).map(|b| head[..b].chars().count() as isize).unwrap_or(-1);
            let cut = rfind("; ").max(rfind(": ")).max(rfind(", "));
            let cut = if cut > 200 { cut as usize + 1 } else { 600 };
            fin.push(strip(&sent[..cut].iter().collect::<String>()).to_string());
            sent = strip(&sent[cut..].iter().collect::<String>()).chars().collect();
        }
        let s: String = sent.iter().collect();
        if !strip(&s).is_empty() {
            fin.push(strip(&s).to_string());
        }
    }
    fin
}

pub fn modern_dutch(s: &str) -> String {
    const WORDS: &[(&str, &str)] = &[
        (r"\b([Ee])ene?r?n?\b", "\\1en"), (r"\b([Hh])unne[rn]?\b", "\\1un"), (r"\b([Dd])ezer\b", "van deze"), (r"\b([Zz])ijner\b", "van zijn"),
        (r"\b[Hh]aar\s+(?=eigen)", "haar "), (r"\bden\b", "de"), (r"\bDen\b", "De"), (r"\bder\b", "van de"), (r"\bDer\b", "Van de"), (r"\bdes\b", "van de"), (r"\bten\s+deele\b", "gedeeltelijk"),
        (r"\b[Gg]eene?\b", "geen"), (r"\bwelker\b", "waarvan"), (r"\bwien\b", "wie"), (r"\bgij\b", "jullie"), (r"\bGij\b", "Jullie"), (r"\buwe?\b", "jullie"),
        (r"\bthans\b", "nu"), (r"\bThans\b", "Nu"), (r"\breeds\b", "al"), (r"\bevenwel\b", "echter"), (r"\bweder\b", "weer"), (r"\bdoch\b", "maar"), (r"\bDoch\b", "Maar"), (r"\bvan zelf\b", "vanzelf"), (r"\bop nieuw\b", "opnieuw"),
        (r"\bschacher\b", "handel"), (r"\bpractijk\b", "praktijk"), (r"\blitteratu", "literatu"), (r"\bekonomi", "economi"), (r"\bcommunisten\b", "communisten"), (r"\bgemeensoldaten\b", "gewone soldaten"),
    ];
    let mut t = sub_fn(r"^\s*[A-Z](?: [A-Z])+\.?\s*$", s, |m| m[0].replace(' ', ""));
    for (a, b) in WORDS {
        t = sub(a, b, &t);
    }
    t = sub(r"\b([Zz])oo(?!n\b|ns\b|m|i)", "\\1o", &t);
    t = sub(r"([a-hj-z])sch(e|en|er|es|land)?\b", "\\1s\\2", &t);
    t = sub(r"\b([Tt])usschen\b", "\\1ussen", &t);
    t = sub_fn(r"(oo|ee)(?=[bdfgklmnprstvz][aeiou])", &t, |m| m[1][..1].to_string());
    sub(r"ieel(e|en)\b", "iël\\1", &t)
}

fn tidy_source(s: &str) -> String {
    let s = translate_map(s, &[('‘', "'"), ('’', "'"), ('‚', "'"), ('‛', "'"), ('“', "\""), ('”', "\""), ('„', "\""), ('«', "\""), ('»', "\"")]);
    let s = sub(r"(\d)\s*[–—]\s*(\d)", r"\1-\2", &s);
    let s = sub(r"\s*[–—]+\s*", ", ", &s);
    let mut s = strip_chars(&sub(r",\s*([,.;:!?])", r"\1", &s), ", ").to_string();
    s = sub(r#"(^|[\s(])["']([\w-]+(?: [\w-]+){0,2})["'](?=[\s.,;:!?)]|$)"#, r"\1\2", &s);
    for (a, b) in SOURCE_FIXES {
        s = sub(a, b, &s);
    }
    s
}

const SOURCE_FIXES: &[(&str, &str)] = &[
    (r"\s*\(ISBN[^)]*\)", ""),
    (r"\s*\bE-?mail:\s*[\w.+-]+@[\w-]+\.[\w.]+", ""),
    (r"\s*\(\s*\)", ""),
    (r"\s*\bE-?mail:\s*$", ""),
];

const FA_FIXES: &[(&str, &str)] = &[
    (r"تنها[\x{200c} ]ترین", "تنهاترین"),
    (r"ایده آل", "ایده\u{200c}آل"),
    ("هفته نامه", "هفته\u{200c}نامه"),
    (r"\bقطعانه", "قاطعانه"),
    (r"\bمیکنه\b", "می\u{200c}کند"),
    (r"کار کنه\b", "کار کند"),
    (r"\bداره\b", "دارد"),
    (r"\bنداره\b", "ندارد"),
    (r"\bمیشه\b", "می‌شود"),
    (r"\bنمیشه\b", "نمی‌شود"),
    (r"\bبشه\b", "بشود"),
    (r"\bمی[\x{200c} ]?تونه\b", "می‌تواند"),
    (r"\bمی[\x{200c} ]?تونن\b", "می‌توانند"),
    (r"\bنمی[\x{200c} ]?تونه\b", "نمی‌تواند"),
    (r"\bمیاد\b", "می‌آید"),
    (r"\bمیده\b", "می‌دهد"),
    (r"\bمی[\x{200c} ]?کنه\b", "می‌کند"),
    (r"\bمی[\x{200c} ]?خوایم\b", "می‌خواهیم"),
    (r"\bبگیره\b", "بگیرد"),
    (r"\bبگیرن\b", "بگیرند"),
    (r"\bباشن\b", "باشند"),
    (r"\bبدید\b", "بدهید"),
    (r"\bازش\b", "از آن"),
    (r"\bهمونطور\b", "همان‌طور"),
    (r"\bخودش رو\b", "خودش را"),
    (r"(\S)شون\b", "\\1شان"),
    (r"\bمیکنید\b", "می‌کنید"),
    (r"\bمیکرد\b", "می‌کرد"),
    (r"\bمیسازد\b", "می‌سازد"),
    (r"\bمیروند\b", "می‌روند"),
    (r"\bاصلا\b", "اصلاً"),
    (r"\bمعمولا\b", "معمولاً"),
    (r"\bقطعا\b", "قطعاً"),
    (r"\bصریحا\b", "صریحاً"),
    (r"\bبعدا\b", "بعداً"),
    (r"\bمیخوایم\b", "می‌خواهیم"),
    (r"\bحاصلات\b", "دستاوردهای"),
];

const TERM_FIXES: &[(&str, &str, &str)] = &[
];

fn collapse_echo(src: &str, fa: &str) -> String {
    let words: Vec<String> = findall(r"[\p{L}\p{N}]+", src).iter().map(|w| w.to_lowercase()).collect();
    if words.is_empty() || words.len() > 2 || (words.len() == 2 && words[0] == words[1]) {
        return fa.to_string();
    }
    let toks = findall(r"[\x{0621}-\x{064A}\x{0671}-\x{06D3}\x{200c}]+", fa);
    let n = toks.len();
    if n < 2 || n % 2 != 0 || n > 6 || toks[..n / 2] != toks[n / 2..] {
        return fa.to_string();
    }
    let mark = match src.trim_end().chars().last() {
        Some('?') | Some('؟') => "؟",
        Some('!') => "!",
        Some('.') => ".",
        Some('…') => "…",
        _ => "",
    };
    format!("{}{mark}", toks[..n / 2].join(" "))
}

fn tidy_farsi(src: &str, s: &str) -> String {
    let once = collapse_echo(src, s);
    let s = once.as_str();
    match strip(src) {
        "a." | "A." => return "الف.".to_string(),
        "b." | "B." => return "ب.".to_string(),
        "c." | "C." => return "ج.".to_string(),
        _ => {}
    }
    if fullmatch(r"\s*\d{1,3}\.\s*", src) {
        return strip(src).chars().map(|c| if c.is_ascii_digit() { char::from_u32('۰' as u32 + (c as u32 - '0' as u32)).unwrap() } else { c }).collect();
    }
    let mut s = translate_map(&sub(r"\s*<unk>\s*", " ", s), &[('ي', "ی"), ('ك', "ک")]);
    for (a, b) in FA_FIXES {
        s = sub(a, b, &s);
    }
    s = sub(r"(?<=[\x{0600}-\x{06FF}]ه) (اند|ایم|اید|ام|ای)(?![\x{0600}-\x{06FF}])", "\u{200c}\\1", &s);
    for (en, a, b) in TERM_FIXES {
        if search(en, src) {
            s = sub(a, b, &s);
        }
    }
    let s = strip(&roman_in_text(&s)).to_string();
    let s = sub_fn(r"(?<![A-Za-z0-9.])[0-9]+(?:[.][0-9]+)*(?![A-Za-z0-9])", &s, |c| {
        c[0].chars().map(|ch| if ch.is_ascii_digit() { char::from_u32('۰' as u32 + (ch as u32 - '0' as u32)).unwrap() } else { ch }).collect()
    });
    if search(r#"[.!?][’”"')]*$"#, strip(src)) && search(r"[\p{L}\d]$", &s) {
        return s + ".";
    }
    s
}

fn roman_value(t: &str) -> Option<i32> {
    if t.is_empty() || t.len() > 7 || !t.chars().all(|c| "IVXLC".contains(c)) {
        return None;
    }
    let val = |c| match c { 'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50, _ => 100 };
    let v: Vec<i32> = t.chars().map(val).collect();
    Some((0..v.len()).map(|i| if i + 1 < v.len() && v[i] < v[i + 1] { -v[i] } else { v[i] }).sum())
}

fn fa_number(n: i32) -> String {
    n.to_string().chars().map(|d| char::from_u32('۰' as u32 + d.to_digit(10).unwrap_or(0)).unwrap_or(d)).collect()
}

fn roman_heading(p: &str) -> Option<String> {
    roman_value(strip_chars(strip(p), ".")).map(fa_number)
}

fn roman_in_text(s: &str) -> String {
    sub_fn(r"\b[IVX]{2,6}\b", s, |c| {
        let t = g(c, 0).unwrap_or("");
        match roman_value(t) {
            Some(n) if fullmatch(r"X{0,3}(IX|IV|V?I{0,3})", t) => fa_number(n),
            _ => t.to_string(),
        }
    })
}

fn model_words(sentences: &[String], lang: &str) -> Option<Vec<String>> {
    if sentences.is_empty() {
        return Some(Vec::new());
    }
    let sp = nllb_sp()?;
    let src = crate::text::nllb_code(lang);
    let lines: Vec<String> = sentences
        .iter()
        .map(|s| {
            let mut t = vec![src.to_string()];
            let s = if lang == "nl" { modern_dutch(s) } else { s.clone() };
            t.extend(sp.encode_pieces(&tidy_source(&s)));
            t.push("</s>".into());
            t.join(" ")
        })
        .collect();
    let opts = Options { beam: 3, max_batch: 16, max_len: 300, repetition_penalty: 1.1 };
    let out = with_model(|m| m.translate(&lines.join("\n"), "pes_Arab", &opts))??;
    Some(
        out.split('\n')
            .take(sentences.len())
            .map(|h| {
                let toks: Vec<String> = h.split(' ').filter(|t| !t.is_empty() && *t != "pes_Arab" && *t != "</s>").map(|t| t.to_string()).collect();
                sp.decode_pieces(&toks)
            })
            .collect(),
    )
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphabetic())).count()
}

fn lost_words(src: &str, fa: &str) -> bool {
    let n = word_count(src);
    n >= 8 && (word_count(fa) as f32) < 0.6 * n as f32
}

fn two_clauses(s: &str) -> Option<(String, String, &'static str)> {
    let mid = s.len() / 2;
    let side_ok = |b: usize, w: usize| word_count(&s[..b]) >= 3 && word_count(&s[b + w..]) >= 3;
    let best = |pat: &str| s.match_indices(pat).map(|(b, _)| b).filter(|&b| side_ok(b, pat.len())).min_by_key(|&b| b.abs_diff(mid));
    let (b, w, sep) = if let Some(b) = best("; ") {
        (b, 2, "؛ ")
    } else if let Some(b) = best(": ") {
        (b, 2, ": ")
    } else {
        (best(", ")?, 2, "، ")
    };
    Some((strip(&s[..b]).to_string(), strip(&s[b + w..]).to_string(), sep))
}

fn recover_lost(sentences: &[String], first: Vec<String>, lang: &str) -> Vec<String> {
    let mut parts: Vec<Vec<(String, String, &'static str)>> =
        sentences.iter().zip(&first).map(|(s, f)| vec![(s.clone(), f.clone(), "")]).collect();
    for _ in 0..2 {
        let mut asked: Vec<String> = Vec::new();
        for sent in parts.iter_mut() {
            let mut next = Vec::new();
            for (src, fa, sep) in sent.drain(..) {
                match if lost_words(&src, &fa) { two_clauses(&src) } else { None } {
                    Some((l, r, mid)) => {
                        asked.push(l.clone());
                        asked.push(r.clone());
                        next.push((l, String::new(), mid));
                        next.push((r, String::new(), sep));
                    }
                    None => next.push((src, fa, sep)),
                }
            }
            *sent = next;
        }
        if asked.is_empty() {
            break;
        }
        let Some(words) = model_words(&asked, lang) else { break };
        let mut it = words.into_iter();
        for sent in parts.iter_mut() {
            for part in sent.iter_mut().filter(|p| p.1.is_empty()) {
                part.1 = it.next().unwrap_or_default();
            }
        }
    }
    parts
        .iter()
        .zip(first)
        .map(|(sent, f)| {
            let joined: String = sent.iter().map(|(_, fa, sep)| format!("{}{sep}", strip_chars(strip(fa), if sep.is_empty() { "" } else { ".،؛" }))).collect();
            if word_count(&joined) > word_count(&f) { joined } else { f }
        })
        .collect()
}

pub fn translate_sentences(sentences: &[String], lang: &str) -> Vec<String> {
    if sentences.is_empty() {
        return Vec::new();
    }
    let Some(first) = model_words(sentences, lang) else { return sentences.to_vec() };
    let words = recover_lost(sentences, first, lang);
    if std::env::var("KHANA_RAW_TRANSLATION").is_ok() {
        return words;
    }
    words.iter().zip(sentences).map(|(h, src)| tidy_farsi(src, h)).collect()
}

pub fn translate_title(title: &str, lang: &str) -> String {
    split(r"\.\s+", &nfkc(title))
        .iter()
        .map(|p| strip_chars(strip(p), ". ").to_string())
        .filter(|p| !p.is_empty())
        .map(|p| translate_sentences(&[p], lang).join(" "))
        .map(|t| strip_chars(strip(&t), ".").to_string())
        .collect::<Vec<_>>()
        .join("، ")
}

pub fn translate_page(paras: &[String], lang: &str) -> Vec<String> {
    let sents: Vec<Vec<String>> = paras.iter().map(|p| split_source_sentences(&nfkc(p))).collect();
    let flat: Vec<String> = sents.iter().flatten().cloned().collect();
    let tr = translate_sentences(&flat, lang);
    let mut out = Vec::new();
    let mut k = 0;
    for (p, ss) in paras.iter().zip(&sents) {
        if let Some(h) = roman_heading(p) {
            out.push(h);
            k += ss.len();
            continue;
        }
        out.push(tr[k..(k + ss.len()).min(tr.len())].join(" "));
        k += ss.len();
    }
    out
}

