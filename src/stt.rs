
use crate::ffi::ct2::Whisper;
use crate::pyre::sub;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const RATE: usize = 16000;
const N_FFT: usize = 400;
const HOP: usize = 160;
const N_MELS: usize = 128;
const CHUNK: usize = 30 * RATE;
const FRAMES: usize = 3000;

pub fn model_dir() -> PathBuf {
    crate::vocab::models_dir().join("whisper")
}

pub fn available() -> bool {
    cfg!(not(any(shell, windows))) && model_dir().join("model.bin").is_file() && model_dir().join("vocabulary.json").is_file()
}


fn mel_filters() -> Vec<f32> {
    let bins = N_FFT / 2 + 1;
    let hz_to_mel = |f: f64| if f < 1000.0 { f / (200.0 / 3.0) } else { 15.0 + (f / 1000.0).ln() / (6.4f64.ln() / 27.0) };
    let mel_to_hz = |m: f64| if m < 15.0 { m * (200.0 / 3.0) } else { 1000.0 * ((m - 15.0) * (6.4f64.ln() / 27.0)).exp() };
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(8000.0));
    let mel_f: Vec<f64> = (0..N_MELS + 2).map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (N_MELS + 1) as f64)).collect();
    let freqs: Vec<f64> = (0..bins).map(|i| 8000.0 * i as f64 / (bins - 1) as f64).collect();
    let mut w = vec![0f32; N_MELS * bins];
    for m in 0..N_MELS {
        let (f0, f1, f2) = (mel_f[m], mel_f[m + 1], mel_f[m + 2]);
        let enorm = 2.0 / (f2 - f0);
        for (k, f) in freqs.iter().enumerate() {
            let lower = (f - f0) / (f1 - f0);
            let upper = (f2 - f) / (f2 - f1);
            w[m * bins + k] = (lower.min(upper).max(0.0) * enorm) as f32;
        }
    }
    w
}

struct Tables {
    filters: Vec<f32>,
    cos: Vec<f32>,
    sin: Vec<f32>,
    window: Vec<f32>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let bins = N_FFT / 2 + 1;
        let mut cos = vec![0f32; bins * N_FFT];
        let mut sin = vec![0f32; bins * N_FFT];
        for k in 0..bins {
            for n in 0..N_FFT {
                let a = 2.0 * std::f64::consts::PI * ((k * n) % N_FFT) as f64 / N_FFT as f64;
                cos[k * N_FFT + n] = a.cos() as f32;
                sin[k * N_FFT + n] = a.sin() as f32;
            }
        }
        let window = (0..N_FFT).map(|n| (0.5 - 0.5 * (2.0 * std::f64::consts::PI * n as f64 / N_FFT as f64).cos()) as f32).collect();
        Tables { filters: mel_filters(), cos, sin, window }
    })
}

pub fn log_mel(piece: &[f32]) -> Vec<f32> {
    let t = tables();
    let bins = N_FFT / 2 + 1;
    let mut a = vec![0f32; CHUNK];
    let n = piece.len().min(CHUNK);
    a[..n].copy_from_slice(&piece[..n]);
    let pad = N_FFT / 2;
    let mut p = Vec::with_capacity(a.len() + 2 * pad);
    for i in 0..pad {
        p.push(a[pad - i]);
    }
    p.extend_from_slice(&a);
    for i in 0..pad {
        p.push(a[a.len() - 2 - i]);
    }
    let frames_all = (p.len() - N_FFT) / HOP + 1;
    let frames = frames_all - 1;
    debug_assert_eq!(frames, FRAMES);
    let mut power = vec![0f32; bins * frames];
    let mut buf = vec![0f32; N_FFT];
    for f in 0..frames {
        for n in 0..N_FFT {
            buf[n] = p[f * HOP + n] * t.window[n];
        }
        for k in 0..bins {
            let (c, s) = (&t.cos[k * N_FFT..(k + 1) * N_FFT], &t.sin[k * N_FFT..(k + 1) * N_FFT]);
            let (mut re, mut im) = (0f32, 0f32);
            for n in 0..N_FFT {
                re += buf[n] * c[n];
                im -= buf[n] * s[n];
            }
            power[k * frames + f] = re * re + im * im;
        }
    }
    let mut mel = vec![0f32; N_MELS * frames];
    for m in 0..N_MELS {
        let w = &t.filters[m * bins..(m + 1) * bins];
        for f in 0..frames {
            let mut s = 0f32;
            for k in 0..bins {
                s += w[k] * power[k * frames + f];
            }
            mel[m * frames + f] = s.max(1e-10).log10();
        }
    }
    let top = mel.iter().cloned().fold(f32::MIN, f32::max);
    for v in mel.iter_mut() {
        *v = (v.max(top - 8.0) + 4.0) / 4.0;
    }
    mel
}


fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        0.0
    } else {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}

pub fn pieces(a: &[f32]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let win = RATE / 10;
    while start < a.len() {
        if a.len() - start <= CHUNK {
            out.push((start, a.len()));
            break;
        }
        let (from, to) = (start + 18 * RATE, start + CHUNK - win);
        let mut best = (f32::MAX, to);
        let mut i = from;
        while i <= to {
            let e = rms(&a[i..i + win]);
            if e < best.0 {
                best = (e, i + win / 2);
            }
            i += win / 2;
        }
        out.push((start, best.1));
        start = best.1;
    }
    out
}


fn byte_of_char() -> std::collections::HashMap<char, u8> {
    let mut keep: Vec<u32> = (b'!' as u32..=b'~' as u32).chain(0xA1..=0xAC).chain(0xAE..=0xFF).collect();
    let mut map = std::collections::HashMap::new();
    for b in keep.iter() {
        map.insert(char::from_u32(*b).unwrap(), *b as u8);
    }
    let mut n = 0u32;
    for b in 0u32..256 {
        if !keep.contains(&b) {
            map.insert(char::from_u32(256 + n).unwrap(), b as u8);
            keep.push(b);
            n += 1;
        }
    }
    map
}

pub fn tokens_to_text(tokens: &str) -> String {
    static M: OnceLock<std::collections::HashMap<char, u8>> = OnceLock::new();
    let m = M.get_or_init(byte_of_char);
    let mut bytes = Vec::new();
    for t in tokens.split(' ').filter(|t| !t.is_empty() && !t.starts_with("<|")) {
        for c in t.chars() {
            if let Some(b) = m.get(&c) {
                bytes.push(*b);
            }
        }
    }
    clean(&String::from_utf8_lossy(&bytes))
}

pub fn clean(t: &str) -> String {
    let t: String = t.chars().map(|c| match c {
        'ي' => 'ی',
        'ك' => 'ک',
        'ە' => 'ه',
        _ => c,
    }).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = sub(r"(?<![\p{L}])(نمی|می) (?=[\u0600-\u06FF])", "\\1\u{200c}", &t);
    sub(r"(?<=[\u0600-\u06FF]) (ها|های|هایی|ترین|تری)(?![\p{L}])", "\u{200c}\\1", &t)
}

pub fn drop_loops(t: &str) -> String {
    let w: Vec<&str> = t.split_whitespace().collect();
    for n in 1..=8usize {
        if w.len() < n * 4 {
            continue;
        }
        let mut i = 0;
        while i + n * 4 <= w.len() {
            let unit = &w[i..i + n];
            let k = w[i..].chunks_exact(n).take_while(|c| *c == unit).count();
            if k >= 4 {
                let mut out: Vec<&str> = w[..i + n].to_vec();
                out.extend_from_slice(&w[i + k * n..]);
                return drop_loops(&out.join(" "));
            }
            i += 1;
        }
    }
    t.to_string()
}



struct Bpe {
    rank: std::collections::HashMap<(String, String), usize>,
    to_char: Vec<char>,
}

fn bpe() -> Option<&'static Bpe> {
    static B: OnceLock<Option<Bpe>> = OnceLock::new();
    B.get_or_init(|| {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(model_dir().join("tokenizer.json")).ok()?).ok()?;
        let mut rank = std::collections::HashMap::new();
        for (i, m) in v["model"]["merges"].as_array()?.iter().enumerate() {
            let (a, b) = match m {
                serde_json::Value::String(s) => {
                    let (a, b) = s.split_once(' ')?;
                    (a.to_string(), b.to_string())
                }
                serde_json::Value::Array(p) => (p.first()?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()),
                _ => return None,
            };
            rank.insert((a, b), i);
        }
        let mut to_char = vec!['\0'; 256];
        for (c, b) in byte_of_char() {
            to_char[b as usize] = c;
        }
        Some(Bpe { rank, to_char })
    })
    .as_ref()
}

pub fn encode(text: &str) -> Vec<String> {
    let Some(b) = bpe() else { return Vec::new() };
    let re = crate::pyre::rx(r"'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+");
    let mut out = Vec::new();
    for m in re.find_iter(text).flatten() {
        let mut parts: Vec<String> = m.as_str().bytes().map(|x| b.to_char[x as usize].to_string()).collect();
        loop {
            let best = (0..parts.len().saturating_sub(1))
                .filter_map(|i| b.rank.get(&(parts[i].clone(), parts[i + 1].clone())).map(|r| (*r, i)))
                .min();
            let Some((_, i)) = best else { break };
            let merged = format!("{}{}", parts[i], parts[i + 1]);
            parts.splice(i..=i + 1, [merged]);
        }
        out.extend(parts);
    }
    out
}

const STYLE: &str = "این متن به زبان فارسی معیار و رسمی نوشته می\u{200c}شود، با نشانه\u{200c}گذاری درست و نیم\u{200c}فاصله: می\u{200c}خواهم، کتاب\u{200c}ها.";


#[derive(Default, Clone)]
pub struct Status {
    pub state: String,
    pub done: usize,
    pub total: usize,
    pub text: String,
    pub error: String,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static CANCEL: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

fn set(f: impl FnOnce(&mut Status)) {
    let mut g = STATUS.lock().unwrap();
    let s = g.get_or_insert_with(Status::default);
    f(s);
}

pub fn status() -> Status {
    STATUS.lock().unwrap().clone().unwrap_or(Status { state: "idle".into(), ..Default::default() })
}

pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub fn start(audio: Vec<f32>, lang: &str) -> Result<(), String> {
    if !available() {
        return Err("مدل نوشتن گفتار در این نسخه نیست.".into());
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("یک نوشتن دیگر در حال انجام است.".into());
    }
    CANCEL.store(false, Ordering::SeqCst);
    set(|s| *s = Status { state: "loading".into(), ..Default::default() });
    let lang = lang.to_string();
    let audio = Arc::new(audio);
    std::thread::spawn(move || {
        let r = run(&audio, &lang);
        match r {
            Ok(()) => {}
            Err(e) => set(|s| {
                s.state = "error".into();
                s.error = e;
            }),
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn model() -> Result<&'static Mutex<Whisper>, String> {
    static M: OnceLock<Mutex<Whisper>> = OnceLock::new();
    if let Some(m) = M.get() {
        return Ok(m);
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let w = Whisper::load(&model_dir(), threads)?;
    let _ = M.set(Mutex::new(w));
    Ok(M.get().unwrap())
}

fn run(audio: &[f32], lang: &str) -> Result<(), String> {
    let vocab: Vec<String> = serde_json::from_slice(&std::fs::read(model_dir().join("vocabulary.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let tag = format!("<|{lang}|>");
    if !vocab.iter().any(|t| *t == tag) {
        return Err("این زبان پشتیبانی نمی‌شود.".into());
    }
    let start = format!("<|startoftranscript|> {tag} <|transcribe|> <|notimestamps|>");
    let style = if lang == "fa" && std::env::var("KHANA_STT_NOSTYLE").is_err() { encode(STYLE) } else { Vec::new() };
    let prompt = if style.is_empty() { start } else { format!("<|startofprev|> {} {start}", style.join(" ")) };
    let model = model()?;
    let parts = pieces(audio);
    set(|s| {
        s.state = "working".into();
        s.total = parts.len();
    });
    let mut text = String::new();
    for (i, (a, b)) in parts.iter().enumerate() {
        if CANCEL.load(Ordering::SeqCst) {
            set(|s| s.state = "done".into());
            return Ok(());
        }
        let mut piece = audio[*a..*b].to_vec();
        let peak = piece.iter().fold(0f32, |m, x| m.max(x.abs()));
        if rms(&piece) > 0.002 && peak > 0.01 {
            if peak < 0.5 {
                let g = 0.8 / peak;
                piece.iter_mut().for_each(|x| *x *= g);
            }
            let mel = log_mel(&piece);
            let out = model.lock().unwrap().transcribe(&mel, N_MELS, FRAMES, &prompt, 5, 400);
            let Some(out) = out else { return Err("نوشتن گفتار ناموفق بود.".into()) };
            let t = drop_loops(&tokens_to_text(&out));
            if !t.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t);
            }
        }
        let snapshot = text.clone();
        set(|s| {
            s.done = i + 1;
            s.text = snapshot;
        });
    }
    set(|s| s.state = "done".into());
    Ok(())
}

