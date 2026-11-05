
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Question,
    Exclaim,
    Continues,
    Title,
    Statement,
}

pub fn kind_of(text: &str) -> Kind {
    let t = text.trim().trim_end_matches(|c: char| "»”\"'’)]".contains(c)).trim_end();
    match t.chars().last() {
        Some('؟' | '?') => Kind::Question,
        Some('!') => Kind::Exclaim,
        Some('،' | ',' | '؛' | ';' | ':') => Kind::Continues,
        Some('.' | '…') => Kind::Statement,
        Some(_) if t.split_whitespace().count() <= 10 && t.chars().count() <= 70 => Kind::Title,
        Some(_) => Kind::Statement,
        None => Kind::Statement,
    }
}

pub fn gap_after(kind: Kind, usual: f64) -> f64 {
    match kind {
        Kind::Question | Kind::Exclaim => usual + 0.15,
        Kind::Continues => usual * 0.6,
        Kind::Title => usual + 0.35,
        Kind::Statement => usual,
    }
}

pub fn pace(kind: Kind) -> f64 {
    if kind == Kind::Title { 0.92 } else { 1.0 }
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn varispeed(x: &[f32], sample_rate: usize, rate: impl Fn(f64, f64) -> f64) -> Vec<f32> {
    if x.len() < 2 {
        return x.to_vec();
    }
    let length = x.len() as f64 / sample_rate as f64;
    let mut out = Vec::with_capacity(x.len() + x.len() / 8);
    let mut pos = 0f64;
    while pos < (x.len() - 1) as f64 {
        let i = pos as usize;
        let f = (pos - i as f64) as f32;
        out.push(x[i] * (1.0 - f) + x[i + 1] * f);
        pos += rate(pos / sample_rate as f64, length).clamp(0.5, 2.0);
    }
    out
}

pub fn shape(x: Vec<f32>, sample_rate: usize, kind: Kind) -> Vec<f32> {
    if x.len() < sample_rate / 2 {
        return x;
    }
    let (tail, end) = match kind {
        Kind::Question => (0.50, 1.18),
        Kind::Exclaim => (0.40, 0.95),
        Kind::Continues => (0.35, 1.10),
        Kind::Title => (0.35, 0.96),
        Kind::Statement => (0.40, 0.93),
    };
    let start = if kind == Kind::Exclaim { 1.04 } else { 1.0 };
    varispeed(&x, sample_rate, |t, len| {
        let at_start = 1.0 + (start - 1.0) * (1.0 - smooth(t / 0.5));
        let into_tail = smooth((t - (len - tail)) / tail);
        at_start + (end - 1.0) * into_tail
    })
}

