//! Persian human-like prosody: sentence-type recognition (Yes/No questions, WH-questions,
//! exclamations, ellipses, continuation clauses, headings, statements), independent pitch
//! contour + phrase-final lengthening ("stopping slowly"), and natural breath dynamics.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    YesNoQuestion,
    WhQuestion,
    Exclaim,
    Ellipsis,
    Continues,
    Title,
    Statement,
}

const WH_WORDS: &[&str] = &[
    "چرا", "کجا", "کجاست", "چگونه", "چطور", "چه", "چه‌طور", "چه‌گونه",
    "کی", "کیست", "چیست", "کدام", "چند", "چقدر", "چه‌قدر", "چه‌کسی", "چه‌چیزی",
];

const POLAR_WORDS: &[&str] = &["آیا", "مگر", "نکند", "یعنی"];

fn has_word(text: &str, list: &[&str]) -> bool {
    text.split_whitespace().any(|raw| {
        let clean = raw.trim_matches(|c: char| "«»\"'’”()[]،؛,:؟?!.…".contains(c));
        list.contains(&clean)
    })
}

pub fn kind_of(text: &str) -> Kind {
    let t = text.trim().trim_end_matches(|c: char| "»”\"'’)]".contains(c)).trim_end();
    if t.ends_with("...") || t.ends_with('…') {
        return Kind::Ellipsis;
    }
    match t.chars().last() {
        Some('؟' | '?') => {
            if has_word(t, POLAR_WORDS) {
                Kind::YesNoQuestion
            } else if has_word(t, WH_WORDS) {
                Kind::WhQuestion
            } else {
                Kind::YesNoQuestion
            }
        }
        Some('!') => Kind::Exclaim,
        Some('،' | ',' | '؛' | ';' | ':' | '—' | '–') => Kind::Continues,
        Some('.') => Kind::Statement,
        Some(_) if t.split_whitespace().count() <= 10 && t.chars().count() <= 70 => Kind::Title,
        Some(_) => Kind::Statement,
        None => Kind::Statement,
    }
}

pub fn gap_after(kind: Kind, usual: f64) -> f64 {
    match kind {
        Kind::YesNoQuestion | Kind::WhQuestion | Kind::Exclaim => usual + 0.12,
        Kind::Ellipsis => usual + 0.16,
        Kind::Continues => usual * 0.55,
        Kind::Title => usual + 0.24,
        Kind::Statement => usual,
    }
}

pub fn pace(kind: Kind) -> f64 {
    match kind {
        Kind::Title => 0.92,
        _ => 1.0,
    }
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

/// Applies independent pitch (`pitch_fn`) and speaking-tempo (`tempo_fn` scaled by `base_speed`)
/// contours plus a gentle human breath/dynamics envelope (`gain_fn`) in a single TD-PSOLA pass.
fn shape_contours(
    x: Vec<f32>,
    sample_rate: usize,
    base_speed: f64,
    pitch_fn: impl Fn(f64, f64) -> f64,
    tempo_fn: impl Fn(f64, f64) -> f64,
    gain_fn: impl Fn(f64, f64) -> f32,
) -> Vec<f32> {
    let base_speed = base_speed.clamp(0.5, 2.0);
    if x.len() < sample_rate / 4 {
        return if (base_speed - 1.0).abs() >= 0.008 { crate::stretch::stretch(&x, base_speed, sample_rate) } else { x };
    }
    // Step 1: Time-stretch by (tempo * base_speed / pitch) without changing pitch.
    // Step 2: Resample via varispeed by (pitch), shifting pitch to `pitch` while net tempo becomes
    //         (tempo * base_speed / pitch) * pitch = tempo * base_speed in one pass!
    let stretched = crate::stretch::stretch_curve(&x, sample_rate, |t, len| {
        let p = pitch_fn(t, len).clamp(0.82, 1.24);
        let spd = (tempo_fn(t, len).clamp(0.76, 1.15) * base_speed).clamp(0.45, 2.2);
        spd / p
    });
    let mut shaped = varispeed(&stretched, sample_rate, |t, len| {
        pitch_fn(t, len).clamp(0.82, 1.24)
    });
    let total = shaped.len() as f64 / sample_rate as f64;
    if total > 0.05 {
        for (idx, s) in shaped.iter_mut().enumerate() {
            let t = idx as f64 / sample_rate as f64;
            *s = (*s * gain_fn(t, total)).clamp(-0.98, 0.98);
        }
    }
    shaped
}

/// Shapes a full sentence's intonation, phrase-final lengthening, and breath dynamics.
pub fn shape(x: Vec<f32>, sample_rate: usize, kind: Kind) -> Vec<f32> {
    shape_with_speed(x, sample_rate, kind, 1.0)
}

/// Shapes a full sentence's intonation and applies overall `speed` in a single time-scale pass.
pub fn shape_with_speed(x: Vec<f32>, sample_rate: usize, kind: Kind, speed: f64) -> Vec<f32> {
    let base_speed = (speed * pace(kind)).clamp(0.5, 2.0);
    if x.len() < sample_rate / 3 {
        return if (base_speed - 1.0).abs() >= 0.008 { crate::stretch::stretch(&x, base_speed, sample_rate) } else { x };
    }
    match kind {
        Kind::YesNoQuestion => {
            // Persian polar question: unmistakable smooth pitch rise (+13%) over final 0.55s
            // paired with gentle final slowing (0.92x) instead of rushing the last word.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let tail = 0.55f64.min(len * 0.48);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 + 0.13 * w
                },
                |t, len| {
                    let tail = 0.50f64.min(len * 0.42);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.08 * w
                },
                |t, len| {
                    let tail = 0.45f64.min(len * 0.40);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 + 0.04 * w as f32
                },
            )
        }
        Kind::WhQuestion => {
            // Persian WH-question: subtle expressive focus lift early-mid (+4.5%) + inquiring
            // rising-suspended end (+7%) with gentle slowing (0.91x).
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let focus = (-((t / len.max(0.2) - 0.25) / 0.22).powi(2)).exp();
                    let tail = 0.46f64.min(len * 0.42);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 + 0.045 * focus + 0.07 * w
                },
                |t, len| {
                    let tail = 0.45f64.min(len * 0.40);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.09 * w
                },
                |t, len| {
                    let focus = (-((t / len.max(0.2) - 0.25) / 0.25).powi(2)).exp();
                    1.0 + 0.04 * focus as f32
                },
            )
        }
        Kind::Exclaim => {
            // Engaged bright onset (+4% pitch, +7% energy) landing with grounded weight.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let head = 1.0 - smooth(t / 0.48f64.min(len * 0.4));
                    let tail = 0.38f64.min(len * 0.35);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 + 0.045 * head - 0.04 * w
                },
                |t, len| {
                    let tail = 0.38f64.min(len * 0.35);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.08 * w
                },
                |t, len| {
                    let head = 1.0 - smooth(t / 0.50f64.min(len * 0.45));
                    1.0 + 0.07 * head as f32
                },
            )
        }
        Kind::Ellipsis => {
            // Trailing thought: noticeable thoughtful slowing (0.84x), level-soft pitch, gentle breath fade.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let tail = 0.60f64.min(len * 0.50);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.02 * w
                },
                |t, len| {
                    let tail = 0.60f64.min(len * 0.50);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.16 * w
                },
                |t, len| {
                    let tail = 0.50f64.min(len * 0.45);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.16 * w as f32
                },
            )
        }
        Kind::Continues => {
            // Continuation clause (comma/semicolon/colon): gentle upward lift (+5%) + slight breath ease
            // so the listener immediately hears more of the sentence is coming.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let tail = 0.32f64.min(len * 0.35);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 + 0.05 * w
                },
                |t, len| {
                    let tail = 0.30f64.min(len * 0.32);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.05 * w
                },
                |_, _| 1.0,
            )
        }
        Kind::Title => {
            // Calm, grounded chapter/heading ending.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let tail = 0.38f64.min(len * 0.35);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.04 * w
                },
                |t, len| {
                    let tail = 0.38f64.min(len * 0.35);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.06 * w
                },
                |t, len| {
                    let tail = 0.28f64.min(len * 0.30);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.08 * w as f32
                },
            )
        }
        Kind::Statement => {
            // Natural Persian narrator cadence: warm declination (-6% pitch) + clear phrase-final
            // slowing down (0.88x) + gentle breath taper so endings stop smoothly, never abruptly.
            shape_contours(
                x,
                sample_rate,
                base_speed,
                |t, len| {
                    let tail = 0.52f64.min(len * 0.42);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.06 * w
                },
                |t, len| {
                    let tail = 0.52f64.min(len * 0.42);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.12 * w
                },
                |t, len| {
                    let tail = 0.30f64.min(len * 0.30);
                    let w = smooth((t - (len - tail)) / tail);
                    1.0 - 0.12 * w as f32
                },
            )
        }
    }
}

/// Subtle continuation cue applied to internal comma/colon clauses inside a multi-clause sentence.
pub fn shape_clause(x: Vec<f32>, sample_rate: usize) -> Vec<f32> {
    if x.len() < sample_rate / 3 {
        return x;
    }
    shape_contours(
        x,
        sample_rate,
        1.0,
        |t, len| {
            let tail = 0.24f64.min(len * 0.32);
            let w = smooth((t - (len - tail)) / tail);
            1.0 + 0.04 * w
        },
        |t, len| {
            let tail = 0.24f64.min(len * 0.32);
            let w = smooth((t - (len - tail)) / tail);
            1.0 - 0.04 * w
        },
        |_, _| 1.0,
    )
}
