
use crate::consts::*;
use crate::pyre::*;

fn under_thousand(mut n: u64) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if n >= 100 {
        parts.push(NF_HUNDREDS[(n / 100) as usize]);
        n %= 100;
    }
    if n >= 20 {
        parts.push(NF_TENS[(n / 10) as usize]);
        n %= 10;
    }
    if (10..20).contains(&n) {
        parts.push(NF_TEENS[(n - 10) as usize]);
        n = 0;
    }
    if n > 0 {
        parts.push(NF_ONES[n as usize]);
    }
    parts.join(" و ")
}

pub fn number_to_words(mut n: u64) -> String {
    if n == 0 {
        return "صفر".into();
    }
    let scales: [(u64, &str); 4] = [(1_000_000_000_000, "تریلیون"), (1_000_000_000, "میلیارد"), (1_000_000, "میلیون"), (1000, "هزار")];
    let mut parts = Vec::new();
    for (value, name) in scales {
        if n >= value {
            let count = n / value;
            n %= value;
            let head = if count == 1 && value == 1000 { String::new() } else { under_thousand(count) + " " };
            parts.push(strip(&format!("{head}{name}")).to_string());
        }
    }
    if n > 0 {
        parts.push(under_thousand(n));
    }
    parts.join(" و ")
}

fn digits_one_by_one(d: &str) -> String {
    d.chars()
        .map(|c| if c == '0' { "صفر".to_string() } else { NF_ONES[c.to_digit(10).unwrap_or(0) as usize].to_string() })
        .collect::<Vec<_>>()
        .join(" ")
}

fn digit_value(s: &str) -> String {
    s.chars().map(|c| decimal_value(c).map(|v| char::from(b'0' + v as u8)).unwrap_or(c)).collect()
}

fn decimal_value(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        return c.to_digit(10);
    }
    if category(c) != "Nd" {
        return None;
    }
    let mut z = c as u32;
    while z > 0 && category(char::from_u32(z - 1).unwrap_or('\0')) == "Nd" && (c as u32 - (z - 1)) < 10 {
        z -= 1;
    }
    Some((c as u32 - z) % 10)
}

fn number_token(whole: &str, frac: Option<&str>) -> String {
    let whole = digit_value(whole);
    let frac = frac.map(digit_value);
    if len(&whole) > 12 || (whole.starts_with('0') && whole.len() > 1) {
        return format!(
            " {}{} ",
            digits_one_by_one(&whole),
            frac.as_ref().filter(|f| !f.is_empty()).map(|f| format!(" {}", digits_one_by_one(f))).unwrap_or_default()
        );
    }
    let w = whole.trim_start_matches('0');
    let w = if w.is_empty() { "0" } else { w };
    let mut words = number_to_words(w.parse::<u64>().unwrap_or(0));
    if let Some(f) = frac.filter(|f| !f.is_empty()) {
        let f = f.trim_end_matches('0');
        if f.is_empty() {
            return format!(" {words} ");
        }
        let unit = match len(f) {
            1 => Some("دهم"),
            2 => Some("صدم"),
            3 => Some("هزارم"),
            _ => None,
        };
        match unit {
            Some(u) => words += &format!(" ممیز {} {u}", number_to_words(f.parse::<u64>().unwrap_or(0))),
            None => words += &format!(" ممیز {}", digits_one_by_one(f)),
        }
    }
    format!(" {words} ")
}

pub fn numbers_to_words(text: &str) -> String {
    let t = sub(r"(?<=\d),(?=\d\d\d\b)", "", text);
    let t = sub_fn(r"((?:شکل|تصویر|جدول|نمودار|نقشه|فصل|بخش|پیوست|عکس)(?:\x{200c}?(?:های|ها))?\s+)(\d+)((?:[.]\d+)+)\b", &t, |c| {
        let parts: Vec<String> = c[3].split('.').filter(|p| !p.is_empty()).map(|p| strip(&number_token(p, None)).to_string()).collect();
        format!("{}{}، {} ", &c[1], strip(&number_token(&c[2], None)), parts.join("، "))
    });
    sub_fn(r"(\d+)(?:[.](\d+))?", &t, |c| number_token(&c[1], g(c, 2)))
}

pub fn join_compounds(text: &str) -> String {
    let mut t = text.to_string();
    for (a, b) in COMPOUND_PAIRS {
        t = sub(&format!(r"(?<![\p{{L}}\x{{200c}}])({a}) ({b})(?![\p{{L}}])"), "\\1\u{200c}\\2", &t);
    }
    t
}

pub fn is_phonemic(text: &str) -> bool {
    !text.chars().any(|c| NF_PERSIAN_LETTERS.contains(c)) && text.chars().any(|c| c.is_ascii_alphabetic())
}

pub fn normalize_for_model(text: &str) -> Result<String, String> {
    if is_phonemic(text) {
        return Ok(text.to_string());
    }
    let out = normalize(text);
    if !strip(text).is_empty() && strip(&out).is_empty() {
        return Err(format!("normalisation emptied the text: {:?}", text.chars().take(60).collect::<String>()));
    }
    Ok(out)
}

fn map_lookup<'a>(m: &'a [(&str, &str)], c: char) -> Option<&'a str> {
    let mut b = [0u8; 4];
    let s: &str = c.encode_utf8(&mut b);
    m.iter().find(|(k, _)| *k == s).map(|(_, v)| *v)
}

pub fn normalize(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut t: String = text.nfc().collect();
    t = sub(NF_INVISIBLES, "", &t);
    t = sub(NF_DIACRITICS, "", &t);
    t = t.chars().map(|c| map_lookup(NF_CHAR_MAP, c).map(|s| s.to_string()).unwrap_or_else(|| c.to_string())).collect();
    t = t
        .chars()
        .map(|c| match c {
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            _ => c,
        })
        .collect();
    t = t.replace('٬', "").replace('٫', ".");
    t = sub(r"[٪%]", " درصد ", &t);
    t = numbers_to_words(&t);
    t = t.chars().map(|c| map_lookup(NF_PUNCT_MAP, c).map(|s| s.to_string()).unwrap_or_else(|| c.to_string())).collect();
    t = t.chars().map(|c| if NF_ALLOWED.contains(c) { c } else { ' ' }).collect();
    let z = NF_ZWNJ;
    t = sub(r"(?<![\p{L}])((?:نا|غیر)?(?:همجنس|دگرجنس|دوجنس|تک[\x{200c}]?جنس))\s+(گرا|باز)", &format!("\\1{z}\\2"), &t);
    t = join_compounds(&t);
    t = sub(&format!("{z}+"), z, &t);
    t = sub_fn(&format!(r"\s*{z}\s*"), &t, |c| if c[0].contains(' ') { " ".into() } else { z.into() });
    t = sub(&format!("(?<![{p}]){z}|{z}(?![{p}])", p = NF_PERSIAN_LETTERS), "", &t);
    let kp = fancy_regex::escape(NF_KEPT_PUNCT);
    t = sub(&format!(r"\s+([{kp}])"), r"\1", &t);
    t = sub(&format!(r"([{kp}])\1+"), r"\1", &t);
    t = strip(&sub(r"\s+", " ", &t)).to_string();
    let lead: String = format!("{NF_KEPT_PUNCT} ");
    t.trim_start_matches(|c| lead.contains(c)).to_string()
}
