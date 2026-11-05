
use fancy_regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const W: &str = r"\p{L}\p{N}_";
const D: &str = r"\p{Nd}";
const S: &str = r"\t\n\x0B\x0C\r\x1C-\x1F \x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";

pub fn translate(p0: &str) -> String {
    let p = p0
        .replace(r"[^\W\d_]", r"[\p{L}\p{Nl}\p{No}]")
        .replace(r"[^\W\d]", r"[\p{L}\p{Nl}\p{No}_]");
    let c: Vec<char> = p.chars().collect();
    let mut out = String::with_capacity(p.len() + 16);
    let mut in_class = false;
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\\' && i + 1 < c.len() {
            let n = c[i + 1];
            let rep: Option<String> = match n {
                'w' => Some(if in_class { W.into() } else { format!("[{W}]") }),
                'W' => {
                    assert!(!in_class, "\\W inside a class: {p0}");
                    Some(format!("[^{W}]"))
                }
                'd' => Some(if in_class { D.into() } else { format!("[{D}]") }),
                'D' => {
                    assert!(!in_class, "\\D inside a class: {p0}");
                    Some(format!("[^{D}]"))
                }
                's' => Some(if in_class { S.into() } else { format!("[{S}]") }),
                'S' => {
                    assert!(!in_class, "\\S inside a class: {p0}");
                    Some(format!("[^{S}]"))
                }
                'b' => Some(if in_class {
                    r"\x08".into()
                } else {
                    format!("(?:(?<=[{W}])(?![{W}])|(?<![{W}])(?=[{W}]))")
                }),
                'Z' => Some(r"\z".into()),
                _ => None,
            };
            match rep {
                Some(r) => out.push_str(&r),
                None => {
                    out.push(ch);
                    out.push(n);
                }
            }
            i += 2;
            continue;
        }
        if !in_class && ch == '[' {
            in_class = true;
            out.push('[');
            if i + 1 < c.len() && c[i + 1] == '^' {
                out.push('^');
                i += 1;
            }
            if i + 1 < c.len() && c[i + 1] == ']' {
                out.push_str(r"\]");
                i += 1;
            }
            i += 1;
            continue;
        }
        if in_class {
            match ch {
                '[' => out.push_str(r"\["),
                ']' => {
                    in_class = false;
                    out.push(']');
                }
                '-' if i + 1 < c.len() && c[i + 1] == '-' => out.push_str(r"\-"),
                '&' | '~' => {
                    out.push('\\');
                    out.push(ch);
                }
                _ => out.push(ch),
            }
            i += 1;
            continue;
        }
        if ch == '$' {
            out.push_str(r"(?=\n?\z)");
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn cache() -> &'static Mutex<HashMap<String, &'static Regex>> {
    static C: OnceLock<Mutex<HashMap<String, &'static Regex>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn rx(p: &str) -> &'static Regex {
    let mut c = cache().lock().unwrap();
    if let Some(r) = c.get(p) {
        return r;
    }
    let t = translate(p);
    let r = fancy_regex::RegexBuilder::new(&t)
        .backtrack_limit(10_000_000)
        .build()
        .unwrap_or_else(|e| panic!("bad pattern {p:?} -> {t:?}: {e}"));
    let r: &'static Regex = Box::leak(Box::new(r));
    c.insert(p.to_string(), r);
    r
}

pub fn rx_i(p: &str) -> &'static Regex {
    rx(&format!("(?i){p}"))
}

pub fn search(p: &str, s: &str) -> bool {
    rx(p).is_match(s).unwrap_or(false)
}

pub fn is_match_start(p: &str, s: &str) -> bool {
    rx(&format!(r"\A(?:{p})")).is_match(s).unwrap_or(false)
}

pub fn fullmatch(p: &str, s: &str) -> bool {
    rx(&format!(r"\A(?:{p})\z")).is_match(s).unwrap_or(false)
}

pub fn fullmatch_groups(p: &str, s: &str) -> Option<Vec<Option<String>>> {
    let c = rx(&format!(r"\A(?:{p})\z")).captures(s).ok()??;
    Some((0..c.len()).map(|i| c.get(i).map(|m| m.as_str().to_string())).collect())
}

pub fn count(p: &str, s: &str) -> usize {
    rx(p).find_iter(s).filter(|m| m.is_ok()).count()
}

pub fn findall(p: &str, s: &str) -> Vec<String> {
    rx(p).find_iter(s).filter_map(|m| m.ok()).map(|m| m.as_str().to_string()).collect()
}

pub fn sub(p: &str, repl: &str, s: &str) -> String {
    sub_n(p, repl, s, 0)
}

pub fn sub_n(p: &str, repl: &str, s: &str, count: usize) -> String {
    sub_fn_n(p, s, count, |c| expand(repl, c))
}

pub fn sub_fn<F: FnMut(&Captures) -> String>(p: &str, s: &str, f: F) -> String {
    sub_fn_n(p, s, 0, f)
}

pub fn sub_fn_n<F: FnMut(&Captures) -> String>(p: &str, s: &str, count: usize, f: F) -> String {
    sub_re(rx(p), s, count, f)
}

pub fn sub_re<F: FnMut(&Captures) -> String>(r: &Regex, s: &str, count: usize, mut f: F) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    let mut n = 0;
    for c in r.captures_iter(s) {
        let c = match c {
            Ok(c) => c,
            Err(_) => break,
        };
        let m = c.get(0).unwrap();
        out.push_str(&s[last..m.start()]);
        out.push_str(&f(&c));
        last = m.end();
        n += 1;
        if count > 0 && n >= count {
            break;
        }
    }
    out.push_str(&s[last..]);
    out
}

pub fn expand(t: &str, c: &Captures) -> String {
    if !t.contains('\\') {
        return t.to_string();
    }
    let ch: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        if ch[i] == '\\' && i + 1 < ch.len() && ch[i + 1].is_ascii_digit() {
            let g = ch[i + 1] as usize - '0' as usize;
            if let Some(m) = c.get(g) {
                out.push_str(m.as_str());
            }
            i += 2;
        } else if ch[i] == '\\' && i + 1 < ch.len() && ch[i + 1] == '\\' {
            out.push('\\');
            i += 2;
        } else {
            out.push(ch[i]);
            i += 1;
        }
    }
    out
}

pub fn g<'a>(c: &'a Captures, n: usize) -> Option<&'a str> {
    c.get(n).map(|m| m.as_str())
}

pub fn split(p: &str, s: &str) -> Vec<String> {
    let r = rx(p);
    let mut out = Vec::new();
    let mut last = 0;
    for c in r.captures_iter(s) {
        let c = match c {
            Ok(c) => c,
            Err(_) => break,
        };
        let m = c.get(0).unwrap();
        if m.end() == m.start() && m.start() == last && false {
            continue;
        }
        out.push(s[last..m.start()].to_string());
        for i in 1..c.len() {
            out.push(c.get(i).map(|x| x.as_str().to_string()).unwrap_or_default());
        }
        last = m.end();
    }
    out.push(s[last..].to_string());
    out
}


pub fn is_space(c: char) -> bool {
    matches!(c, '\t'..='\r' | '\x1c'..='\x1f' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}'
        | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

pub fn split_ws(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|w| !w.is_empty()).collect()
}

pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

pub fn strip_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    s.trim_matches(|c| chars.contains(c))
}

pub fn len(s: &str) -> usize {
    s.chars().count()
}

pub fn slice(s: &str, a: isize, b: Option<isize>) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len() as isize;
    let norm = |i: isize| -> usize {
        let i = if i < 0 { i + n } else { i };
        i.clamp(0, n) as usize
    };
    let a = norm(a);
    let b = norm(b.unwrap_or(n));
    if a >= b {
        return String::new();
    }
    c[a..b].iter().collect()
}

pub fn translate_map(s: &str, map: &[(char, &str)]) -> String {
    if !s.chars().any(|c| map.iter().any(|(k, _)| *k == c)) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match map.iter().find(|(k, _)| *k == c) {
            Some((_, v)) => out.push_str(v),
            None => out.push(c),
        }
    }
    out
}

pub fn translate_chars(s: &str, from: &str, to: &str) -> String {
    let f: Vec<char> = from.chars().collect();
    let t: Vec<char> = to.chars().collect();
    s.chars()
        .map(|c| match f.iter().position(|x| *x == c) {
            Some(i) => t[i],
            None => c,
        })
        .collect()
}

pub fn nfkc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc().collect()
}

pub fn category(c: char) -> &'static str {
    use unicode_general_category::{get_general_category, GeneralCategory::*};
    match get_general_category(c) {
        UppercaseLetter => "Lu",
        LowercaseLetter => "Ll",
        TitlecaseLetter => "Lt",
        ModifierLetter => "Lm",
        OtherLetter => "Lo",
        NonspacingMark => "Mn",
        SpacingMark => "Mc",
        EnclosingMark => "Me",
        DecimalNumber => "Nd",
        LetterNumber => "Nl",
        OtherNumber => "No",
        ConnectorPunctuation => "Pc",
        DashPunctuation => "Pd",
        OpenPunctuation => "Ps",
        ClosePunctuation => "Pe",
        InitialPunctuation => "Pi",
        FinalPunctuation => "Pf",
        OtherPunctuation => "Po",
        MathSymbol => "Sm",
        CurrencySymbol => "Sc",
        ModifierSymbol => "Sk",
        OtherSymbol => "So",
        SpaceSeparator => "Zs",
        LineSeparator => "Zl",
        ParagraphSeparator => "Zp",
        Control => "Cc",
        Format => "Cf",
        Surrogate => "Cs",
        PrivateUse => "Co",
        Unassigned => "Cn",
        _ => "Cn",
    }
}

pub fn is_mark(c: char) -> bool {
    category(c).starts_with('M')
}

pub fn is_digit(c: char) -> bool {
    let cat = category(c);
    cat == "Nd" || (cat == "No" && matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}' | '\u{2070}'..='\u{2079}'
        | '\u{2080}'..='\u{2089}' | '\u{2460}'..='\u{2468}' | '\u{2474}'..='\u{247c}'
        | '\u{2488}'..='\u{2490}' | '\u{24ea}' | '\u{24f5}'..='\u{24fd}' | '\u{24ff}'
        | '\u{2776}'..='\u{277e}' | '\u{2780}'..='\u{2788}' | '\u{278a}'..='\u{2792}'))
}

pub fn is_alpha(c: char) -> bool {
    category(c).starts_with('L')
}

pub fn is_alnum(c: char) -> bool {
    let cat = category(c);
    cat.starts_with('L') || cat.starts_with('N')
}

pub fn is_upper(s: &str) -> bool {
    let mut any = false;
    for c in s.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            any = true;
        }
    }
    any
}

pub fn lower(s: &str) -> String {
    s.to_lowercase()
}


