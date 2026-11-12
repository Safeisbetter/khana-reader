
use crate::consts::*;
use crate::difflib;
use crate::phon::pronounce_word;
use crate::pyre::*;
use crate::vocab::{english_phonemes, farsi_words, nllb_sp};
use std::collections::{HashMap, HashSet};

pub use crate::consts::{HEH, YEH_KAF};

pub const CHARS_PER_TEXT_PAGE: usize = 2200;

pub fn rtl(s: &str) -> bool {
    search(RTL_RE, s)
}

pub fn has_latin(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_alphabetic())
}

fn fa_word_chars() -> &'static str {
    r"\u0621-\u063A\u0641-\u064Aیپچژکگ"
}


pub fn repair(line: &str, reversed_text: bool, persian_yeh: bool) -> String {
    let mut t = translate_map(&nfkc(line), HEH);
    t = sub(FOOTNOTE_REF, " ", &t);
    t = sub(r"(?:https?://|www\.)\S+|[\w.+-]+@[\w-]+\.[\w.]+|[\ue000-\uf8ff]", " ", &t);
    let fw = fa_word_chars();
    t = sub(&format!(r"[\[\]]\s*[\d۰-۹٠-٩]{{1,2}}(?=[{fw}])|(?<=[{fw}])[\d۰-۹٠-٩]{{1,2}}\s*[\[\]]"), " ", &t);
    if rtl(&t) {
        t = sub(CITATION, " ", &t);
        t = sub(PAGE_REF, " ", &t);
        t = sub(r"(?:[\d۰-۹٠-٩]{1,3}\s*\.\s*)?نگاه کنید به\s*[:.]?\s*(?:[\d۰-۹٠-٩]{1,3}\s*\.?)?\s*$", "", strip(&t));
        t = sub(r"([،,:؛.])\s*\1+", r"\1", &t);
    }
    if reversed_text && rtl(&t) {
        t = t.chars().rev().collect();
        t = sub_fn(LTR_RUN_RE, &t, |c| c[0].chars().rev().collect());
        t = translate_map(&t, MIRROR);
    } else if rtl(&t) {
        if t.chars().find(|c| *c == '(' || *c == ')') == Some(')') {
            t = translate_map(&t, BRACKETS);
        }
    }
    if rtl(&t) {
        t = sub(r"(?<=[»”])[\d۰-۹٠-٩]{1,2}(?=\s+[ء-ی])", "", &t);
    }
    t = sub(r"\(\(+", " «", &t);
    t = sub(r"\(\s*»", "»", &t);
    t = sub(r"([(\[«])\s+", r"\1", &t);
    t = sub(r"\s+([)\]»])", r"\1", &t);
    t = sub(r"\s+([.,،؛:!?؟])", r"\1", &t);
    t = sub(r"(?<=[ء-ی])[()](?=[ء-ی])", " ", &t);
    if persian_yeh {
        t = translate_map(&t, YEH_KAF);
    }
    t = t.replace('\u{0640}', "");
    t = sub(r"([\u064b-\u0652])([^\u064b-\u0652\s]{1,2})\1", r"\2\1", &t);
    t = strip(&sub(r"[ \t\u00a0]+", " ", &t)).to_string();
    t = sub(r"(?<![\w\u200c])ب(?=آ)", "به ", &t);
    t = sub(r"(?<=[\u0621-\u063A\u0641-\u064A\u067E\u0686\u0698\u06A9\u06AF\u06CC])(?=[0-9۰-۹٠-٩])", " ", &t);
    t = sub(
        r"(?<=[0-9۰-۹٠-٩])(?=[\u0621-\u063A\u0641-\u064A\u067E\u0686\u0698\u06A9\u06AF\u06CC])(?!(?:ام|امین|مین|م)(?![\u0621-\u06CC]))",
        " ",
        &t,
    );
    t = sub(r"الهل(?![\w])", "الله", &t);
    t = sub(r"(?<![\w\u200c])االله(?![\w])", "الله", &t);
    t = sub_fn(r"(?<![\w\u200c])([\u0621-\u06CC]{2,6}) الله(?![\w])", &t, |c| {
        let g1 = &c[1];
        if farsi_words().contains(&translate_map(&format!("{g1}الله"), YEH_KAF)) && !OCR_SHORT_WORDS.contains(&g1) {
            format!("{g1}\u{200c}الله")
        } else {
            c[0].to_string()
        }
    });
    t = sub_re(rx(MI_PREFIX), &t, 0, |c| format!("{}\u{200c}", &c[1]));
    t = sub_re(rx(MI_GLUED), &t, 0, |c| format!("{}\u{200c}", &c[1]));
    t = sub(
        &format!(r"(?<![{FA_LETTER}\u200c])([{FA_LETTER}]{{2,}}(?:ده|ته))(است|اند|ایم|اید)(?![{FA_LETTER}])"),
        "\\1\u{200c}\\2",
        &t,
    );
    t = sub_re(rx(WORD_SUFFIX), &t, 0, |c| format!("\u{200c}{}", &c[1]));
    t = sub_re(rx(SPLIT_WORDS), &t, 0, |c| {
        JOINED.iter().find(|(k, _)| *k == &c[1]).map(|(_, v)| v.to_string()).unwrap_or_else(|| c[0].to_string())
    });
    t = sub_re(rx(PREFIXED_VERB), &t, 0, |c| format!("{}\u{200c}", &c[1]));
    sub_re(rx(HA_SUFFIX), &t, 0, |c| format!("\u{200c}{}", g(c, 1).or(g(c, 2)).unwrap_or("")))
}


fn word_char(c: char) -> bool {
    (('\u{0621}'..='\u{06cc}').contains(&c) && !('\u{0660}'..='\u{0669}').contains(&c))
        || ('\u{fb50}'..='\u{fdff}').contains(&c)
        || ('\u{fe70}'..='\u{fefc}').contains(&c)
        || c == '\u{200c}'
}

fn lookup_key(w: &str) -> String {
    translate_map(&translate_map(&nfkc(w), HEH), YEH_KAF).replace(['\u{200c}', '\u{0640}'], "")
}

const UNSEEN: f32 = 20.0;
const FRAGMENT_COST: f32 = 9.3;
const LETTER_NAMES: &[&str] = &["ای", "بی", "سی", "دی", "اف", "جی", "اچ", "آی", "کی", "ال", "ام", "ان", "او", "پی", "کیو", "آر", "اس", "تی", "یو", "وی", "دبلیو", "اکس", "وای", "زد", "زی"];

#[derive(Default)]
pub struct DocWords {
    words: HashMap<String, u32>,
    pairs: std::collections::HashSet<String>,
}

impl DocWords {
    pub fn wipe(&mut self) {
        for (mut w, _) in self.words.drain() {
            crate::ffi::wipe::string(&mut w);
        }
        crate::ffi::wipe::set(&mut self.pairs);
    }

    pub fn count(pages: &[Vec<String>]) -> DocWords {
        let mut raw: HashMap<&str, u32> = HashMap::new();
        let mut raw_pairs: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();
        for para in pages.iter().flatten() {
            for w in para.split(|c: char| !word_char(c)).filter(|w| !w.is_empty()) {
                *raw.entry(w).or_default() += 1;
            }
            let toks: Vec<&str> = para.split(' ').collect();
            for t in toks.windows(2) {
                if !t[0].is_empty() && !t[1].is_empty() && t[0].chars().all(word_char) && t[1].chars().all(word_char) {
                    raw_pairs.insert((t[0], t[1]));
                }
            }
        }
        let mut words: HashMap<String, u32> = HashMap::with_capacity(raw.len());
        for (w, n) in raw {
            *words.entry(lookup_key(w)).or_default() += n;
        }
        let pairs = raw_pairs.into_iter().map(|(a, b)| format!("{} {}", lookup_key(a), lookup_key(b))).collect();
        DocWords { words, pairs }
    }

    fn get(&self, key: &str) -> u32 {
        self.words.get(key).copied().unwrap_or(0)
    }

    fn pair(&self, a: &str, b: &str) -> bool {
        self.pairs.contains(&format!("{a} {b}"))
    }
}

const SUFFIXES: &[&str] = &["اند", "است", "ایم", "اید", "ام", "ای", "تر", "ترین", "ها", "های", "هایی", "ی"];
const SPLIT_ENDINGS: &[&str] = &["های", "هایی", "مان", "تان", "شان", "تری", "ترین", "گری", "گرا", "گرایی", "گرایانه", "سازی", "ساز", "شدگان", "شده", "آمیز", "محور", "پوستی", "انه", "گذاری", "گرایان", "هایشان", "هایمان", "هایش", "هایم", "هایت", "شان", "ست", "دار", "ای", "اند", "است", "ام", "ات", "اش", "ها", "ی"];
const SP_ONLY_COST: f32 = 11.0;
const VERY_RARE_COST: f32 = 14.0;

struct Judge<'a> {
    costs: &'static HashMap<Box<str>, crate::vocab::WordCost>,
    doc: &'a DocWords,
}

impl Judge<'_> {
    fn in_vocab(&self, k: &str) -> bool {
        self.costs.contains_key(k)
    }

    fn sp_word(&self, k: &str) -> bool {
        crate::vocab::nllb_sp().is_none_or(|sp| sp.encode(k).len() <= 1)
    }

    fn cost(&self, k: &str) -> f32 {
        match self.costs.get(k) {
            Some(c) => c.cost,
            None if self.sp_word(k) => SP_ONLY_COST,
            None => UNSEEN,
        }
    }

    fn future(k: &str) -> bool {
        k.starts_with("خواه") || k.starts_with("نخواه")
    }

    fn broken(&self, k: &str) -> bool {
        if k.chars().count() == 1 {
            return true;
        }
        if self.in_vocab(k) || Self::future(k) {
            return false;
        }
        !(self.sp_word(k) && self.doc.get(k) >= 2)
    }

    fn rare(&self, k: &str) -> bool {
        self.broken(k) || (self.cost(k) >= FRAGMENT_COST && k.chars().count() <= 3) || self.cost(k) >= VERY_RARE_COST
    }

    fn rare_here(&self, k: &str) -> bool {
        self.broken(k) || (self.rare(k) && self.doc.get(k) <= if k.chars().count() <= 3 { 2 } else { 1 })
    }

    fn fragment(&self, k: &str) -> bool {
        k.chars().count() <= 2 && self.broken(k)
    }

    fn kept_apart(ka: &str, kb: &str) -> bool {
        ka.is_empty() || kb.is_empty() || ka == "و" || kb == "و" || Self::future(ka) || Self::future(kb)
            || (LETTER_NAMES.contains(&ka) && LETTER_NAMES.contains(&kb))
    }

    fn lookup(&self, ka: &str, kb: &str) -> Option<(String, &crate::vocab::WordCost, bool)> {
        let plain = format!("{ka}{kb}");
        let mut best: Option<(String, bool)> = self.in_vocab(&plain).then(|| (plain.clone(), false));
        let respelled = [kb.strip_prefix("ال").map(|r| format!("{ka}لا{r}")), kb.strip_prefix('آ').filter(|r| ka.chars().count() >= 2 && r.chars().count() >= 2).map(|r| format!("{ka}ا{r}"))];
        for k in respelled.into_iter().flatten() {
            if self.in_vocab(&k) && best.as_ref().is_none_or(|b| self.cost(&k) < self.cost(&b.0)) {
                best = Some((k, true));
            }
        }
        let (k, re) = best?;
        let w = self.costs.get(k.as_str())?;
        Some((k, w, re))
    }

    fn written(parts: &[&str], key: &str, w: &crate::vocab::WordCost, respelled: bool) -> String {
        match &w.spelling {
            Some(s) => s.to_string(),
            None if respelled => key.to_string(),
            None => parts.concat(),
        }
    }

    fn join_two(&self, a: &str, b: &str, after_join: bool) -> Option<(String, f32)> {
        let (ka, kb) = (lookup_key(a), lookup_key(b));
        if Self::kept_apart(&ka, &kb) {
            return None;
        }
        let plain_word = self.costs.get(format!("{ka}هی").as_str()).is_some_and(|w| w.spelling.is_none() && w.cost < VERY_RARE_COST);
        if kb == "هی" && !plain_word && self.costs.get(format!("{ka}ه").as_str()).is_some_and(|w| w.cost < VERY_RARE_COST) {
            return Some((format!("{a}ه\u{200c}ی"), self.cost(&format!("{ka}ه"))));
        }
        let (key, w, respelled) = self.lookup(&ka, &kb)?;
        let spelling = w.spelling.as_deref().unwrap_or(&key);
        let n_a = ka.chars().count();
        let mut letters = 0usize;
        let mut half_space_at_break = false;
        for c in spelling.chars() {
            if c == '\u{200c}' {
                half_space_at_break |= letters == n_a;
            } else {
                letters += 1;
            }
        }
        let sure = self.broken(&ka)
            || self.broken(&kb)
            || (self.rare(&ka) && self.rare(&kb) && {
                let both_one_piece = self.sp_word(&ka) && self.sp_word(&kb);
                (self.rare_here(&ka) && self.rare_here(&kb) && !both_one_piece)
                    || (w.cost <= 11.0 && !both_one_piece)
                    || w.cost <= 9.0
            })
            || ((self.rare(&ka) || self.rare(&kb))
                && self.doc.get(&key) >= 2
                && self.doc.get(&key) >= self.doc.get(&ka).min(self.doc.get(&kb)))
            || (after_join && half_space_at_break && SUFFIXES.contains(&kb.as_str()))
            || (kb == "ان" && !BEFORE_AN.contains(&ka.as_str()))
            || (ka.ends_with("شنا") && kb == "سی");
        let margin = if half_space_at_break { 0.5 } else { 3.0 };
        if !sure || w.cost + margin >= self.cost(&ka) + self.cost(&kb) {
            return None;
        }
        Some((Self::written(&[a, b], &key, w, respelled), w.cost))
    }

    fn join_three(&self, a: &str, b: &str, c: &str) -> Option<(String, f32)> {
        let (ka, kb, kc) = (lookup_key(a), lookup_key(b), lookup_key(c));
        if Self::kept_apart(&ka, &kb) || Self::kept_apart(&kb, &kc) || !self.fragment(&kb) {
            return None;
        }
        let key = format!("{ka}{kb}{kc}");
        let w = self.costs.get(key.as_str())?;
        if w.cost + 3.0 >= self.cost(&ka) + self.cost(&kb) + self.cost(&kc) {
            return None;
        }
        Some((Self::written(&[a, b, c], &key, w, false), w.cost))
    }

    fn heh_ye(&self, w: &str) -> Option<String> {
        let k = lookup_key(w);
        let stem = k.strip_suffix("هی")?;
        let written_apart = match self.costs.get(k.as_str()) {
            Some(e) => e.spelling.as_deref().is_some_and(|s| s.ends_with("ه\u{200c}ی")),
            None => !self.sp_word(&k),
        };
        if stem.chars().count() < 2 || Self::future(&k) || w.contains('\u{200c}') || !written_apart || !self.in_vocab(&format!("{stem}ه")) {
            return None;
        }
        let base = w.strip_suffix('ی').or_else(|| w.strip_suffix('ي'))?;
        Some(format!("{base}\u{200c}ی"))
    }

    fn split_merged(&self, w: &str, only_vav: bool) -> Option<String> {
        let k = lookup_key(w);
        let chars: Vec<char> = w.chars().collect();
        if chars.len() != k.chars().count() || chars.len() < 4 || (!only_vav && self.doc.get(&k) > 1) {
            return None;
        }
        let whole = self.cost(&k);
        let unknown = whole >= 16.0 || (!self.in_vocab(&k) && !self.sp_word(&k));
        let common = |x: &str| x.chars().count() >= 3 && !SPLIT_ENDINGS.contains(&x) && self.costs.get(x).is_some_and(|c| c.cost <= 13.0);
        let kc: Vec<char> = k.chars().collect();
        let with_ending = (2..kc.len()).any(|i| {
            let (l, r): (String, String) = (kc[..i].iter().collect(), kc[i..].iter().collect());
            SPLIT_ENDINGS.contains(&r.as_str()) && self.costs.get(l.as_str()).is_some_and(|c| c.cost < VERY_RARE_COST)
        });
        if with_ending {
            return None;
        }
        let mut best: Option<(f32, usize)> = None;
        for i in 1..kc.len() - 1 {
            let (l, r): (String, String) = (kc[..i].iter().collect(), kc[i..].iter().collect());
            let ok = if only_vav && i > 1 {
                false
            } else if i == 1 {
                l == "و" && r.chars().count() >= 3 && self.costs.get(r.as_str()).is_some_and(|c| c.cost + 5.5 < whole)
            } else {
                unknown && common(&l) && common(&r) && self.doc.pair(&l, &r)
            };
            if ok {
                let c = self.cost(&l) + self.cost(&r);
                if best.is_none_or(|b| c < b.0) {
                    best = Some((c, i));
                }
            }
        }
        let (_, i) = best?;
        Some(format!("{} {}", chars[..i].iter().collect::<String>(), chars[i..].iter().collect::<String>()))
    }
}

fn reversed_number(t: &str) -> Option<String> {
    let sep = t.chars().find(|c| matches!(c, '،' | ',' | '٬'))?;
    let groups: Vec<&str> = t.split(sep).collect();
    let digits = |g: &str| !g.is_empty() && g.chars().all(|c| c.is_ascii_digit() || ('٠'..='٩').contains(&c) || ('۰'..='۹').contains(&c));
    let zero = |c: char| c == '0' || c == '٠' || c == '۰';
    let (first, last) = (groups[0], groups[groups.len() - 1]);
    let ok = groups.len() >= 2
        && groups.iter().all(|g| digits(g))
        && groups[..groups.len() - 1].iter().all(|g| g.chars().count() == 3)
        && (1..=3).contains(&last.chars().count())
        && first.starts_with(zero)
        && !last.starts_with(zero);
    ok.then(|| groups.iter().rev().copied().collect::<Vec<_>>().join(&sep.to_string()))
}

const BEFORE_AN: &[&str] = &["روی", "از", "در", "به", "با", "بر", "که", "برای", "این", "همه", "تا", "بی", "زیر", "پس", "پیش", "بعد", "قبل", "جای", "مثل", "درباره", "سر", "کنار", "طرف", "وجود", "دل", "آخر", "اول", "خود"];

pub fn join_split_words(text: &str, doc: &DocWords) -> String {
    join_split_words_traced(text, doc, &mut |_, _, _| {})
}

fn leading_word(tok: &str) -> usize {
    tok.chars().take_while(|c| word_char(*c)).map(char::len_utf8).sum()
}

pub fn join_split_words_traced(text: &str, doc: &DocWords, seen: &mut dyn FnMut(&str, &str, &str)) -> String {
    let costs = crate::vocab::word_costs();
    if costs.is_empty() || !text.chars().any(word_char) {
        return text.to_string();
    }
    let judge = Judge { costs, doc };
    let toks: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut after_join = false;
    let mut i = 0;
    while i < toks.len() {
        let tok = toks[i];
        i += 1;
        let was_join = std::mem::take(&mut after_join);
        let Some((prev, prev_joined)) = out.last_mut() else {
            out.push((tok.to_string(), false));
            continue;
        };
        let a_at = prev.char_indices().rev().take_while(|(_, c)| word_char(*c)).last().map(|(i, _)| i);
        let b_len = leading_word(tok);
        let (Some(a_at), true) = (a_at, b_len > 0) else {
            out.push((tok.to_string(), false));
            continue;
        };
        let (a, b) = (&prev[a_at..], &tok[..b_len]);
        let abbrev = b == "ان" && tok[b_len..] == *"." && toks.get(i).is_some_and(|n| fullmatch(r"\w{1,3}\.", n));
        let mut ab = if abbrev { None } else { judge.join_two(a, b, was_join) };
        let next = toks.get(i).filter(|_| b_len == tok.len()).map(|n| (*n, leading_word(n))).filter(|(_, l)| *l > 0);
        if let Some((next, c_len)) = next.filter(|_| judge.fragment(&lookup_key(b))) {
            let c = &next[..c_len];
            let bc = judge.join_two(b, c, false);
            let abc = judge.join_three(a, b, c);
            let cost = |w: &str| judge.cost(&lookup_key(w));
            let ab_total = ab.as_ref().map(|x| x.1 + cost(c));
            let bc_total = bc.as_ref().map(|x| cost(a) + x.1);
            let abc_total = abc.as_ref().map(|x| x.1);
            let lowest = [ab_total, bc_total, abc_total].into_iter().flatten().fold(f32::INFINITY, f32::min);
            if abc_total == Some(lowest) {
                let joined = abc.map(|x| x.0).unwrap_or_default();
                seen(&format!("{a} {b}"), c, &joined);
                let rest = &next[c_len..];
                *prev = format!("{}{joined}{rest}", &prev[..a_at]);
                *prev_joined = true;
                after_join = rest.is_empty();
                i += 1;
                continue;
            }
            if bc_total == Some(lowest) {
                ab = None;
            }
        }
        if let Some((joined, _)) = ab {
            seen(a, b, &joined);
            let rest = &tok[b_len..];
            *prev = format!("{}{joined}{rest}", &prev[..a_at]);
            *prev_joined = true;
            after_join = rest.is_empty();
            continue;
        }
        out.push((tok.to_string(), false));
    }
    let mut fixed: Vec<String> = Vec::with_capacity(out.len());
    for (tok, joined) in out {
        let w_len = leading_word(&tok);
        let (w, rest) = tok.split_at(w_len);
        let mended = if w.is_empty() {
            reversed_number(&tok).map(|n| (n, String::new()))
        } else if let Some(m) = judge.heh_ye(w) {
            Some((m, rest.to_string()))
        } else {
            judge.split_merged(w, joined).map(|m| (m, rest.to_string()))
        };
        let tok = match mended {
            Some((m, rest)) => {
                seen(&tok, "", &format!("{m}{rest}"));
                format!("{m}{rest}")
            }
            None => tok,
        };
        if let Some(last) = fixed.last_mut() {
            let (kl, kt) = (lookup_key(last), lookup_key(&tok[..leading_word(&tok)]));
            if leading_word(last) == last.len() && kt.chars().count() >= 2 && kl == format!("{kt}به") {
                let m = format!("{last}\u{200c}{tok}");
                seen(last, &tok, &m);
                *last = m;
                continue;
            }
        }
        fixed.push(tok);
    }
    fixed.join(" ")
}

pub fn repair_default(line: &str) -> String {
    repair(line, false, true)
}


fn digits_value(d: &str) -> Option<u32> {
    d.chars().try_fold(0u32, |a, c| Some(a * 10 + (c.to_digit(10).or_else(|| "۰۱۲۳۴۵۶۷۸۹".chars().position(|x| x == c).or_else(|| "٠١٢٣٤٥٦٧٨٩".chars().position(|x| x == c)).map(|i| i as u32)))?))
}

pub fn drop_note_marks(p: &str, marks: &[u32]) -> String {
    if marks.is_empty() {
        return p.to_string();
    }
    const BEFORE: &[&str] = &[
        "در", "از", "تا", "به", "و", "با", "بر", "یا", "هر", "این", "آن", "سال", "ساله", "ساعت", "ماه", "روز", "صفحه", "شماره",
        "ماده", "بند", "اصل", "فصل", "قرن", "دهه", "حدود", "حدودا", "تقریبا", "جلد", "بخش", "قسمت", "مرحله", "رتبه", "کلاس",
        "پلاک", "خیابان", "کوچه", "سوره", "آیه", "نمره", "تبصره", "قانون", "مصوبه", "سطر", "دی", "بهمن", "اسفند", "آذر", "آبان",
        "مهر", "شهریور", "مرداد", "تیر", "خرداد", "اردیبهشت", "فروردین", "جدول", "شکل", "نمودار", "تصویر", "عکس", "کدام",
        "شماره\u{200c}ی", "ماده\u{200c}ی", "دقیقه\u{200c}ی", "صفحه\u{200c}ی", "فصل\u{200c}های",
    ];
    const AFTER: &[&str] = &[
        "درصد", "نفر", "سال", "ساله", "سالگی", "ماه", "روز", "ساعت", "دقیقه", "ثانیه", "هزار", "میلیون", "میلیارد", "تومان",
        "ریال", "دلار", "یورو", "کیلو", "کیلومتر", "متر", "درجه", "بار", "عدد", "تا", "نوع", "مورد", "تن", "و", "یا", "دی",
        "بهمن", "اسفند", "آذر", "آبان", "مهر", "شهریور", "مرداد", "تیر", "خرداد", "اردیبهشت", "فروردین", "صفحه", "قرن",
    ];
    let mut out = String::new();
    let mut pos = 0;
    let re = rx(r#"([\p{L}‌]+)([»"”]?)\s?([\d۰-۹٠-٩]{1,3})(?=[\s،؛!?؟»"”)]|[.:](?![\d۰-۹٠-٩])|$)"#);
    for c in re.captures_iter(p).flatten() {
        let (all, word, num) = (c.get(0).unwrap(), &c[1], &c[3]);
        let rest = &p[all.end()..];
        let next = rest.trim_start().split(|ch: char| !ch.is_alphanumeric() && ch != '\u{200c}').next().unwrap_or("");
        let listed = search(r"^\.\s*(?:\S+\s+){1,2}[\d۰-۹٠-٩]{1,3}\.", rest) || search(r"[\d۰-۹٠-٩]{1,3}\.\s*(?:\S+\s+)?$", &p[..all.start()]);
        let is_mark = rtl(word)
            && digits_value(num).is_some_and(|n| marks.contains(&n))
            && !BEFORE.contains(&word)
            && !word.ends_with("ماه")
            && !AFTER.contains(&next)
            && !listed;
        if is_mark {
            out.push_str(&p[pos..all.start()]);
            out.push_str(word);
            out.push_str(&c[2]);
            pos = all.end();
        }
    }
    out.push_str(&p[pos..]);
    out
}

pub fn drop_repeated_phrase(p: &str) -> String {
    const OPENING: usize = 16;
    let words = split_ws(p);
    let w: Vec<&str> = words
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let half = x.len() / 2;
            let doubled = i < OPENING && x.is_char_boundary(half) && x[..half] == x[half..] && x[..half].chars().count() >= 5
                && x.chars().all(|c| c.is_alphabetic());
            if doubled { &x[..half] } else { *x }
        })
        .collect();
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    'next: while i < w.len().min(OPENING) {
        for n in 2..=(w.len() - i) / 2 {
            let unit = &w[i..i + n];
            if unit.iter().any(|x| x.contains(['،', ','])) || unit.iter().all(|x| *x == unit[0]) {
                continue;
            }
            let k = w[i..].chunks_exact(n).take_while(|c| *c == unit).count();
            if k >= 2 {
                out.extend_from_slice(unit);
                i += k * n;
                continue 'next;
            }
        }
        out.push(w[i]);
        i += 1;
    }
    out.extend_from_slice(&w[i..]);
    if out == words {
        return p.to_string();
    }
    out.join(" ")
}

const CREDIT_START: &str = r"^(?:Source|Written|Published|First [Pp]ublished|From|Translated|Translation|Translator|Transcribed|Transcription|Transcriber|HTML|Mark-?up|Proofed|Proofread|Proofreading|Online Version|Copyleft|CopyLeft|Version|Delivered)\b[^:]{0,30}:";

pub fn is_credits(p: &str) -> bool {
    if !(p.starts_with(|c: char| c.is_ascii_uppercase())) {
        return false;
    }
    if search(CREDIT_START, p) {
        return len(p) < 300 || count(r"(?:^|[.;]\s+|\s)(?:Source|Written|Published|Translat\w*|Transcri\w*|HTML|Mark-?up|Proof\w*|Copyleft|CopyLeft)\b[^:]{0,30}:", p) >= 2;
    }
    p.starts_with("From ") && search(r"^From [A-Z]", p) && count(r"\b(?:Translated by|Transcribed by|Marked up by|Downloaded with thanks|Republished here)\b", p) >= 2
}

pub fn move_credits(pages: &mut [Vec<String>], scanned: &HashSet<usize>) {
    let n = pages.len();
    if n == 0 || scanned.contains(&n) {
        return;
    }
    let Some(k) = (0..n.min(2)).find(|&k| !scanned.contains(&(k + 1)) && !pages[k].is_empty()) else { return };
    let Some(i) = pages[k].iter().take(3).position(|p| is_credits(p)) else { return };
    let p = pages[k].remove(i);
    let p = strip(&sub(r"\b[A-Z][\w ]{0,20}:\s*\?\s*", "", &p)).to_string();
    if !p.is_empty() {
        pages[n - 1].push(p);
    }
}

const PO_BOX_ADDRESS: &str = r"(?:,\s*[A-Z][\w.]*(?: [A-Z][\w.]*){0,3} \d{1,5}[a-z]?)?,?\s*\b(?:P\.\s?O\.|PO|Post Office) ?Box\s+\d+(?:,?\s*(?:\d{4}\s?[A-Z]{2}|[A-Z]{1,2}-?\d{4,5}|\d{5}(?:-\d{4})?)(?:\s+[A-Z][a-z]+(?:[- ][A-Z][a-z]+)?)?)?(?:,\s*(?:[Tt]he )?(?:Netherlands|Holland|USA|U\.S\.A\.|United States|United Kingdom|UK|Germany|Belgium|France)\b)?";

const DOI: &str = r"(?i)(?:\b(?:https?://(?:dx\.)?doi\.org/|doi\b:?\s*)|(?<![\w/.]))10\.\d{4,9}/\S*[^\s.,;:)\]]";
const ISBN: &str = r"(?i)\b(?:e-?)?isbn(?:-1[03])?:?\s*(?:97[89][\s-]?)?\d[\d\s-]{7,14}[\dX]\b(?:\s*\((?:pdf|epub|e-?book|print|hbk|pbk|hardback|paperback)\))?";

pub fn drop_imprint_pages(pages: &mut [Vec<String>]) {
    const MARKS: &[&str] = &[
        r"(?i)\bcover (?:design|illustration|image|photo)\b", r"(?i)\blay-?out\b\s*:", r"(?i)\btypeset(?:ting)?\b", r"\bISBN\b",
        r"\be-?ISBN\b", r"\bDOI\b|\b10\.\d{4,}/", r"(?i)\bcreative commons\b", r"(?i)\b(?:all|some) rights reserved\b",
        r"(?i)\bwithout limiting the rights\b", r"\bnur \d{3}\b", r"(?i)\bprinted (?:in|by)\b", r"(?i)\bcataloging|cataloguing\b",
        r"(?i)\bUniversity Press\b", r"©", r"(?i)\bfirst published\b", r"(?i)\bpublisher\b", r"شابک", r"(?:کلیه|همه‌ی|تمام) حقوق",
    ];
    for page in pages.iter_mut().take(3) {
        for p in page.iter_mut() {
            *p = strip(&sub(r"(?:\[\d{1,3}(?:\.\d{1,3}){3}\]\s*)?Project MUSE \(\d{4}-\d\d-\d\d[^)]*\)", "", p)).to_string();
        }
        page.retain(|p| !search(r"(?i)^\s*zie de opmerkingen over de transcriptie|^\s*see the transcriber", p));
        page.retain(|p| !p.is_empty() && !is_match_start(r"\s*[A-Z][\w&:.’' -]{1,60}\s\((?:19|20)\d\d\),?\s*\d+(?:\s*\(\d+\))?,\s*(?:e?\d+,\s*)?(?:pp?\.\s*)?\d+\s*[–-]\s*\d+\.?\s*$", &sub(r"\s*(?:doi:\s*\S+|https?://\S+)", "", p)));
    }
    for page in pages.iter_mut().take(10) {
        let text = page.join("\n");
        let only_marks = page.iter().all(|p| len(p) < 120 || MARKS.iter().any(|m| search(m, p)));
        if only_marks && MARKS.iter().filter(|m| search(m, &text)).count() >= 3 {
            page.clear();
        }
    }
}

pub fn drop_back_matter(pages: &mut [Vec<String>]) {
    let n = pages.len();
    if n < 8 {
        return;
    }
    let from = n - (n / 5).max(1);
    for k in from..n {
        let prices = pages[k].iter().filter(|p| search(r"(?i)\b(?:prijs|price|preis|prix)\s*:?\s*(?:f|ƒ|fl\.?|\$|£|€)?\s?\d|\bfr\. ?p\. ?p\.", p)).count();
        let notes = pages[k].iter().any(|p| search(r"(?i)\btranscripti[eo]n?\b|\btranscriber|\betekst\b|\be-?text\b|project gutenberg", p));
        let contents = k + 2 >= n && pages[k].iter().any(|p| contents_heading(p));
        if prices >= 3 {
            pages[k].clear();
        } else if notes || contents {
            pages[k..].iter_mut().for_each(|p| p.clear());
            break;
        }
    }
}

pub fn drop_addresses(pages: &mut [Vec<String>]) {
    for p in pages.iter_mut().flatten() {
        if search(r"\bBox\s+\d", p) {
            *p = strip(&sub(r"\s+([;.,])", r"\1", &sub(PO_BOX_ADDRESS, "", p))).to_string();
        }
        if search(r"(?i)\bdoi\b|\b10\.\d{4,9}/|\bisbn", p) {
            let t = sub(ISBN, "", &sub(DOI, "", p));
            let t = sub(r"(?i)\bDOI:?\s*(?=[.,;]|$)", "", &t);
            *p = strip(&sub(r"\s+([;.,])", r"\1", &sub(r"\s{2,}", " ", &t))).to_string();
        }
    }
    for page in pages.iter_mut() {
        page.retain(|p| !p.is_empty());
    }
}

pub fn is_figure_caption(p: &str) -> bool {
    search(r"^\s*(?:Figures?|Fig\.|Plates?|Illustration|Photo(?:graph)?|Image|شکل|تصویر|نگاره|عکس)\s*[\d۰-۹IVX]", p)
}

pub fn is_caption(p: &str) -> bool {
    search(r"^\s*(?:(?:Figures?|Fig\.|Plates?|Table|Map|Illustration|Photo(?:graph)?|Image|شکل|تصویر|نگاره|جدول|عکس)\s*[\d۰-۹IVX]|(?:Sources?|Credits?|Courtesy|Photo(?:graph)?|منبع|عکس)\s*:)", p)
}

fn sentence_end(p: &str) -> Option<usize> {
    let re = crate::pyre::rx(r#"[.!?؟…][»”"’')\]]*\s+(?=[«“"‘„(\[]?[\p{Lu}\x{0600}-\x{06FF}\d])"#);
    for m in re.find_iter(p).flatten() {
        let head = &p[..m.end()];
        if search(ABBREV, head.trim_end()) || search(r"(?:^|[\s(.])[A-Z][a-z]?\.\s*$", head) {
            continue;
        }
        return Some(m.end());
    }
    None
}

fn caption_block(page: &[String], foot: bool) -> Option<usize> {
    let n = page.len();
    let at = |k: usize| if foot { n - 1 - k } else { k };
    let mut k = 0;
    while k < n {
        if is_caption(&page[at(k)]) {
            k += 1;
            continue;
        }
        if k > 0 {
            if let Some(d) = (k..(k + 2).min(n)).find(|&d| is_caption(&page[at(d)])) {
                k = d + 1;
                continue;
            }
        }
        return Some(at(k));
    }
    None
}

pub fn join_across_pages(mut pages: Vec<Vec<String>>) -> Vec<Vec<String>> {
    let mut carry: Option<(usize, usize, usize)> = None;
    for n in 0..pages.len().saturating_sub(1) {
        let target = match caption_block(&pages[n], true) {
            Some(i) => Some((n, i)),
            None => carry.filter(|c| c.0 == n).map(|c| (c.1, c.2)),
        };
        carry = None;
        let Some((t, i)) = target else { continue };
        let Some((m, j)) = (n + 1..pages.len().min(n + 3)).find_map(|m| caption_block(&pages[m], false).map(|j| (m, j))) else { continue };
        if m == n + 2 && !pages[n + 1].iter().any(|p| is_caption(p)) {
            continue;
        }
        let open = !search(r"[.!?؟:؛»)\]…]\s*$", &pages[t][i]) || search(r"(?:^|[\s(])(?:(?:[A-Z][a-z]?\.){1,3}|Dr\.|Mr\.|Mrs\.|Prof\.|St\.)\s*$", &pages[t][i]);
        let only_farsi = |p: &str| rtl(p) && !p.chars().any(|c| c.is_ascii_alphabetic());
        let same_script = !((!rtl(&pages[t][i]) && only_farsi(&pages[m][j])) || (only_farsi(&pages[t][i]) && !rtl(&pages[m][j])));
        let stops = |p: &str| notes_heading(p) || contents_heading(p);
        let title_page = {
            let tail: Vec<&str> = pages[t][i].split_whitespace().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
            let head: Vec<&str> = pages[m][j].split_whitespace().take(tail.len()).collect();
            let heading = pages[m][j].split_whitespace().count() <= 5 && !pages[m][j].chars().any(|c| ".!?؟…،,;؛:»”)".contains(c));
            (tail.len() == 2 && tail == head) || heading
        };
        if open && !title_page && same_script && !stops(&pages[t][i]) && !stops(&pages[m][j]) {
            let cut = sentence_end(&pages[m][j]);
            let first = match cut {
                Some(e) => {
                    let (head, rest) = pages[m][j].split_at(e);
                    let (head, rest) = (head.trim_end().to_string(), rest.trim_start().to_string());
                    pages[m][j] = rest;
                    head
                }
                None => pages[m].remove(j),
            };
            let last = &mut pages[t][i];
            last.push(' ');
            last.push_str(&first);
            carry = Some((n + 1, t, i));
        }
    }
    pages
}

pub fn chunk_text_pages(paragraphs: Vec<String>) -> Vec<Vec<String>> {
    let mut pages = Vec::new();
    let mut cur = Vec::new();
    let mut size = 0;
    for p in paragraphs {
        size += len(&p);
        cur.push(p);
        if size >= CHARS_PER_TEXT_PAGE {
            pages.push(std::mem::take(&mut cur));
            size = 0;
        }
    }
    if !cur.is_empty() || pages.is_empty() {
        pages.push(cur);
    }
    pages
}

pub fn decode_text(data: &[u8]) -> String {
    let d = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    if let Ok(s) = std::str::from_utf8(d) {
        return s.to_string();
    }
    if data.len() % 2 == 0 {
        let (le, body) = if data.starts_with(b"\xff\xfe") {
            (true, &data[2..])
        } else if data.starts_with(b"\xfe\xff") {
            (false, &data[2..])
        } else {
            (true, data)
        };
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        if let Ok(s) = String::from_utf16(&units) {
            return s;
        }
    }
    String::from_utf8_lossy(data).into_owned()
}

pub fn split_paragraphs(text: &str) -> Vec<String> {
    split(r"\n\s*\n|\r?\n", text).iter().map(|p| strip(p).to_string()).filter(|p| !p.is_empty()).collect()
}


pub fn known_ratio(paras: &[String]) -> Option<f64> {
    let vocab = farsi_words();
    let mut words = Vec::new();
    for p in paras {
        for w in findall(r"[\u0621-\u06CC\u200c]{2,}", &translate_map(p, YEH_KAF)) {
            words.push(w.replace('\u{200c}', ""));
        }
    }
    if words.len() < 15 || vocab.is_empty() {
        return None;
    }
    Some(words.iter().filter(|w| vocab.contains(*w)).count() as f64 / words.len() as f64)
}

pub fn latin_gibberish(paras: &[String]) -> bool {
    let toks: Vec<&str> = paras.iter().flat_map(|p| split_ws(p)).filter(|t| has_latin(t) && !rtl(t)).collect();
    if toks.len() < 30 {
        return false;
    }
    let cmu = crate::vocab::cmu();
    let longer: Vec<String> = toks.iter().map(|t| sub(r"[^A-Za-z']", "", t).to_lowercase()).filter(|w| len(w) >= 3).collect();
    let eng = longer.iter().filter(|w| cmu.contains_key(*w)).count() as f64 / longer.len().max(1) as f64;
    let odd = toks.iter().filter(|t| search(r#"[A-Za-z][.:;_=!?¥°'"\-][A-Za-z.:;]|\d[A-Za-z]|[A-Za-z]\d"#, t)).count() as f64
        / toks.len() as f64;
    eng < 0.5 && odd >= 0.3
}

fn odd_symbol(ch: char) -> bool {
    if ('\u{0600}'..='\u{06ff}').contains(&ch) || "\u{200c}\u{200d}\u{200e}\u{200f}".contains(ch) || is_space(ch) || is_digit(ch) {
        return false;
    }
    if (32..127).contains(&(ch as u32)) && !is_alpha(ch) {
        return false;
    }
    let cat = category(ch);
    !(cat.starts_with('P') || cat.starts_with('Z') || cat.starts_with('M'))
}

pub fn fix_font_symbols(pages: Vec<Vec<String>>) -> (Vec<Vec<String>>, Vec<(char, String)>) {
    let vocab = farsi_words();
    if vocab.is_empty() {
        return (pages, Vec::new());
    }
    let nfkc_keep = |p: &str| -> String {
        split(r"([¼½¾])", p).iter().map(|part| if "¼½¾".contains(part.as_str()) && !part.is_empty() { part.clone() } else { nfkc(part) }).collect()
    };
    let pages: Vec<Vec<String>> = pages.iter().map(|pg| pg.iter().map(|p| nfkc_keep(p)).collect()).collect();
    let sp = nllb_sp();
    let mut tokens: Vec<(char, Vec<(String, usize)>)> = Vec::new();
    for page in &pages {
        for p in page {
            for w in findall(r#"[^\s«»()\[\].,،؛:!?؟"'0-9۰-۹]+"#, p) {
                if search(LIG_FA, &w) {
                    let mut seen: Vec<char> = Vec::new();
                    for ch in w.chars() {
                        if seen.contains(&ch) {
                            continue;
                        }
                        seen.push(ch);
                        if odd_symbol(ch) {
                            let e = match tokens.iter().position(|(c, _)| *c == ch) {
                                Some(i) => &mut tokens[i].1,
                                None => {
                                    tokens.push((ch, Vec::new()));
                                    &mut tokens.last_mut().unwrap().1
                                }
                            };
                            match e.iter_mut().find(|(x, _)| *x == w) {
                                Some(x) => x.1 += 1,
                                None => e.push((w.clone(), 1)),
                            }
                        }
                    }
                }
            }
        }
    }
    let mut fix: Vec<(char, String)> = Vec::new();
    for (ch, words) in &tokens {
        let total: usize = words.iter().map(|x| x.1).sum();
        if total < 3 {
            continue;
        }
        let cands = ["", "ما", "نم", "نر", "لا", "لم", "مم", "تم", "یم", "هم", "نت", "تن", "تر", "مت", "سم", "لل"];
        let hits = |rep: &str| -> usize {
            words.iter().filter(|(w, _)| vocab.contains(&w.replace(*ch, rep).replace('\u{200c}', ""))).map(|x| x.1).sum()
        };
        let common = |rep: &str| -> f64 {
            match sp {
                None => 0.0,
                Some(sp) => {
                    let per = |x: &str| {
                        let x = sub(r"[^\w]", "", x);
                        sp.encode(&x).len() as f64 / len(&x).max(1) as f64
                    };
                    -words.iter().map(|(w, n)| *n as f64 * per(&w.replace(*ch, rep))).sum::<f64>()
                }
            }
        };
        let mut top: Vec<&str> = cands.to_vec();
        top.sort_by(|a, b| hits(b).cmp(&hits(a)));
        let h = hits(top[0]);
        if h < 3 || (h as f64) < 0.5 * total as f64 {
            continue;
        }
        let mut best: Option<(&str, f64, usize)> = None;
        for r in &top {
            if r.is_empty() || (hits(r) as f64) < 0.85 * h as f64 {
                continue;
            }
            let k = (common(r), hits(r));
            if best.map_or(true, |(_, c, hh)| (k.0, k.1).partial_cmp(&(c, hh)) == Some(std::cmp::Ordering::Greater)) {
                best = Some((r, k.0, k.1));
            }
        }
        let mut best = best.map(|b| b.0).unwrap_or("");
        if (hits("") as f64) >= 1.5 * hits(best).max(1) as f64 || best.is_empty() {
            best = "";
        }
        if hits(best) >= 3 && hits(best) as f64 >= 0.5 * total as f64 {
            fix.push((*ch, best.to_string()));
        }
    }
    if fix.is_empty() {
        return (pages, fix);
    }
    let lig = rx(LIG_FA);
    let is_fa = |s: &str| s.chars().next().map_or(false, |c| lig.is_match(&c.to_string()).unwrap_or(false));
    let subp = |p: &str| -> String {
        let cs: Vec<char> = p.chars().collect();
        let mut out = String::new();
        for (i, c) in cs.iter().enumerate() {
            match fix.iter().find(|(k, _)| k == c) {
                Some((_, rep)) => {
                    let before = if i > 0 { cs[i - 1].to_string() } else { " ".to_string() };
                    let after = if i + 1 < cs.len() { cs[i + 1].to_string() } else { " ".to_string() };
                    if is_fa(&before) || is_fa(&after) {
                        out.push_str(rep);
                    } else {
                        out.push(*c);
                    }
                }
                None => out.push(*c),
            }
        }
        out
    };
    let pages = pages.iter().map(|pg| pg.iter().map(|p| subp(p)).collect()).collect();
    (pages, fix)
}


const OTHER_WORDS: &[(&str, &[&str])] = &[
    ("de", &["der", "die", "das", "und", "nicht", "ist", "ein", "eine", "zu", "den", "mit", "sich", "auf", "für", "dem", "von", "auch", "werden", "wird", "nur"]),
    ("fr", &["le", "la", "les", "et", "des", "est", "une", "que", "pas", "dans", "pour", "qui", "sur", "avec", "du", "ce", "sont", "mais", "ont", "aux"]),
    ("es", &["el", "los", "las", "que", "y", "del", "por", "con", "una", "para", "es", "se", "su", "como", "pero", "más", "fue", "sus", "esta", "entre"]),
    ("it", &["il", "di", "che", "e", "per", "una", "non", "con", "del", "della", "sono", "gli", "nel", "alla", "anche", "come", "più", "dei", "delle", "questo"]),
    ("pt", &["o", "os", "que", "e", "do", "da", "em", "um", "uma", "para", "com", "não", "por", "mais", "dos", "das", "como", "mas", "foi", "ao"]),
    ("tr", &["ve", "bir", "bu", "için", "ile", "olarak", "olan", "gibi", "daha", "çok", "ama", "ise", "kadar", "değil", "sonra", "her", "ancak", "veya", "çünkü", "artık"]),
    ("pl", &["i", "w", "na", "z", "że", "się", "nie", "do", "to", "jest", "jak", "ale", "po", "co", "przez", "dla", "od", "są", "już", "tylko"]),
];

pub fn nllb_code(lang: &str) -> &'static str {
    match lang {
        "nl" => "nld_Latn",
        "de" => "deu_Latn",
        "fr" => "fra_Latn",
        "es" => "spa_Latn",
        "it" => "ita_Latn",
        "pt" => "por_Latn",
        "tr" => "tur_Latn",
        "pl" => "pol_Latn",
        _ => "eng_Latn",
    }
}

pub fn detect_language(pages: &[Vec<String>]) -> &'static str {
    let joined: Vec<&str> = pages.iter().take(30).flat_map(|p| p.iter().map(|s| s.as_str())).collect();
    let sample: String = joined.join(" ").chars().take(60000).collect();
    let r = count(RTL_RE, &sample);
    let latin = sample.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if r >= latin || latin < 40 {
        return "fa";
    }
    let low = sample.to_lowercase();
    let words = findall(r"[^\W\d_]+", &low);
    let score = |list: &[&str]| words.iter().filter(|w| list.contains(&w.as_str())).count();
    let en = score(EN_WORDS);
    let nl = score(NL_WORDS);
    let other = OTHER_WORDS.iter().map(|(l, list)| (*l, score(list))).max_by_key(|x| x.1);
    if let Some((l, n)) = other {
        if n >= 4 && n * 2 > en * 3 && n > nl {
            return l;
        }
    }
    if nl > en { "nl" } else { "en" }
}


pub fn latin_only_paragraph(p: &str) -> bool {
    !rtl(p) && search(r"[A-Za-zÀ-ÿ]{3,}", p)
}

const PARAGRAPH_OPENER: &str = r"^\s*(?:و\s+اما|اما|ولی|و\s+برای|به\s+(?:این|همین)\s+دلیل|از\s+(?:این|همین)\s*\x{200c}?رو|بنابراین|با\s+این\s+(?:حال|همه)|از\s+(?:سوی|طرف)\s+دیگر)(?:[\s،,]|$)";

pub fn price_line(p: &str) -> bool {
    const AMOUNT: &str = r"(?:US|CAN|CDN|UK|AUS|NZ|EUR)?\s*[$£€¥]\s*[\d۰-۹٠-٩]+(?:[.,٫٬][\d۰-۹٠-٩]+)?\s*(?:USA|USD|US|CAND|CAN|CAD|CDN|UK|GBP|AUS|AUD|NZ|EUR)?";
    fullmatch(&format!(r"\s*{AMOUNT}(?:\s*[/|،,;·\-–]?\s*{AMOUNT})*\s*[.،]?\s*"), p)
}

pub fn drop_fa_citations(text: &str) -> String {
    if !text.contains('(') {
        return text.to_string();
    }
    sub(
        r"\s*\(\s*[ء-یٰ\u200c]+(?:\s+[ء-یٰ\u200c]+){0,3}\s*[,،.]?\s*(?:[ء-ی]\.?[,،]?\s*)?[,،]?\s*(?:1[34]|19|20)[\d۰-۹٠-٩]{2}[a-z]?(?:\s*[:،,]\s*(?:ص\.?\s*)?[\d۰-۹٠-٩]+(?:\s*[\-–]\s*[\d۰-۹٠-٩]+)?)?\s*\.?\s*\)",
        "",
        text,
    )
}

pub fn drop_glosses(text: &str) -> String {
    let text = &drop_fa_citations(text);
    if !rtl(text) || !has_latin(text) {
        return text.to_string();
    }
    let text = sub_fn(
        r"\s*[\[(]\s*([A-Za-zÀ-ÿ][A-Za-zÀ-ÿ'’\-]*(?:\s+[A-Za-zÀ-ÿ][A-Za-zÀ-ÿ'’\-]*){0,5})\s*[\])]",
        text,
        |c| {
            let g = &c[1];
            if !g.contains(' ') && len(g) <= 6 && g.chars().all(|ch| ch.is_uppercase()) { c[0].to_string() } else { String::new() }
        },
    );
    let text = sub(
        r"\s*[\[(]\s*[A-ZÀ-Þ][A-Za-zÀ-ÿ0-9 ,.:;&'’\-–/]{0,90}?(?:1[5-9]|20)\d\d[a-z]?[A-Za-zÀ-ÿ0-9 ,.:;&'’\-–/]{0,30}\s*[\])]",
        "",
        &text,
    );
    let text = text.as_str();
    if !has_latin(text) {
        return text.to_string();
    }
    let toks: Vec<&str> = text.split(' ').collect();
    let skel_fa = |w: &str| sub(r"[aeoiuAy?\-]", "", &pronounce_word(w)).to_lowercase();
    let skel_en = |w: &str| {
        let ph = english_phonemes(w).unwrap_or_else(|| w.to_lowercase());
        sub(r"[aeiouy?]", "", &ph.to_lowercase().replace("ph", "f").replace("th", "s").replace('c', "k"))
    };
    let mut keep: Vec<String> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let word = sub(r"[^A-Za-zÀ-ÿ]", "", t);
        if !word.is_empty() && len(&word) >= 3 && !search(r"[ء-ی]", t) {
            let mut near = Vec::new();
            for j in [i as isize - 1, i as isize + 1] {
                if j >= 0 && (j as usize) < toks.len() {
                    let w = sub(r"[^\w]", "", toks[j as usize]);
                    if !w.is_empty() && rtl(&w) {
                        near.push(w);
                    }
                }
            }
            let se = skel_en(&word);
            if near.iter().any(|w| difflib::ratio_str(&skel_fa(w), &se, true) >= 0.6) {
                keep.push(strip_chars(&sub(r"[A-Za-zÀ-ÿ]+", "", t), "()[] ").to_string());
                continue;
            }
        }
        keep.push(t.to_string());
    }
    strip(&sub(r"\s{2,}", " ", &keep.join(" ")).replace(" ،", "،")).to_string()
}

const NOTES_HEAD_MORE: &str = r"^\s*(?:(?:پی|پا)[\x{200c} ]?(?:نوشت|نویس)(?:[\x{200c} ]?ها)?|یادداشت[\x{200c} ]ها|منابع و ارجاعات|فهرست منابع|ارجاعات|Endnotes|ENDNOTES|Notes and [Rr]eferences|Works [Cc]ited|Archives|Literature|Noten|Literatuur|Anmerkungen|Literatur|Quellen|(?i:archives|archival (?:sources|material|collections)|(?:primary|secondary|unpublished|printed) sources|sources(?: and (?:literature|references|bibliography))?|(?:selected |select )?bibliography|bibliografie|references(?: cited)?|literature cited|further reading|sources consulted|quellen und literatur|(?:general |name |subject )?index(?: of (?:names|persons|subjects))?|personenregister|list of (?:figures|illustrations|tables|plates|maps|abbreviations)|about the (?:authors?|editors?|contributors)|(?:notes on |list of )?contributors|acknowledge?ments?|dankwoord|danksagung|remerciements)|سپاس[\x{200c} ]?گزاری|سپاسگزاری|قدردانی|تشکر و قدردانی|فهرست(?: تصاویر| شکل[\x{200c} ]?ها| جدول[\x{200c} ]?ها)|کتاب[\x{200c} ]?شناسی|آرشیوها|اسناد آرشیوی|مآخذ|نمایه|فهرست راهنما|فهرست اعلام)\s*[:.]?\s*$";

pub fn notes_heading(t: &str) -> bool {
    let t = nfkc(t);
    is_match_start(NOTES_HEAD, &t) || is_match_start(NOTES_HEAD_MORE, &t)
}

const INTRO_HEAD: &str = r"\s*(?:[\d۰-۹]{1,2}[\s.]+)?(?:مقدمه|پیش[\x{200c} ]?گفتار|پیش[\x{200c} ]?درآمد|درآمد|دیباچه|سرآغاز|سخن مترجم|یادداشت مترجم|یادداشت ناشر|(?i:introduction|preface|foreword|prologue|voorwoord|voorrede|inleiding|einleitung|vorwort))(?:[\s:.،,]|$)";

pub fn contents_heading(p: &str) -> bool {
    is_match_start(r"\s*(?:(?i:table of )?(?i:contents)|(?i:inhoud(?:sopgave)?)|فهرست(?:[\x{200c} ]مطالب)?)\s*[:.]?\s*$", p)
}

pub fn tidy_contents(paras: &[String]) -> Vec<String> {
    let leader = r"(?:\.\s?){3,}|(?:…\s?)+|(?:_\s?){3,}";
    let digits = r"[\d۰-۹٠-٩]{1,3}";
    let listed = paras.iter().filter(|p| search(&format!(r"(?:{leader})\s*{digits}\s*$|^\s*{digits}\s*(?:{leader})"), p)).count();
    let heading = paras.iter().any(|p| contents_heading(p));
    let numbered = paras.iter().filter(|p| search(&format!(r"[\p{{L}}.)»’]\s+{digits}\s*$"), p)).count();
    let page_is_contents = heading || listed >= 2 || (numbered >= 3 && numbered * 2 >= paras.len());
    paras
        .iter()
        .map(|p| {
            let mut t = sub(&format!(r"\s*(?:{leader})\s*{digits}(?=\s|$)|^\s*{digits}\s*(?:{leader})\s*"), " ", p);
            if t != *p {
                t = strip(&sub(&format!(r"\s*(?:{leader})\s*"), " ", &t)).to_string();
            }
            t = sub_fn(&format!(r"^(\s*{digits}\s+\S.*?)\s({digits})\s+(\p{{Lu}}.*)$"), &t, |m| {
                if search(r"[.!?؟،,]$", &m[1]) { m[0].to_string() } else { format!("{}. {}", &m[1], &m[3]) }
            });
            if page_is_contents {
                t = sub(&format!(r"(?<=[\p{{L}}.)»’])\s+{digits}\s*$"), "", &t);
            }
            t
        })
        .collect()
}

fn note_like(p: &str) -> bool {
    is_match_start(NOTE_ITEM, p)
        || is_match_start(r"\s*\*", p)
        || search(CITATION_LINE, p)
        || search(r"\b(?:[Pp]apers|Archie[fv]|Archives?|Archivo|Library|Collection|Nachlass)\b", p)
        || search(r"^\s*\d{1,2}\s+\S.*\s\d{1,3}\s+[A-Z]", p)
        || search(r"^\s*(?:Index|Bibliography|Notes|Contents)\s+\d{1,3}\s*$", p)
        || search(r"\b(?:[\d۰-۹]{1,3}(?:[-–][\d۰-۹]{1,3})?n?[,،]\s*){2}[\d۰-۹]{1,3}\b", p)
        || search(r"\bنک\.|ن\. ?ک\.|بنگرید|نگاه کنید به|\bهمان[.،]|\bص\.? ?[\d۰-۹]|\bصص\b|\b(?:1[34]|19|20)\d\d\b|[۱][۳۴۹][۰-۹]{2}|۲۰[۰-۹]{2}", p)
}

pub type NotesState = HashMap<usize, bool>;

#[derive(Clone, Copy, Default)]
pub struct NoteHints {
    pub labels: usize,
    pub last_label: Option<f64>,
}

pub fn note_hints(raw: &[String]) -> NoteHints {
    let at: Vec<usize> = raw.iter().enumerate().filter(|(_, p)| is_match_start(NOTE_LABEL, p)).map(|(i, _)| i).collect();
    NoteHints { labels: at.len(), last_label: at.last().map(|i| (*i as f64 + 1.0) / raw.len() as f64) }
}

pub fn skip_notes(notes: &mut NotesState, n: usize, paras: &[String], prev_page: &mut dyn FnMut(usize) -> Vec<String>) -> Vec<String> {
    skip_notes_hinted(notes, n, paras, NoteHints::default(), &mut |k| (prev_page(k), NoteHints::default()))
}

pub fn skip_notes_hinted(notes: &mut NotesState, n: usize, paras: &[String], hints: NoteHints, prev_page: &mut dyn FnMut(usize) -> (Vec<String>, NoteHints)) -> Vec<String> {
    let labels = hints.labels;
    let mut state = if n > 1 { *notes.get(&(n - 1)).unwrap_or(&false) } else { false };
    if n > 1 && !notes.contains_key(&(n - 1)) {
        let (prev, prev_hints) = prev_page(n - 1);
        skip_notes_hinted(notes, n - 1, &prev, prev_hints, prev_page);
        state = *notes.get(&(n - 1)).unwrap_or(&false);
    }
    let paras: Vec<&String> = paras.iter().filter(|p| !strip(p).is_empty()).collect();
    let items = paras.iter().filter(|p| is_match_start(NOTE_ITEM, p)).count().max(labels);
    let like = paras.iter().filter(|p| note_like(p)).count();
    if state && !paras.is_empty() && items < 2 && items * 3 < paras.len() && like * 2 < paras.len() {
        state = false;
    }
    let mut out = Vec::new();
    for (k, p) in paras.iter().enumerate() {
        if is_match_start(NOTES_HEAD, p) || is_match_start(NOTES_HEAD_MORE, p) {
            state = true;
            continue;
        }
        if state && len(p) < 70 && is_match_start(INTRO_HEAD, p) && !search(r"[\d۰-۹][\s.]*$|\.\s?\.", p) {
            let next_prose = paras.get(k + 1).is_some_and(|q| len(q) > 150 && !is_match_start(NOTE_ITEM, q) && !is_match_start(r"[A-Z][\w'’-]+,\s+[A-Z]", q));
            if next_prose {
                state = false;
            }
        }
        let notes_follow = hints.last_label.is_some_and(|f| (k as f64 + 0.5) / paras.len() as f64 <= f);
        if state && !notes_follow && len(p) < 160 && !is_caption(p) && !search(r"[.:؛]$", strip(p)) {
            let prose = |i: usize| {
                paras.get(i).is_some_and(|q| {
                    if is_match_start(r"\s*(?:Abstract|Summary|Introduction|چکیده|خلاصه|مقدمه)\b", q) {
                        return true;
                    }
                    let digits = q.chars().filter(|c| c.is_numeric()).count();
                    let refs = r"\bet al\b|\b[Ii]bid\b|\bpp?\.\s?\d|\bvol\.|\beds?\.|\bPress\b|\bJournal\b|https?://|www\.|\bnote \d|[’'],?\s+in\b|\bنک\.|ن\. ?ک\.|\bهمان[.،]|بنگرید|نگاه کنید به|\bص\.? ?[\d۰-۹]|\bصص\b";
                    len(q) > 150 && (!note_like(q) || ((digits * 40 < len(q) || digits <= 10) && !search(refs, q)))
                        && !is_match_start(NOTE_ITEM, q) && !is_match_start(r"\s*\*", q) && !is_match_start(r"[A-Z][\w'’-]+,\s+[A-Z]", q)
                })
            };
            let short = |i: usize| paras.get(i).is_some_and(|q| !note_like(q) && len(q) < 80);
            let titled = !is_match_start(NOTE_ITEM, p) && !search(r"\b(?:1[5-9]|20)\d\d\b", p);
            if (!note_like(p) && len(p) < 80 && prose(k + 1)) || ((!note_like(p) || titled) && short(k + 1) && prose(k + 2)) {
                state = false;
            }
        }
        if !state {
            out.push((*p).clone());
        }
    }
    notes.insert(n, state);
    out
}

pub fn garbled(p: &str) -> bool {
    let words = split_ws(p);
    let bad = words
        .iter()
        .filter(|w| {
            let farsi = w.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c));
            let latin = w.chars().any(|c| c.is_ascii_alphabetic());
            (farsi && latin) || w.chars().any(|c| "¿¡ÔÕÐÞ".contains(c) || ('\u{0300}'..='\u{036F}').contains(&c))
        })
        .count();
    bad >= 2 && bad * 5 >= words.len()
}

pub fn has_letters(p: &str) -> bool {
    count(r"[^\W\d_]", p) > 1
}




#[derive(Clone, Debug)]
pub struct LayoutItem {
    pub s: String,
    pub x0: f64,
    pub top: f64,
    pub x1: f64,
    pub bot: f64,
    pub size: f64,
    pub rtl: bool,
}

pub struct LLine {
    pub text: String,
    top: f64,
    bot: f64,
    x0: f64,
    x1: f64,
    size: f64,
}

impl LLine {
    pub fn geometry(&self) -> (f64, f64, f64, f64, f64) {
        (self.top, self.bot, self.x0, self.x1, self.size)
    }
}

const CONNECTING_FORMS: &[(char, char)] = &[
    ('\u{fb54}', '\u{fb55}'),
    ('\u{fb58}', '\u{fb59}'),
    ('\u{fb5c}', '\u{fb5d}'),
    ('\u{fb60}', '\u{fb61}'),
    ('\u{fb64}', '\u{fb65}'),
    ('\u{fb68}', '\u{fb69}'),
    ('\u{fb6c}', '\u{fb6d}'),
    ('\u{fb70}', '\u{fb71}'),
    ('\u{fb74}', '\u{fb75}'),
    ('\u{fb78}', '\u{fb79}'),
    ('\u{fb7c}', '\u{fb7d}'),
    ('\u{fb80}', '\u{fb81}'),
    ('\u{fb90}', '\u{fb91}'),
    ('\u{fb94}', '\u{fb95}'),
    ('\u{fb98}', '\u{fb99}'),
    ('\u{fb9c}', '\u{fb9d}'),
    ('\u{fba2}', '\u{fba3}'),
    ('\u{fba8}', '\u{fba9}'),
    ('\u{fbac}', '\u{fbad}'),
    ('\u{fbd5}', '\u{fbd6}'),
    ('\u{fbe6}', '\u{fbe9}'),
    ('\u{fbf8}', '\u{fbf8}'),
    ('\u{fbfb}', '\u{fbfb}'),
    ('\u{fbfe}', '\u{fbff}'),
    ('\u{fc97}', '\u{fcf4}'),
    ('\u{fd2d}', '\u{fd3b}'),
    ('\u{fd50}', '\u{fd50}'),
    ('\u{fd52}', '\u{fd57}'),
    ('\u{fd59}', '\u{fd59}'),
    ('\u{fd5c}', '\u{fd5d}'),
    ('\u{fd60}', '\u{fd61}'),
    ('\u{fd63}', '\u{fd63}'),
    ('\u{fd65}', '\u{fd65}'),
    ('\u{fd68}', '\u{fd68}'),
    ('\u{fd6b}', '\u{fd6b}'),
    ('\u{fd6d}', '\u{fd6d}'),
    ('\u{fd70}', '\u{fd70}'),
    ('\u{fd72}', '\u{fd73}'),
    ('\u{fd77}', '\u{fd77}'),
    ('\u{fd7d}', '\u{fd7d}'),
    ('\u{fd83}', '\u{fd83}'),
    ('\u{fd86}', '\u{fd86}'),
    ('\u{fd88}', '\u{fd8a}'),
    ('\u{fd8c}', '\u{fd8f}'),
    ('\u{fd92}', '\u{fd95}'),
    ('\u{fd98}', '\u{fd98}'),
    ('\u{fd9d}', '\u{fd9d}'),
    ('\u{fdb4}', '\u{fdb5}'),
    ('\u{fdb8}', '\u{fdb8}'),
    ('\u{fdba}', '\u{fdba}'),
    ('\u{fdc3}', '\u{fdc5}'),
    ('\u{fe77}', '\u{fe77}'),
    ('\u{fe79}', '\u{fe79}'),
    ('\u{fe7b}', '\u{fe7b}'),
    ('\u{fe7d}', '\u{fe7d}'),
    ('\u{fe7f}', '\u{fe7f}'),
    ('\u{fe8b}', '\u{fe8c}'),
    ('\u{fe91}', '\u{fe92}'),
    ('\u{fe97}', '\u{fe98}'),
    ('\u{fe9b}', '\u{fe9c}'),
    ('\u{fe9f}', '\u{fea0}'),
    ('\u{fea3}', '\u{fea4}'),
    ('\u{fea7}', '\u{fea8}'),
    ('\u{feb3}', '\u{feb4}'),
    ('\u{feb7}', '\u{feb8}'),
    ('\u{febb}', '\u{febc}'),
    ('\u{febf}', '\u{fec0}'),
    ('\u{fec3}', '\u{fec4}'),
    ('\u{fec7}', '\u{fec8}'),
    ('\u{fecb}', '\u{fecc}'),
    ('\u{fecf}', '\u{fed0}'),
    ('\u{fed3}', '\u{fed4}'),
    ('\u{fed7}', '\u{fed8}'),
    ('\u{fedb}', '\u{fedc}'),
    ('\u{fedf}', '\u{fee0}'),
    ('\u{fee3}', '\u{fee4}'),
    ('\u{fee7}', '\u{fee8}'),
    ('\u{feeb}', '\u{feec}'),
    ('\u{fef3}', '\u{fef4}'),
];

fn cut_evidence(left: &str) -> Option<bool> {
    let c = left.chars().last()?;
    let base = nfkc(&c.to_string());
    if base.chars().last().is_some_and(|b| "اآأإٱدذرزژوؤة".contains(b)) {
        return Some(false);
    }
    if ('\u{fb50}'..='\u{fdff}').contains(&c) || ('\u{fe70}'..='\u{feff}').contains(&c) {
        return Some(!CONNECTING_FORMS.iter().any(|(a, b)| (*a..=*b).contains(&c)));
    }
    None
}

fn is_number_piece(s: &str) -> bool {
    s.chars().any(|c| c.is_numeric()) && !s.chars().any(|c| c.is_alphabetic())
}

pub fn paragraphs_from_layout(items: &[LayoutItem], page_h: f64) -> Vec<String> {
    paragraphs_from_lines(&layout_lines(items, page_h))
}

fn leading_marks_moved(items: &[LayoutItem]) -> Vec<LayoutItem> {
    let mut v: Vec<LayoutItem> = items.to_vec();
    let is_mark = |c: &char| ('\u{064b}'..='\u{0652}').contains(c);
    for k in 0..v.len().saturating_sub(1) {
        let lead: String = v[k].s.chars().take_while(is_mark).collect();
        let n = lead.chars().count();
        if n == 0 || n == v[k].s.chars().count() || !v[k].rtl || !v[k + 1].rtl {
            continue;
        }
        if (v[k].top - v[k + 1].top).abs() >= 0.5 * (v[k].bot - v[k].top).min(v[k + 1].bot - v[k + 1].top) {
            continue;
        }
        v[k].s = v[k].s.chars().skip(n).collect();
        v[k + 1].s.push_str(&lead);
    }
    v
}

fn word_cut_in_two(left: &str, right: &str) -> bool {
    let is_letter = |c: char| ('\u{0621}'..='\u{06cc}').contains(&c);
    let plain = |t: String| translate_map(&t.chars().filter(|c| *c != '\u{0640}' && !('\u{064b}'..='\u{0652}').contains(c)).collect::<String>(), YEH_KAF);
    let tail: String = left.chars().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
    let (left, right) = (nfkc(&tail), nfkc(right));
    let l = plain(left.chars().rev().take_while(|c| is_letter(*c)).collect::<Vec<_>>().into_iter().rev().collect());
    let r = plain(right.chars().take_while(|c| is_letter(*c)).collect());
    let (nl, nr) = (l.chars().count(), r.chars().count());
    if nl == 0 || nr == 0 || nl.min(nr) > 2 {
        return false;
    }
    const STAND_ALONE: &[&str] = &["می", "نمی", "بی", "به", "هم", "نا", "یک", "دو", "سه", "تک", "پی", "پس", "تن", "کم", "پر", "دی", "در", "بر", "از", "آن", "این"];
    const ENDINGS: &[&str] = &["ها", "ی", "ای", "ام", "ات", "اش", "تر", "ان", "اند", "ایم", "اید", "رو", "گر", "گو", "زا", "کن", "سو", "ماه", "بار", "وار"];
    if (nl <= 2 && STAND_ALONE.contains(&l.as_str())) || (nr <= 2 && ENDINGS.contains(&r.as_str())) {
        return false;
    }
    let c = crate::vocab::word_costs();
    let word = |x: &str| x.chars().count() >= 2 && c.get(x).is_some_and(|v| v.cost <= 13.0);
    let whole = format!("{l}{r}");
    let short = if nl <= nr { &l } else { &r };
    let fragment = (short.chars().count() == 2 && !word(short)) || (nl == 1 && l != "و");
    ((!word(&l) && !word(&r)) || fragment) && c.get(whole.as_str()).is_some_and(|v| v.cost <= 13.5)
}

fn mend_heh_cuts(line: &str) -> String {
    if !line.contains("ه\u{200c}") {
        return line.to_string();
    }
    let c = crate::vocab::word_costs();
    let cost = |x: &str| c.get(x).map_or(99.0, |v| v.cost);
    const ENDINGS: &[&str] = &["ها", "های", "هایی", "هایش", "هایشان", "ی", "ای", "ام", "ات", "اش", "اند", "ایم", "اید", "است", "ان", "تر", "ترین", "شان", "مان", "تان"];
    sub_fn(r"([ء-ی]*ه)\x{200c}([ء-ی]+)", line, |m| {
        let plain = |t: &str| translate_map(&nfkc(t), YEH_KAF);
        let (h, r) = (plain(&m[1]), plain(&m[2]));
        let first_no_word = h.chars().count() == 1 || cost(&h) >= 99.0;
        if first_no_word && !ENDINGS.contains(&r.as_str()) && cost(&format!("{h}{r}")) <= 12.5 { format!("{}{}", &m[1], &m[2]) } else { m[0].to_string() }
    })
}

pub fn join_spaced_letters(piece: &str) -> String {
    let t = strip(piece);
    if fullmatch(r"\p{Ll} \p{Ll}", t) && t.is_ascii() {
        return piece.replace(' ', "");
    }
    if !search(r"[A-Za-zÀ-ÿ] [A-Za-zÀ-ÿ] [A-Za-zÀ-ÿ]", piece) {
        return piece.to_string();
    }
    sub_fn(r"(?<![A-Za-zÀ-ÿ])[A-Za-zÀ-ÿ](?: [A-Za-zÀ-ÿ]){2,}(?![A-Za-zÀ-ÿ])", piece, |m| {
        if m[0].chars().any(|c| c.is_lowercase()) { m[0].replace(' ', "") } else { m[0].to_string() }
    })
}

pub fn layout_lines(items: &[LayoutItem], page_h: f64) -> Vec<LLine> {
    let items: Vec<LayoutItem> = items.iter().map(|i| LayoutItem { s: join_spaced_letters(&sub(r"\+{3,}", "", &i.s.replace('\u{fffd}', ""))), ..i.clone() }).collect();
    let items = leading_marks_moved(&items);
    let mut items: Vec<&LayoutItem> = items.iter().filter(|i| !strip(&i.s).is_empty() && i.bot > i.top).collect();
    let marks: Vec<usize> = (0..items.len()).filter(|&k| is_note_mark(items[k], &items)).collect();
    for k in marks.into_iter().rev() {
        items.remove(k);
    }
    items.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));
    let mut groups: Vec<Vec<&LayoutItem>> = Vec::new();
    for it in items {
        let joins = groups.last().is_some_and(|g| {
            let (t, b) = (g.iter().map(|x| x.top).fold(f64::MAX, f64::min), g.iter().map(|x| x.bot).fold(f64::MIN, f64::max));
            let overlap = it.bot.min(b) - it.top.max(t);
            overlap > 0.5 * (it.bot - it.top).min(b - t)
        });
        if joins {
            groups.last_mut().unwrap().push(it);
        } else {
            groups.push(vec![it]);
        }
    }
    let mut split: Vec<Vec<&LayoutItem>> = Vec::new();
    for mut g in groups {
        g.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cut = None;
        for k in 1..g.len() {
            let (l, r) = (&g[..k], &g[k..]);
            let reach = l.iter().map(|x| x.x1).fold(f64::MIN, f64::max);
            let size = g[k].size.min(g[k - 1].size);
            let mid = |v: &[&LayoutItem]| v.iter().map(|x| x.top + x.bot).sum::<f64>() / (2.0 * v.len() as f64);
            let big = |v: &[&LayoutItem]| v.iter().map(|x| x.size).fold(f64::MIN, f64::max);
            let other_size = (big(l) - big(r)).abs() > 0.04 * big(l).max(big(r));
            let wide = |v: &[&LayoutItem]| v.iter().map(|x| x.x1).fold(f64::MIN, f64::max) - v.iter().map(|x| x.x0).fold(f64::MAX, f64::min) > 6.0 * size;
            if g[k].x0 - reach > 0.5 * size && ((mid(l) - mid(r)).abs() > 0.25 * size || other_size) && wide(l) && wide(r) {
                cut = Some(k);
                break;
            }
        }
        match cut {
            Some(k) => {
                let r = g.split_off(k);
                split.push(g);
                split.push(r);
            }
            None => split.push(g),
        }
    }
    split.sort_by(|a, b| {
        let t = |g: &Vec<&LayoutItem>| g.iter().map(|x| x.top).fold(f64::MAX, f64::min);
        t(a).partial_cmp(&t(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    let groups = split;
    let mut lines: Vec<LLine> = Vec::new();
    for mut g in groups {
        let rtl_letters: usize = g.iter().map(|i| count(RTL_RE, &i.s)).sum();
        let ltr_letters: usize = g.iter().map(|i| i.s.chars().filter(|c| c.is_alphabetic() && !rtl(&c.to_string())).count()).sum();
        let right_to_left = rtl_letters > ltr_letters;
        if right_to_left {
            g.sort_by(|a, b| b.x1.partial_cmp(&a.x1).unwrap_or(std::cmp::Ordering::Equal));
            let mut k = 0;
            while k < g.len() {
                let mut j = k + 1;
                if is_number_piece(&g[k].s) {
                    while j < g.len() && is_number_piece(&g[j].s) && g[j - 1].x0 - g[j].x1 <= 0.05 * g[j].size.min(g[j - 1].size) {
                        j += 1;
                    }
                    g[k..j].reverse();
                }
                k = j;
            }
        } else {
            g.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        }
        let farsi_end = |t: &str| t.chars().last().is_some_and(|c| rtl(&c.to_string()) && c.is_alphabetic());
        let farsi_start = |t: &str| t.chars().next().is_some_and(|c| rtl(&c.to_string()) && c.is_alphabetic());
        let mut touch = vec![false; g.len()];
        let mut space = vec![false; g.len()];
        for k in 1..g.len() {
            let (p, it) = (g[k - 1], g[k]);
            let gap = if right_to_left { p.x0 - it.x1 } else { it.x0 - p.x1 };
            if gap > 0.05 * it.size.min(p.size) {
                space[k] = !p.s.ends_with(' ') && !it.s.starts_with(' ');
            } else if right_to_left && farsi_end(&p.s) && farsi_start(&it.s) {
                touch[k] = true;
            }
        }
        let mut text = String::new();
        let mut cuts = 0;
        let mut k = 0;
        while k < g.len() {
            if space[k] {
                text.push(' ');
            }
            let mut j = k + 1;
            while j < g.len() && touch[j] {
                j += 1;
            }
            for m in k..j {
                if m > k {
                    let before_lam_alef = nfkc(&g[m].s).starts_with("لا");
                    let lone = |t: &str| !strip(t).is_empty() && strip(t).chars().all(|c| c == 'ي' || c == 'ك');
                    let ezafe = lone(&g[m].s) && g[m - 1].s.ends_with(['ه', 'ة']) && !(m + 1 < j);
                    let stretched = g[m - 1].s.trim_end().ends_with('\u{0640}') || g[m].s.trim_start().starts_with(|c: char| c == '\u{0640}' || ('\u{064b}'..='\u{0652}').contains(&c));
                    let font_change = (lone(&g[m].s) && !ezafe) || lone(&g[m - 1].s) || stretched;
                    if !font_change && !word_cut_in_two(&text, &g[m].s) && cut_evidence(&g[m - 1].s).unwrap_or(!before_lam_alef) {
                        text.push('\u{200c}');
                        cuts += 1;
                    }
                }
                text.push_str(&g[m].s);
            }
            k = j;
        }
        let mut sizes: Vec<f64> = g.iter().map(|i| i.size).collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if cuts >= 3 && cuts > 2 * text.matches(' ').count() {
            text = text.replace('\u{200c}', " ");
        }
        let text = mend_heh_cuts(&split_ws(&text).join(" "));
        let text = if right_to_left { translate_chars(&text, "٠١٢٣٤٥٦٧٨٩", "۰۱۲۳۴۵۶۷۸۹") } else { text };
        let (top, bot) = (g.iter().map(|x| x.top).fold(f64::MAX, f64::min), g.iter().map(|x| x.bot).fold(f64::MIN, f64::max));
        if fullmatch(r"[\d۰-۹٠-٩\-–—\s]{1,8}", &text) && (bot < page_h * 0.08 || top > page_h * 0.9) {
            continue;
        }
        lines.push(LLine {
            text,
            top,
            bot,
            x0: g.iter().map(|x| x.x0).fold(f64::MAX, f64::min),
            x1: g.iter().map(|x| x.x1).fold(f64::MIN, f64::max),
            size: sizes[sizes.len() / 2],
        });
    }
    order_side_columns(lines)
}

fn order_side_columns(lines: Vec<LLine>) -> Vec<LLine> {
    let (left, right) = (lines.iter().map(|l| l.x0).fold(f64::MAX, f64::min), lines.iter().map(|l| l.x1).fold(f64::MIN, f64::max));
    let width = right - left;
    if lines.len() < 4 || width <= 0.0 {
        return lines;
    }
    let rtl_page = lines.iter().filter(|l| rtl(&l.text)).count() * 2 > lines.len();
    let narrow = |l: &LLine| l.x1 - l.x0 < 0.6 * width;
    let mut out: Vec<LLine> = Vec::new();
    let mut run: Vec<LLine> = Vec::new();
    let flush = |run: &mut Vec<LLine>, out: &mut Vec<LLine>| {
        let mut best: Option<f64> = None;
        for l in run.iter() {
            let g = l.x1;
            if run.iter().all(|m| m.x1 <= g + 1.0 || m.x0 >= g - 1.0) {
                let (a, b) = (run.iter().filter(|m| m.x1 <= g + 1.0).count(), run.iter().filter(|m| m.x0 >= g - 1.0).count());
                if a >= 2 && b >= 2 && a + b == run.len() {
                    best = Some(g);
                }
            }
        }
        match best {
            Some(g) => {
                let (mut l_col, mut r_col): (Vec<LLine>, Vec<LLine>) = run.drain(..).partition(|m| m.x1 <= g + 1.0);
                if rtl_page {
                    out.append(&mut r_col);
                    out.append(&mut l_col);
                } else {
                    out.append(&mut l_col);
                    out.append(&mut r_col);
                }
            }
            None => out.append(run),
        }
    };
    for l in lines {
        if narrow(&l) {
            run.push(l);
        } else {
            flush(&mut run, &mut out);
            out.push(l);
        }
    }
    flush(&mut run, &mut out);
    out
}

fn is_note_mark(m: &LayoutItem, items: &[&LayoutItem]) -> bool {
    let t = strip(&m.s);
    if !fullmatch(r"[\d۰-۹٠-٩]{1,3}", t) {
        return false;
    }
    let base = |i: &LayoutItem| i.bot - 0.2 * i.size;
    items.iter().any(|w| {
        let gap = (w.x0 - m.x1).max(m.x0 - w.x1);
        let after = if rtl(&w.s) { w.x0 >= m.x1 - 0.2 * w.size } else { w.x1 <= m.x0 + 0.2 * w.size };
        after
            && w.size >= 1.25 * m.size
            && !is_number_piece(&w.s)
            && gap < 0.5 * w.size
            && m.top < w.bot
            && m.bot > w.top
            && base(w) - base(m) > 0.15 * w.size
    })
}

pub fn drop_footnote_lines(lines: &mut Vec<LLine>, page_h: f64) -> Vec<u32> {
    let mut weights: Vec<(f64, usize)> = Vec::new();
    for l in lines.iter() {
        let n = len(&l.text);
        match weights.iter_mut().find(|(k, _)| (*k - l.size).abs() < 0.05) {
            Some(w) => w.1 += n,
            None => weights.push((l.size, n)),
        }
    }
    let Some(&(body, _)) = weights.iter().max_by_key(|w| w.1) else { return Vec::new() };
    let body_bottom = lines.iter().filter(|l| l.size >= body * 0.93).map(|l| l.bot).fold(f64::NEG_INFINITY, f64::max);
    let below = |l: &LLine| l.size < body * 0.93 && l.top >= body_bottom - 2.0;
    let body_lines: Vec<&LLine> = lines.iter().filter(|l| l.size >= body * 0.93).collect();
    let right = body_lines.iter().map(|l| l.x1).fold(f64::NEG_INFINITY, f64::max);
    let left = body_lines.iter().map(|l| l.x0).fold(f64::INFINITY, f64::min);
    let at_margin = |l: &LLine| if rtl(&l.text) { (l.x1 - right).abs() <= 0.3 * body } else { (l.x0 - left).abs() <= 0.3 * body };
    let start = (0..lines.len()).find(|&k| {
        let l = &lines[k];
        below(l) && l.top > 0.5 * page_h && (at_margin(l) || note_number(&l.text).is_some() || k == 0 || l.top - lines[k - 1].bot >= 0.9 * body)
    });
    let mut mid: Vec<usize> = Vec::new();
    let mut k = 0;
    while k < lines.len() {
        if lines[k].size < body * 0.85 && is_match_start(r"\s*(?:\[[\d۰-۹]{1,3}\]|\*)", &lines[k].text) {
            let from = k;
            while k < lines.len() && lines[k].size < body * 0.85 {
                k += 1;
            }
            if lines[k..].iter().any(|l| l.size >= body * 0.93) {
                mid.extend(from..k);
            }
        } else {
            k += 1;
        }
    }
    let mut mid_marks: Vec<u32> = mid.iter().filter_map(|&k| fullmatch_groups(r"\s*\[([\d۰-۹]{1,3})\].*", &lines[k].text).and_then(|g| digits_value(g[1].as_deref()?))).collect();
    for &k in mid.iter().rev() {
        lines.remove(k);
    }
    let start = start.filter(|_| mid.is_empty());
    let Some(start) = start else { return mid_marks };
    let first_top = lines[start].top;
    let is_note = |l: &LLine| below(l) && l.top >= first_top;
    let mut numbers: Vec<u32> = lines.iter().filter(|l| is_note(l)).filter_map(|l| note_number(&l.text)).collect();
    numbers.append(&mut mid_marks);
    if std::env::var("KHANA_PIECES").is_ok() {
        lines.iter().filter(|l| is_note(l)).for_each(|l| eprintln!("footnote line: {}", l.text));
    }
    lines.retain(|l| !is_note(l));
    numbers
}

fn note_number(line: &str) -> Option<u32> {
    let g = fullmatch_groups(r"\s*(?:\.\s*([\d۰-۹٠-٩]{1,3})|([\d۰-۹٠-٩]{1,3})\s*[.)\-]?)(?:\s.*)?", line)?;
    digits_value(g[1].as_deref().or(g[2].as_deref())?)
}

pub fn drop_running_lines(pages: &mut [(Vec<LLine>, f64)]) {
    let key = |t: &str| split_ws(&sub(r"[\d۰-۹٠-٩]+", "", t)).join(" ");
    let edge = |l: &LLine, h: f64| l.bot < h * 0.1 || l.top > h * 0.9;
    let folio = |t: &str| {
        let g = fullmatch_groups(r"\s*(?:(\d{1,4})\s+\D.*|.*\D\s+(\d{1,4}))\s*", t)?;
        let ascii: String = g[1].as_deref().or(g[2].as_deref())?.chars().map(|c| match c {
            '۰'..='۹' => char::from(b'0' + (c as u32 - '۰' as u32) as u8),
            '٠'..='٩' => char::from(b'0' + (c as u32 - '٠' as u32) as u8),
            _ => c,
        }).collect();
        ascii.parse::<i64>().ok()
    };
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut numbered: HashMap<(String, i64), usize> = HashMap::new();
    for (i, (lines, h)) in pages.iter().enumerate() {
        let keys: HashSet<String> = lines.iter().filter(|l| edge(l, *h)).map(|l| key(&l.text)).filter(|k| !k.is_empty()).collect();
        for k in keys {
            *seen.entry(k).or_default() += 1;
        }
        let keys: HashSet<(String, i64)> =
            lines.iter().filter(|l| edge(l, *h)).filter_map(|l| Some((key(&l.text), folio(&l.text)? - i as i64))).filter(|(k, _)| k.chars().any(char::is_alphabetic)).collect();
        for k in keys {
            *numbered.entry(k).or_default() += 1;
        }
    }
    let n = pages.len();
    let mut running: HashSet<String> = seen.into_iter().filter(|(_, c)| *c >= 3 && *c * 4 >= n).map(|(k, _)| k).collect();
    running.extend(numbered.into_iter().filter(|(_, c)| *c >= 2).map(|((k, _), _)| k));
    if running.is_empty() {
        return;
    }
    for (lines, h) in pages.iter_mut() {
        let h = *h;
        lines.retain(|l| !(edge(l, h) && running.contains(&key(&l.text))));
    }
}

pub fn paragraphs_from_lines(lines: &[LLine]) -> Vec<String> {
    let mut widths: Vec<f64> = lines.iter().map(|l| l.x1 - l.x0).collect();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let full = widths.get(widths.len() * 4 / 5).copied().unwrap_or(0.0);
    let bucket = |size: f64| (size * 2.0).round() as i64;
    let mut by_size: HashMap<i64, Vec<f64>> = HashMap::new();
    for w in lines.windows(2) {
        if (w[1].size - w[0].size).abs() > 0.15 * w[0].size.max(w[1].size) {
            continue;
        }
        let g = (w[1].top - w[0].bot) / (w[0].bot - w[0].top).max(w[1].bot - w[1].top).max(1.0);
        if g > 0.0 && g < 1.2 {
            by_size.entry(bucket(w[0].size)).or_default().push(g);
        }
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v[v.len() / 2]
    };
    let mut all: Vec<f64> = by_size.values().flatten().copied().collect();
    let usual_all = if all.len() >= 5 { median(&mut all) } else { 0.0 };
    let usual_by: HashMap<i64, f64> = by_size.iter_mut().filter(|(_, v)| v.len() >= 5).map(|(k, v)| (*k, median(v))).collect();
    let close_gap_for = |size: f64| 0.6f64.max(usual_by.get(&bucket(size)).copied().unwrap_or(usual_all) * 1.3);
    let col_full = |p: &LLine| {
        let mut w: Vec<f64> = lines
            .iter()
            .filter(|l| (l.x1.min(p.x1) - l.x0.max(p.x0)) > 0.5 * (l.x1 - l.x0).min(p.x1 - p.x0))
            .map(|l| l.x1 - l.x0)
            .collect();
        w.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        w.get(w.len() * 4 / 5).copied().unwrap_or(full)
    };
    let mut paras: Vec<String> = Vec::new();
    let mut prev: Option<&LLine> = None;
    for l in lines {
        let heading = |t: &str| notes_heading(t) || contents_heading(t);
        let same_para = !heading(&l.text) && !prev.is_some_and(|p| heading(&p.text)) && prev.is_some_and(|p| {
            let gap = l.top - p.bot;
            let h = (p.bot - p.top).max(l.bot - l.top);
            let same_edges = (p.x0 - l.x0).abs() < 2.0 && (p.x1 - l.x1).abs() < 2.0 && (p.x1 - p.x0) >= 0.45 * full;
            let close = gap < 0.6 * h || (gap < close_gap_for(l.size) * h && ((p.x1 - p.x0) >= 0.75 * col_full(p) || same_edges) && rtl(&p.text) == rtl(&l.text));
            let same_size = (l.size - p.size).abs() <= 0.15 * p.size.max(l.size);
            let ended = (p.x1 - p.x0) < 0.75 * full && !same_edges && search(r"[.!?؟:»)\]…]$", &p.text);
            let label = search(r"^\s*(?:Keywords|Key words|Abstract|Title|کلیدواژه(?:\x{200c}?ها)?|کلید واژه(?:\x{200c}?ها)?|واژگان کلیدی|چکیده)\s*:", &l.text);
            let turn = search(r#"[.!?؟…]\s*[»"”)\]]?\s*$"#, &p.text) && search(PARAGRAPH_OPENER, &l.text);
            close && same_size && !ended && !label && !turn
        });
        match (same_para, paras.last_mut()) {
            (true, Some(last)) => {
                if search(r"[A-Za-z]-$", last) && is_match_start(r"[a-z]", &l.text) {
                    last.pop();
                } else {
                    last.push(' ');
                }
                last.push_str(&l.text);
            }
            _ => paras.push(l.text.clone()),
        }
        prev = Some(l);
    }
    paras
}






