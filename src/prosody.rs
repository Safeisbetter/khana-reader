
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
    // Keep the final phoneme at its natural rate and pitch. A varispeed tail
    // changes both together and can make the last letter sound unnaturally
    // low, stretched, or synthetic.
    let start = if kind == Kind::Exclaim { 1.04 } else { 1.0 };
    varispeed(&x, sample_rate, |t, _len| {
        1.0 + (start - 1.0) * (1.0 - smooth(t / 0.5))
    })
}

