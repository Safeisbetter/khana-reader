
use crate::audio;
use crate::doc::{open_document, Document, Engines, Opts};
use crate::pyre::*;
use crate::tts::{self, Engine};
use crate::vocab::models_dir;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const APP_NAME: &str = "Khana Reader Rust";

pub struct VoiceInfo {
    pub id: &'static str,
    pub wav: &'static str,
    pub name: &'static str,
    pub gender: &'static str,
    pub group: &'static str,
    pub extra: bool,
    pub tempo: f64,
}

pub const VOICES: &[VoiceInfo] = &[
    VoiceInfo { id: "khabari", wav: "male_news.wav", name: "آقا، خبری", gender: "مرد", group: "natural", extra: false, tempo: 0.92 },
];

pub fn data_dir() -> PathBuf {
    crate::platform::data_dir()
}

fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn my_voice() -> PathBuf {
    data_dir().join("my_voice.wav")
}

pub fn save_dir() -> PathBuf {
    crate::platform::save_dir()
}

fn has_engine() -> bool {
    models_dir().join("parsigo/model/onnx/flow_lm_step.onnx").is_file()
}

fn voice_path(v: &VoiceInfo) -> PathBuf {
    tts::voice_file(models_dir(), v.extra, v.wav)
}

fn voices() -> Vec<&'static VoiceInfo> {
    VOICES.iter().filter(|v| voice_path(v).is_file() && has_engine()).collect()
}

fn all_voices() -> Vec<(String, String, String, String)> {
    let mut v: Vec<_> = voices().iter().map(|v| (v.id.to_string(), v.name.to_string(), v.gender.to_string(), v.group.to_string())).collect();
    if has_engine() && my_voice().is_file() {
        v.push(("mine".into(), "صدای من".into(), "خودم".into(), "mine".into()));
    }
    v
}

fn default_settings() -> Map<String, Value> {
    let first = voices().first().map(|v| v.id).unwrap_or("");
    let v = json!({"voice": first, "speed": 1.0, "fontSize": 21, "reversed": false, "persianYeh": true, "autoNext": true,
                   "bitrate": 64, "lexicon": {}, "autoPlay": true, "highlight": "yellow", "translate": true, "naturalTone": true});
    v.as_object().unwrap().clone()
}

pub fn load_settings() -> Map<String, Value> {
    let mut s = default_settings();
    if let Ok(t) = std::fs::read_to_string(settings_file()) {
        if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&t) {
            for (k, v) in m {
                s.insert(k, v);
            }
        }
    }
    let voice = s.get("voice").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if !all_voices().iter().any(|v| v.0 == voice) {
        let d = default_settings();
        s.insert("voice".into(), d["voice"].clone());
    }
    s
}

fn save_settings(new: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    let mut s = load_settings();
    for k in default_settings().keys() {
        if let Some(v) = new.get(k) {
            s.insert(k.clone(), v.clone());
        }
    }
    let lex = match s.get("lexicon") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    let mut clean = Map::new();
    for (w, p) in lex.iter().take(2000) {
        let ps = match p {
            Value::String(x) => x.clone(),
            other => other.to_string(),
        };
        if !strip(w).is_empty() {
            clean.insert(w.chars().take(60).collect(), Value::String(ps.chars().take(120).collect()));
        }
    }
    s.insert("lexicon".into(), Value::Object(clean));
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(settings_file(), serde_json::to_string_pretty(&Value::Object(s.clone())).unwrap()).map_err(|e| e.to_string())?;
    Ok(s)
}

fn lexicon_pairs(s: &Map<String, Value>) -> Vec<(String, String)> {
    match s.get("lexicon") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.as_str().map(|x| x.to_string()).unwrap_or_else(|| v.to_string()))).collect(),
        _ => Vec::new(),
    }
}


pub struct Speech {
    engine: Mutex<Option<Arc<Engine>>>,
    busy: Mutex<()>,
    last_play: Mutex<Option<Instant>>,
}

pub fn speech() -> &'static Speech {
    static S: OnceLock<Speech> = OnceLock::new();
    S.get_or_init(|| Speech { engine: Mutex::new(None), busy: Mutex::new(()), last_play: Mutex::new(None) })
}

impl Speech {
    pub fn engine(&self) -> Result<Arc<Engine>, String> {
        let mut e = self.engine.lock().unwrap();
        if e.is_none() {
            *e = Some(Arc::new(Engine::new(&models_dir().join("parsigo/model"))?));
        }
        Ok(e.as_ref().unwrap().clone())
    }

    fn mark_play(&self) {
        *self.last_play.lock().unwrap() = Some(Instant::now());
    }

    pub fn synthesize(&self, text: &str, voice_id: &str, speed: f64, cancel: &(dyn Fn() -> bool + Sync)) -> Result<(Vec<f32>, usize), String> {
        let all = voices();
        let (id, wav, tempo) = if voice_id == "mine" && my_voice().is_file() {
            ("mine".to_string(), my_voice(), 1.0)
        } else {
            let v = all.iter().find(|v| v.id == voice_id).or(all.first()).ok_or("no voices")?;
            (v.id.to_string(), voice_path(v), v.tempo)
        };
        let speed = (speed.clamp(0.5, 2.0) * tempo).clamp(0.5, 2.0);
        let eng = self.engine()?;
        let text = tts::speech_clean(text);
        let _g = self.busy.lock().unwrap();
        let settings = load_settings();
        eng.g2p.set_lexicon(&lexicon_pairs(&settings));
        eng.natural.store(settings.get("naturalTone").and_then(|v| v.as_bool()).unwrap_or(true), Ordering::Relaxed);
        let recent = self.last_play.lock().unwrap().map_or(false, |t| t.elapsed() < Duration::from_secs(30));
        let workers = if recent { 3 } else { 5 };
        let mut a = eng.synthesize_text(&text, &id, &wav, workers, cancel)?;
        if (speed - 1.0).abs() >= 0.03 && !a.is_empty() {
            a = crate::stretch::stretch(&a, speed, tts::RATE);
        }
        Ok((audio::fade_edges(a, tts::RATE, 8), tts::RATE))
    }
}


fn make_my_voice(data: &[u8], ext: &str) -> Result<f64, String> {
    let tmp = std::env::temp_dir().join(format!("khana-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let src = tmp.join(format!("input{ext}"));
    let dst = tmp.join("voice.wav");
    let res = (|| {
        std::fs::write(&src, data).map_err(|e| e.to_string())?;
        if !crate::platform::to_wav24k(&src, &dst) {
            return Err("این فایل صوتی باز نشد. یک فایل WAV، M4A یا MP3 انتخاب کنید.".to_string());
        }
        audio::read_wav(&dst)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    let (a, rate) = res?;
    let rate = rate as usize;
    if a.is_empty() || a.iter().fold(0f32, |m, x| m.max(x.abs())) < 0.01 {
        return Err("در این ضبط صدایی شنیده نمی\u{200c}شود. میکروفون را بررسی کنید.".into());
    }
    let f = (rate as f64 * 0.02) as usize;
    let frames: Vec<f64> = (0..=(a.len().saturating_sub(f)) / f).map(|i| audio::rms(&a[i * f..(i * f + f).min(a.len())])).collect();
    let mut sorted = frames.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let pos = 0.1 * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    let p10 = sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64);
    let floor = f64::max(p10 * 3.0, sorted.last().copied().unwrap_or(0.0) * 0.05);
    let mut keep: Vec<f32> = Vec::new();
    let mut quiet = 0;
    let mut any = false;
    for (i, fr) in frames.iter().enumerate() {
        let on = *fr > floor;
        quiet = if on { 0 } else { quiet + 1 };
        if on || (quiet <= 12 && any) {
            keep.extend_from_slice(&a[i * f..(i * f + f).min(a.len())]);
            any = true;
        }
    }
    if (keep.len() as f64) < rate as f64 * 2.5 {
        return Err("ضبط خیلی کوتاه است. دست\u{200c}کم ۵ ثانیه پشت سر هم صحبت کنید.".into());
    }
    keep.truncate(rate * 5);
    let peak = keep.iter().fold(0f32, |m, x| m.max(x.abs()));
    for x in keep.iter_mut() {
        *x *= 0.6 / peak;
    }
    let n = (rate as f64 * 0.02) as usize;
    let up = audio::linspace(0.0, 1.0, n);
    let down = audio::linspace(1.0, 0.0, n);
    let len = keep.len();
    for i in 0..n {
        keep[i] *= up[i];
        keep[len - n + i] *= down[i];
    }
    std::fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    std::fs::write(my_voice(), audio::wav_bytes(&keep, rate as u32)).map_err(|e| e.to_string())?;
    if let Ok(e) = speech().engine() {
        e.forget_pace("mine");
    }
    Ok((keep.len() as f64 / rate as f64 * 10.0).round() / 10.0)
}


pub struct Job {
    pub id: String,
    pub filename: String,
    status: Mutex<String>,
    error: Mutex<String>,
    done_chars: Mutex<usize>,
    total_chars: Mutex<usize>,
    seconds: Mutex<f64>,
    cancelled: AtomicBool,
    pub path: Mutex<Option<PathBuf>>,
    pub saved: Mutex<Option<PathBuf>>,
}

impl Job {
    fn new(filename: String) -> Job {
        Job {
            id: new_token()[..32].to_string(),
            filename,
            status: Mutex::new("running".into()),
            error: Mutex::new(String::new()),
            done_chars: Mutex::new(0),
            total_chars: Mutex::new(1),
            seconds: Mutex::new(0.0),
            cancelled: AtomicBool::new(false),
            path: Mutex::new(None),
            saved: Mutex::new(None),
        }
    }

    fn finish(&self, res: Result<(), String>) {
        match res {
            Ok(()) => *self.status.lock().unwrap() = "done".into(),
            Err(e) if e == "cancelled" => {
                *self.status.lock().unwrap() = "cancelled".into();
                self.cleanup();
            }
            Err(e) => {
                *self.status.lock().unwrap() = "error".into();
                *self.error.lock().unwrap() = e;
                self.cleanup();
            }
        }
    }

    fn info(&self) -> Value {
        let done = *self.done_chars.lock().unwrap() as f64;
        let total = (*self.total_chars.lock().unwrap()).max(1) as f64;
        json!({"id": self.id, "status": *self.status.lock().unwrap(), "error": *self.error.lock().unwrap(),
               "progress": (done / total).min(1.0), "seconds": self.seconds.lock().unwrap().round() as i64,
               "filename": self.filename,
               "saved": self.saved.lock().unwrap().as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()})
    }

    fn cleanup(&self) {
        if let Some(p) = self.path.lock().unwrap().take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn unique_path(p: PathBuf) -> PathBuf {
    let mut p = p;
    let mut n = 2;
    while p.exists() {
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        let base = stem.rsplit_once(" (").map(|x| x.0.to_string()).unwrap_or(stem);
        let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        p = p.with_file_name(format!("{base} ({n}){ext}"));
        n += 1;
    }
    p
}

pub const END_WORD: &str = "پایان کتاب.";

struct Mp3Req {
    doc: Arc<Document>,
    first: usize,
    last: usize,
    voice: String,
    speed: f64,
    bitrate: u32,
    opts: Opts,
}

fn run_job(job: Arc<Job>, r: Mp3Req) {
    let path = std::env::temp_dir().join(format!("khana-{}.mp3", job.id));
    *job.path.lock().unwrap() = Some(path.clone());
    let _awake = crate::platform::awake(c"Khana Reader is saving an MP3");
    let res = (|| -> Result<(), String> {
        let mut paras = Vec::new();
        for n in r.first..=r.last {
            paras.extend(r.doc.page_text(n, r.opts));
        }
        if r.first == 1 && !paras.is_empty() {
            paras.insert(0, format!("{}.", r.doc.spoken_title(r.opts)));
        }
        if r.last >= r.doc.n_pages && !paras.is_empty() {
            paras.push(END_WORD.into());
        }
        *job.total_chars.lock().unwrap() = paras.iter().map(|p| len(p)).sum::<usize>().max(1);
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        for p in paras {
            if !cur.is_empty() && cur.iter().map(|x| len(x)).sum::<usize>() + len(&p) > 1500 {
                groups.push(std::mem::take(&mut cur));
            }
            cur.push(p);
        }
        if !cur.is_empty() {
            groups.push(cur);
        }
        let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut enc: Option<crate::mp3::Mp3> = None;
        let cancel = || job.cancelled.load(Ordering::SeqCst);
        for group in groups {
            if cancel() {
                return Err("cancelled".into());
            }
            let text = group.join("\n\n");
            let (a, rate) = speech().synthesize(&text, &r.voice, r.speed, &cancel)?;
            if cancel() {
                return Err("cancelled".into());
            }
            if enc.is_none() {
                enc = Some(crate::mp3::Mp3::new(rate as u32, r.bitrate)?);
            }
            let mut pcm = a.clone();
            pcm.extend(std::iter::repeat(0f32).take((rate as f64 * 0.45) as usize));
            out.write_all(&enc.as_mut().unwrap().encode(&audio::to_pcm16(&pcm))).map_err(|e| e.to_string())?;
            *job.seconds.lock().unwrap() += pcm.len() as f64 / rate as f64;
            *job.done_chars.lock().unwrap() += group.iter().map(|x| len(x)).sum::<usize>();
        }
        let Some(mut e) = enc else { return Err("در این صفحه\u{200c}ها متنی برای خواندن نیست.".into()) };
        out.write_all(&e.flush()).map_err(|e| e.to_string())?;
        drop(out);
        let saved = unique_path(save_dir().join(&job.filename));
        std::fs::copy(&path, &saved).map_err(|e| e.to_string())?;
        crate::platform::saved(&saved, "mp3");
        *job.saved.lock().unwrap() = Some(saved);
        Ok(())
    })();
    job.finish(res);
}

fn doc_info(d: &Arc<Document>) -> Value {
    json!({"id": d.id, "name": d.name, "title": d.title(), "pages": d.n_pages, "lang": d.lang(),
           "canTranslate": d.can_translate(), "hasImages": d.pdfjs, "hasText": d.has_text(), "scanned": d.had_scans,
           "engine": if d.pdfjs { "pdfjs" } else { "text" }})
}

fn fa_digits(n: usize) -> String {
    n.to_string().chars().map(|d| char::from_u32('۰' as u32 + d.to_digit(10).unwrap_or(0)).unwrap_or(d)).collect()
}

static VAZIRMATN: &[u8] = include_bytes!("../web/Vazirmatn.woff2");

fn run_pdf_job(job: Arc<Job>, doc: Arc<Document>, first: usize, last: usize, opts: Opts) {
    use crate::ffi::typeset::Block;
    let _awake = crate::platform::awake(c"Khana Reader is saving a translated PDF");
    let res = (|| -> Result<(), String> {
        *job.total_chars.lock().unwrap() = last + 1 - first;
        let title = doc.spoken_title(opts);
        let sub = format!("ترجمه\u{200c}ی ماشینی (روی همین کامپیوتر) از «{}»، صفحه\u{200c}های {} تا {}", doc.name, fa_digits(first), fa_digits(last));
        let mut pages: Vec<(String, Vec<String>)> = Vec::new();
        for n in first..=last {
            if job.cancelled.load(Ordering::SeqCst) {
                return Err("cancelled".into());
            }
            let paras = doc.page_text(n, opts);
            if !paras.is_empty() {
                pages.push((format!("صفحه {}", fa_digits(n)), paras));
            }
            *job.done_chars.lock().unwrap() = n + 1 - first;
        }
        if pages.is_empty() {
            return Err("در این صفحه\u{200c}ها متنی برای ترجمه نیست.".into());
        }
        let mut blocks = vec![Block::Title(&title), Block::Subtitle(&sub)];
        for (head, paras) in &pages {
            blocks.push(Block::Heading(head));
            blocks.extend(paras.iter().map(|p| Block::Paragraph(p)));
        }
        let font = VAZIRMATN.to_vec();
        let saved = unique_path(save_dir().join(&job.filename));
        crate::platform::farsi_pdf(&blocks, &font, &saved)?;
        crate::platform::saved(&saved, "pdf");
        *job.saved.lock().unwrap() = Some(saved);
        Ok(())
    })();
    job.finish(res);
}


#[cfg(not(target_os = "android"))]
const FRAME_SRC: &str = "khana:";
#[cfg(target_os = "android")]
const FRAME_SRC: &str = "https://khana.invalid";

const LOCKED_POWERS: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), bluetooth=(), camera=(), clipboard-read=(), clipboard-write=(), display-capture=(), document-domain=(), encrypted-media=(), fullscreen=(), gamepad=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), interest-cohort=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), speaker-selection=(), storage-access=(), sync-xhr=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

const PAGE_PERMISSIONS: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(self), battery=(), bluetooth=(), camera=(), clipboard-read=(), clipboard-write=(self), display-capture=(), document-domain=(), encrypted-media=(), fullscreen=(), gamepad=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), interest-cohort=(), local-fonts=(), magnetometer=(), microphone=(self), midi=(), payment=(), picture-in-picture=(), publickey-credentials-get=(), screen-wake-lock=(), serial=(), speaker-selection=(), storage-access=(), sync-xhr=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";

pub struct Reply {
    pub status: u16,
    pub ctype: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

fn reply_json(v: &Value) -> Reply {
    Reply { status: 200, ctype: "application/json; charset=utf-8", body: serde_json::to_vec(v).unwrap(), headers: Vec::new() }
}

pub enum ApiError {
    NotFound(String),
    Bad(String),
    Internal(String),
}

type R = Result<Reply, ApiError>;

fn bad<E: ToString>(e: E) -> ApiError {
    ApiError::Bad(e.to_string())
}

pub struct App {
    pub token: String,
    docs: Mutex<Option<Arc<Document>>>,
    jobs: Mutex<HashMap<String, Arc<Job>>>,
    engines: Arc<Engines>,
}

fn query(q: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for part in q.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        m.insert(pct_decode(k), pct_decode(v));
    }
    m
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn tf(q: &HashMap<String, String>, k: &str, default: bool) -> bool {
    match q.get(k) {
        Some(v) => v == "true",
        None => default,
    }
}

pub fn new_token() -> String {
    let mut b = [0u8; 24];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl App {
    pub fn new(engines: Engines) -> App {
        App { token: new_token(), docs: Mutex::new(None), jobs: Mutex::new(HashMap::new()), engines: Arc::new(engines) }
    }

    fn doc(&self, q: &HashMap<String, String>) -> Result<Arc<Document>, ApiError> {
        let d = self.docs.lock().unwrap().clone();
        match d {
            Some(d) if Some(&d.id) == q.get("doc") => Ok(d),
            _ => Err(ApiError::NotFound("سند پیدا نشد. فایل را دوباره باز کنید.".into())),
        }
    }

    fn opts(&self, q: &HashMap<String, String>) -> Opts {
        let s = load_settings();
        let b = |k: &str| s.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        Opts { reversed: tf(q, "rev", b("reversed")), persian_yeh: tf(q, "yeh", b("persianYeh")), translate: tf(q, "tr", b("translate")) }
    }

    fn add_job(&self, job: Job) -> Arc<Job> {
        let job = Arc::new(job);
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|_, j| {
            let keep = *j.status.lock().unwrap() == "running";
            if !keep {
                j.cleanup();
            }
            keep
        });
        jobs.insert(job.id.clone(), job.clone());
        job
    }

    pub fn handle(&self, method: &str, path: &str, qs: &str, token: Option<&str>, body: &[u8], web: &Path) -> Reply {
        let q = query(qs);
        if method == "GET" && (path == "/" || path == "/index.html") {
            let html = std::fs::read_to_string(web.join("index.html")).unwrap_or_default().replace("__KHANA_TOKEN__", &self.token);
            #[cfg(target_os = "android")]
            let html = html.replace("<script src=\"app.js\"></script>", "<script src=\"app.js\"></script>\n<script src=\"android.js\"></script>");
            return Reply {
                status: 200,
                ctype: "text/html; charset=utf-8",
                body: html.into_bytes(),
                headers: vec![("Content-Security-Policy".into(),
                    "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' blob:; media-src 'self' blob:; \
                     font-src 'self'; connect-src 'self'; frame-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'".into()),
                    ("Permissions-Policy".into(), PAGE_PERMISSIONS.into())],
            };
        }
        if method == "GET" && path.starts_with("/pdfjs/") {
            let rel = &path[1..];
            if !fullmatch(r"pdfjs/(?:[A-Za-z0-9_\-]+/)?[A-Za-z0-9_\-][A-Za-z0-9_.\-]*\.(?:mjs|js|html|bcmap|pfb|ttf)", rel) || rel.contains("..") {
                return Reply { status: 404, ctype: "text/plain", body: Vec::new(), headers: Vec::new() };
            }
            let Ok(body) = std::fs::read(web.join(rel)) else { return Reply { status: 404, ctype: "text/plain", body: Vec::new(), headers: Vec::new() } };
            let ctype = match rel.rsplit('.').next() {
                Some("mjs") | Some("js") => "text/javascript; charset=utf-8",
                Some("html") => "text/html; charset=utf-8",
                _ => "application/octet-stream",
            };
            let mut headers = vec![("Access-Control-Allow-Origin".to_string(), "*".to_string())];
            if ctype.starts_with("text/html") {
                headers.push(("Content-Security-Policy".into(),
                    format!("default-src 'none'; script-src {o} blob:; worker-src {o} blob:; connect-src {o}; img-src {o} blob: data:; \
                     base-uri 'none'; form-action 'none'; frame-ancestors {o}", o = FRAME_SRC)));
                headers.push(("Permissions-Policy".into(), LOCKED_POWERS.into()));
            } else if ctype.starts_with("text/javascript") {
                headers.push(("Content-Security-Policy".into(), format!("default-src 'none'; script-src {o} blob:; connect-src {o}", o = FRAME_SRC)));
            }
            return Reply { status: 200, ctype, body, headers };
        }
        if method == "GET" && (path == "/app.js" || path == "/app.css" || (cfg!(target_os = "android") && path == "/android.js")) {
            let ctype = if path.ends_with(".js") { "text/javascript; charset=utf-8" } else { "text/css; charset=utf-8" };
            return Reply { status: 200, ctype, body: std::fs::read(web.join(&path[1..])).unwrap_or_default(), headers: Vec::new() };
        }
        if method == "GET" && path == "/Vazirmatn.woff2" {
            return Reply { status: 200, ctype: "font/woff2", body: VAZIRMATN.to_vec(), headers: Vec::new() };
        }
        let authed = token.or(q.get("t").map(|s| s.as_str())) == Some(self.token.as_str());
        if !path.starts_with("/api/") || !authed {
            return Reply { status: if path.starts_with("/api/") { 403 } else { 404 }, ctype: "text/plain", body: Vec::new(), headers: Vec::new() };
        }
        let route = format!("{}_{}", &path[5..].replace('/', "_"), method.to_lowercase());
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.route(&route, &q, body)))
            .unwrap_or_else(|p| Err(ApiError::Internal(p.downcast_ref::<String>().cloned().or(p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())));
        match res {
            Ok(r) => r,
            Err(ApiError::NotFound(m)) => Reply { status: 404, ..reply_json(&json!({"error": m})) },
            Err(ApiError::Bad(m)) => Reply { status: 400, ..reply_json(&json!({"error": m})) },
            Err(ApiError::Internal(m)) => Reply { status: 500, ..reply_json(&json!({"error": format!("خطا: {m}")})) },
        }
    }

    fn route(&self, route: &str, q: &HashMap<String, String>, body: &[u8]) -> R {
        let body_json = || -> Result<Map<String, Value>, ApiError> {
            if body.is_empty() {
                return Ok(Map::new());
            }
            match serde_json::from_slice::<Value>(body) {
                Ok(Value::Object(m)) => Ok(m),
                Ok(_) => Ok(Map::new()),
                Err(e) => Err(bad(e)),
            }
        };
        match route {
            "init_get" => {
                let voices: Vec<Value> = all_voices().into_iter().map(|(id, name, gender, group)| json!({"id": id, "name": name, "gender": gender, "group": group})).collect();
                Ok(reply_json(&json!({"settings": load_settings(), "voices": voices})))
            }
            "settings_post" => Ok(reply_json(&Value::Object(save_settings(&body_json()?).map_err(ApiError::Internal)?))),
            "myvoice_post" => {
                let name = q.get("name").cloned().unwrap_or_else(|| "voice.wav".into());
                let ext = crate::doc::ext_of(&name);
                let ext = if ext.is_empty() { ".wav".to_string() } else { ext };
                if ![".wav", ".m4a", ".mp3", ".aif", ".aiff", ".caf", ".mp4", ".aac", ".flac"].contains(&ext.as_str()) {
                    return Err(bad("این نوع فایل صوتی پشتیبانی نمی\u{200c}شود. WAV، M4A یا MP3 انتخاب کنید."));
                }
                if body.len() > 50 * 1024 * 1024 {
                    return Err(bad("فایل صوتی خیلی بزرگ است. یک ضبط کوتاه (۱۰ تا ۳۰ ثانیه) کافی است."));
                }
                Ok(reply_json(&json!({"seconds": make_my_voice(body, &ext).map_err(bad)?})))
            }
            "myvoice_delete_post" => {
                let _ = std::fs::remove_file(my_voice());
                let s = load_settings();
                if s.get("voice").and_then(|v| v.as_str()) == Some("mine") {
                    let mut m = Map::new();
                    m.insert("voice".into(), json!(voices().first().map(|v| v.id).unwrap_or("")));
                    save_settings(&m).map_err(ApiError::Internal)?;
                }
                Ok(reply_json(&json!({"ok": true})))
            }
            "close_post" => {
                *self.docs.lock().unwrap() = None;
                Ok(reply_json(&json!({"ok": true})))
            }
            "open_post" => {
                let name = q.get("name").cloned().unwrap_or_else(|| "document".into());
                if body.len() > 300 * 1024 * 1024 {
                    return Err(bad("فایل خیلی بزرگ است (بیشتر از ۳۰۰ مگابایت)."));
                }
                *self.docs.lock().unwrap() = None;
                let d = open_document(&name, body, self.engines.clone()).map_err(bad)?;
                crate::vocab::set_source_lang(d.lang());
                *self.docs.lock().unwrap() = Some(d.clone());
                Ok(reply_json(&doc_info(&d)))
            }
            "open_layout_post" => {
                let name = q.get("name").cloned().unwrap_or_else(|| "document".into());
                if body.len() > 400 * 1024 * 1024 {
                    return Err(bad("فایل خیلی بزرگ است."));
                }
                let v: Value = serde_json::from_slice(body).map_err(|_| bad("صفحه‌های pdf.js خوانده نشد."))?;
                let pages = v.get("pages").and_then(|p| p.as_array()).ok_or_else(|| bad("صفحه‌های pdf.js خوانده نشد."))?;
                if pages.is_empty() || pages.len() > 20000 {
                    return Err(bad("صفحه‌های pdf.js خوانده نشد."));
                }
                let num = |x: Option<&Value>| x.and_then(|v| v.as_f64()).filter(|f| f.is_finite() && f.abs() < 1e6).unwrap_or(0.0);
                let mut layout = Vec::with_capacity(pages.len());
                for p in pages {
                    let items = p.get("items").and_then(|i| i.as_array()).map(|a| a.as_slice()).unwrap_or(&[]);
                    let items: Vec<crate::text::LayoutItem> = items.iter().take(200_000).filter_map(|it| {
                        let a = it.as_array()?;
                        let s: String = a.first()?.as_str()?.chars().take(10_000).collect();
                        Some(crate::text::LayoutItem { s, x0: num(a.get(1)), top: num(a.get(2)), x1: num(a.get(3)), bot: num(a.get(4)),
                            size: num(a.get(5)).max(0.1), rtl: num(a.get(6)) != 0.0 })
                    }).collect();
                    layout.push((items, num(p.get("h")).max(1.0), p.get("images").and_then(|b| b.as_bool()).unwrap_or(false)));
                }
                *self.docs.lock().unwrap() = None;
                let d = crate::doc::open_document_layout(&name, layout, self.engines.clone());
                crate::vocab::set_source_lang(d.lang());
                *self.docs.lock().unwrap() = Some(d.clone());
                Ok(reply_json(&doc_info(&d)))
            }
            "page_picture_post" => {
                let d = self.doc(q)?;
                let n: usize = q.get("n").and_then(|x| x.parse().ok()).unwrap_or(0);
                let u32_at = |at: usize| body.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
                let (w, h) = (u32_at(0), u32_at(4));
                if !d.pdfjs || n < 1 || n > d.n_pages || w == 0 || h == 0 || w > 20000 || h > 20000 || w.checked_mul(h) != Some(body.len() - 8) {
                    return Err(bad("تصویر صفحه درست نیست."));
                }
                d.give_picture(n, body[8..].to_vec(), w, h);
                Ok(reply_json(&json!({"ok": true})))
            }
            "page_get" => {
                let d = self.doc(q)?;
                let n: usize = q.get("n").and_then(|x| x.parse().ok()).unwrap_or(1);
                if n < 1 || n > d.n_pages {
                    return Err(bad("شماره صفحه درست نیست."));
                }
                if d.needs_picture(n) {
                    return Ok(reply_json(&json!({"n": n, "needsPicture": true})));
                }
                let opts = self.opts(q);
                if q.get("partial").map(|s| s.as_str()) == Some("1") && opts.translate && d.can_translate() {
                    let (paras, done) = d.page_text_so_far(n);
                    return Ok(reply_json(&json!({"n": n, "paragraphs": paras, "done": done})));
                }
                Ok(reply_json(&json!({"n": n, "paragraphs": d.page_text(n, opts), "done": true})))
            }
            "title_get" => {
                let d = self.doc(q)?;
                Ok(reply_json(&json!({"title": d.spoken_title(self.opts(q))})))
            }
            "tts_post" => {
                let req = body_json()?;
                let text: String = strip(req.get("text").and_then(|v| v.as_str()).unwrap_or("")).chars().take(5000).collect();
                if text.is_empty() {
                    return Err(bad("متنی برای خواندن نیست."));
                }
                let s = load_settings();
                let voice = req.get("voice").and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string())
                    .unwrap_or_else(|| s["voice"].as_str().unwrap_or("").to_string());
                let speed = req.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
                speech().mark_play();
                let (a, rate) = speech().synthesize(&text, &voice, speed, &|| false).map_err(ApiError::Internal)?;
                speech().mark_play();
                Ok(Reply { status: 200, ctype: "audio/wav", body: audio::wav_bytes(&a, rate as u32), headers: Vec::new() })
            }
            "mp3_post" => {
                let req = body_json()?;
                let d = self.docs.lock().unwrap().clone();
                let d = match d {
                    Some(d) if req.get("doc").and_then(|v| v.as_str()) == Some(d.id.as_str()) => d,
                    _ => return Err(ApiError::NotFound("سند پیدا نشد. فایل را دوباره باز کنید.".into())),
                };
                let s = load_settings();
                let num = |k: &str, dflt: f64| req.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|x| x.parse().ok()))).unwrap_or(dflt);
                let first = (num("first", 1.0) as i64).max(1) as usize;
                let last = (num("last", d.n_pages as f64) as i64).min(d.n_pages as i64).max(0) as usize;
                if first > last {
                    return Err(bad("بازه صفحه\u{200c}ها درست نیست."));
                }
                let flag = |k: &str| req.get(k).and_then(|v| v.as_bool()).unwrap_or_else(|| s.get(k).and_then(|v| v.as_bool()).unwrap_or(false));
                let voice = req.get("voice").and_then(|v| v.as_str()).filter(|v| !v.is_empty()).map(|v| v.to_string())
                    .unwrap_or_else(|| s["voice"].as_str().unwrap_or("").to_string());
                let speed = num("speed", s["speed"].as_f64().unwrap_or(1.0));
                let bitrate = (num("bitrate", s["bitrate"].as_f64().unwrap_or(64.0)) as i64).clamp(32, 320) as u32;
                let opts = Opts { reversed: flag("reversed"), persian_yeh: flag("persianYeh"), translate: flag("translate") };
                let stem = std::path::Path::new(&d.name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let rng = if first == last { format!("{first}") } else { format!("{first}-{last}") };
                let job = self.add_job(Job::new(format!("{stem} - p{rng}.mp3")));
                let info = job.info();
                let r = Mp3Req { doc: d, first, last, voice, speed, bitrate, opts };
                std::thread::spawn(move || run_job(job, r));
                Ok(reply_json(&info))
            }
            "pdf_post" => {
                let req = body_json()?;
                let d = self.docs.lock().unwrap().clone();
                let d = match d {
                    Some(d) if req.get("doc").and_then(|v| v.as_str()) == Some(d.id.as_str()) => d,
                    _ => return Err(ApiError::NotFound("سند پیدا نشد. فایل را دوباره باز کنید.".into())),
                };
                if !d.can_translate() {
                    return Err(bad("این سند فارسی است و ترجمه لازم ندارد."));
                }
                let num = |k: &str, dflt: usize| req.get(k).and_then(|v| v.as_f64()).map(|x| x.max(0.0) as usize).unwrap_or(dflt);
                let (first, last) = (num("first", 1).max(1), num("last", d.n_pages).min(d.n_pages));
                if first > last {
                    return Err(bad("بازه صفحه\u{200c}ها درست نیست."));
                }
                let stem = std::path::Path::new(&d.name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let rng = if first == 1 && last == d.n_pages { String::new() } else if first == last { format!(" - p{first}") } else { format!(" - p{first}-{last}") };
                let job = self.add_job(Job::new(format!("{stem} - فارسی{rng}.pdf")));
                let info = job.info();
                let opts = Opts { translate: true, ..Opts::default() };
                std::thread::spawn(move || run_pdf_job(job, d, first, last, opts));
                Ok(reply_json(&info))
            }
            "job_get" => {
                let j = self.jobs.lock().unwrap().get(q.get("id").map(|s| s.as_str()).unwrap_or("")).cloned();
                let j = j.ok_or_else(|| ApiError::NotFound("کار پیدا نشد.".into()))?;
                Ok(reply_json(&j.info()))
            }
            "cancel_post" => {
                if let Some(j) = self.jobs.lock().unwrap().get(q.get("id").map(|s| s.as_str()).unwrap_or("")) {
                    j.cancelled.store(true, Ordering::SeqCst);
                }
                Ok(reply_json(&json!({"ok": true})))
            }
            "reveal_post" => {
                let j = self.jobs.lock().unwrap().get(q.get("id").map(|s| s.as_str()).unwrap_or("")).cloned();
                let saved = j.and_then(|j| j.saved.lock().unwrap().clone()).filter(|p| cfg!(target_os = "android") || p.exists());
                let saved = saved.ok_or_else(|| ApiError::NotFound("فایل پیدا نشد.".into()))?;
                crate::platform::reveal(&saved);
                Ok(reply_json(&json!({"ok": true})))
            }
            "download_get" => {
                let j = self.jobs.lock().unwrap().get(q.get("id").map(|s| s.as_str()).unwrap_or("")).cloned();
                let j = j.filter(|j| *j.status.lock().unwrap() == "done").ok_or_else(|| ApiError::NotFound("فایل MP3 آماده نیست.".into()))?;
                let p = j.path.lock().unwrap().clone().ok_or_else(|| ApiError::NotFound("فایل MP3 آماده نیست.".into()))?;
                let data = std::fs::read(p).map_err(|e| ApiError::Internal(e.to_string()))?;
                let ascii: String = j.filename.chars().map(|c| if c.is_ascii_alphanumeric() || " ._-".contains(c) { c } else { '_' }).collect();
                Ok(Reply { status: 200, ctype: "audio/mpeg", body: data,
                    headers: vec![("Content-Disposition".into(), format!("attachment; filename=\"{ascii}\""))] })
            }
            "selftest_passwords_get" if std::env::var("KHANA_SELFTEST").is_ok() => {
                let list: Vec<String> = std::env::var("KHANA_SELFTEST_PASSWORDS").unwrap_or_default().split(',').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
                Ok(reply_json(&json!({"list": list})))
            }
            "selftest_file_get" if std::env::var("KHANA_SELFTEST").is_ok() => {
                let f = if q.get("name").map(|s| s.as_str()) == Some("scan") {
                    save_dir().join(std::env::var("KHANA_SELFTEST_SCAN").unwrap_or_default())
                } else {
                    save_dir().join(std::env::var("KHANA_SELFTEST").unwrap())
                };
                let data = std::fs::read(&f).map_err(|e| ApiError::Internal(format!("{}: {e}", f.display())))?;
                Ok(Reply { status: 200, ctype: "application/octet-stream", body: data, headers: Vec::new() })
            }
            "selftest_layout_post" if std::env::var("KHANA_SELFTEST").is_ok() => match std::env::var("KHANA_SELFTEST_LAYOUT") {
                Ok(path) if !body.is_empty() => {
                    std::fs::write(&path, body).map_err(|e| ApiError::Internal(e.to_string()))?;
                    Ok(reply_json(&json!({"only": true})))
                }
                Ok(_) => Ok(reply_json(&json!({"only": true}))),
                Err(_) => Ok(reply_json(&json!({"only": false}))),
            },
            "selftest_report_post" if std::env::var("KHANA_SELFTEST").is_ok() => {
                eprintln!("SELFTEST {}", String::from_utf8_lossy(body));
                if String::from_utf8_lossy(body).starts_with("DONE") {
                    std::process::exit(0);
                }
                Ok(reply_json(&json!({"ok": true})))
            }
            _ => Err(ApiError::NotFound("not found".into())),
        }
    }

    pub fn shutdown(&self) {
        for j in self.jobs.lock().unwrap().values() {
            j.cancelled.store(true, Ordering::SeqCst);
            j.cleanup();
        }
    }
}
