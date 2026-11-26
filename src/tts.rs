
use crate::prosody;
use crate::asr;
use crate::audio::{self, median, rms};
use crate::lexicon::LexiconG2P;
use crate::onnx;
use crate::planner::{self, Chunker, CHUNK_GAP, SENTENCE_GAP};
use crate::pyre::*;
use crate::spm::SentencePiece;
use ort::{Session, Tensor};
use rand::{RngExt, SeedableRng};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const RATE: usize = 24000;
const PACE_LOW: f64 = 0.72;
const PACE_HIGH: f64 = 1.6;
const PACE_TRIES: usize = if cfg!(shell) { 3 } else { 4 };
static MAX_TRIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(PACE_TRIES);
static READ_RTF: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x4000_0000); // 2.0f32

/// Records recent synthesis speed (speech_seconds / wall_seconds) so retries adapt to prevent stalls.
pub fn set_read_rtf(rtf: f64) {
    if rtf.is_finite() && rtf > 0.05 {
        let clamped = (rtf as f32).clamp(0.1, 20.0);
        READ_RTF.store(clamped.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

fn read_rtf() -> f64 {
    f32::from_bits(READ_RTF.load(std::sync::atomic::Ordering::Relaxed)) as f64
}

pub static PROFILE: [std::sync::atomic::AtomicU64; 6] = [const { std::sync::atomic::AtomicU64::new(0) }; 6];
fn lap(slot: usize, t: std::time::Instant) {
    PROFILE[slot].fetch_add(t.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
}

static HEAT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_heat(level: u8) {
    HEAT.store(level.min(3), std::sync::atomic::Ordering::Relaxed);
}

pub fn heat() -> u8 {
    HEAT.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn cool_down() {
    match heat() {
        0 => {}
        1 => std::thread::sleep(std::time::Duration::from_millis(80)),
        2 => std::thread::sleep(std::time::Duration::from_millis(400)),
        _ => {
            for _ in 0..150 {
                if heat() < 3 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }
}

static LOW_BATTERY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_power(percent: u8, charging: bool) {
    let low = LOW_BATTERY.load(std::sync::atomic::Ordering::Relaxed);
    let now = !charging && (percent <= 20 || low && percent <= 25);
    LOW_BATTERY.store(now, std::sync::atomic::Ordering::Relaxed);
}

pub fn low_battery() -> bool {
    LOW_BATTERY.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn wait_for_power(cancelled: &dyn Fn() -> bool) {
    while low_battery() && !cancelled() {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

pub fn set_max_tries(n: usize) {
    MAX_TRIES.store(n.clamp(1, PACE_TRIES), std::sync::atomic::Ordering::Relaxed);
}
pub const PARAGRAPH_GAP: f64 = 0.9;

static COMMA_PAUSE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F26_6666);

pub fn set_comma_pause(kind: &str) {
    let f: f32 = match kind {
        "short" => 0.3,
        "long" => 1.0,
        _ => 0.65,
    };
    COMMA_PAUSE.store(f.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

fn comma_pause() -> f64 {
    f32::from_bits(COMMA_PAUSE.load(std::sync::atomic::Ordering::Relaxed)) as f64
}

pub static LISTEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub static LISTEN_LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);


struct Slots {
    state: Mutex<(usize, Vec<u64>)>,
    turn: std::sync::Condvar,
}
static SLOTS: Slots = Slots { state: Mutex::new((0, Vec::new())), turn: std::sync::Condvar::new() };
static TICKETS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 62);
fn slot_count() -> usize {
    static SLOTS_MAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SLOTS_MAX.get_or_init(|| {
        if cfg!(shell) {
            // Mobile SoCs (e.g. Tensor G2) throttle past 80C when 6 ONNX models thrash all 8 cores;
            // 2 concurrent slots x 2 intra-op threads keeps big/mid cores fast and cool.
            2
        } else {
            (std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(2)).clamp(2, 12)
        }
    })
}
pub fn wake_waiters() {
    SLOTS.turn.notify_all();
}
struct Slot;
impl Slot {
    fn take(ticket: u64, cancel: &(dyn Fn() -> bool + Sync)) -> Result<Slot, String> {
        let mut st = SLOTS.state.lock().unwrap_or_else(|e| e.into_inner());
        st.1.push(ticket);
        loop {
            if cancel() {
                if let Some(i) = st.1.iter().position(|t| *t == ticket) {
                    st.1.swap_remove(i);
                }
                drop(st);
                SLOTS.turn.notify_all();
                return Err("cancelled".to_string());
            }
            if st.0 < slot_count() && st.1.iter().min() == Some(&ticket) {
                break;
            }
            let (guard, _) = SLOTS.turn.wait_timeout(st, std::time::Duration::from_millis(60)).unwrap_or_else(|e| e.into_inner());
            st = guard;
        }
        if let Some(i) = st.1.iter().position(|t| *t == ticket) {
            st.1.swap_remove(i);
        }
        st.0 += 1;
        drop(st);
        SLOTS.turn.notify_all();
        Ok(Slot)
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        SLOTS.state.lock().unwrap_or_else(|e| e.into_inner()).0 -= 1;
        SLOTS.turn.notify_all();
    }
}


#[derive(Clone)]
pub enum Arr {
    F32(Vec<usize>, Vec<f32>),
    Bool(Vec<usize>, Vec<bool>),
    I64(Vec<usize>, Vec<i64>),
}

fn parse_npy(b: &[u8]) -> Result<Arr, String> {
    if b.len() < 10 || &b[..6] != b"\x93NUMPY" {
        return Err("not npy".into());
    }
    let (hlen, off) = if b[6] == 1 { (u16::from_le_bytes([b[8], b[9]]) as usize, 10) } else { (u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize, 12) };
    let header = std::str::from_utf8(&b[off..off + hlen]).map_err(|e| e.to_string())?;
    let descr = header.split("'descr':").nth(1).and_then(|s| s.split('\'').nth(1)).ok_or("descr")?;
    let shape_s = header.split("'shape':").nth(1).and_then(|s| s.split('(').nth(1)).and_then(|s| s.split(')').next()).ok_or("shape")?;
    let shape: Vec<usize> = shape_s.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect();
    let data = &b[off + hlen..];
    let n: usize = shape.iter().product();
    Ok(match descr {
        "<f4" => Arr::F32(shape, data[..n * 4].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()),
        "|b1" => Arr::Bool(shape, data[..n].iter().map(|x| *x != 0).collect()),
        "<i8" => Arr::I64(shape, data[..n * 8].chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect()),
        d => return Err(format!("dtype {d}")),
    })
}

pub fn load_npz(path: &Path) -> Result<Vec<(String, Arr)>, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let name = e.name().map(|s| s.to_string()).unwrap_or_default();
        let mut b = Vec::new();
        e.read_to_end(&mut b).map_err(|e| e.to_string())?;
        out.push((name.trim_end_matches(".npy").to_string(), parse_npy(&b)?));
    }
    Ok(out)
}

fn f32s(a: &Arr) -> &[f32] {
    match a {
        Arr::F32(_, v) => v,
        _ => panic!("not f32"),
    }
}

fn input<'v>(v: &'v ort::DynValue) -> ort::SessionInputValue<'v> {
    v.view().try_upgrade().map(ort::SessionInputValue::from).unwrap_or_else(ort::SessionInputValue::from)
}

fn tensor_f32(shape: Vec<usize>, data: Vec<f32>) -> ort::DynValue {
    if shape.contains(&0) {
        return crate::ffi::onnx::empty_f32(&shape);
    }
    Tensor::from_array((shape, data)).unwrap().into_dyn()
}

fn copy_value(v: &ort::DynValue) -> Result<ort::DynValue, String> {
    let ort::ValueType::Tensor { ty, dimensions } = v.dtype() else { return Err("not a tensor".into()) };
    let shape: Vec<usize> = dimensions.iter().map(|x| *x as usize).collect();
    let e = |e: ort::Error| e.to_string();
    Ok(match ty {
        ort::TensorElementType::Float32 => {
            if shape.contains(&0) { tensor_f32(shape, Vec::new()) } else { tensor_f32(shape, v.try_extract_raw_tensor::<f32>().map_err(e)?.1.to_vec()) }
        }
        ort::TensorElementType::Bool => value_of(&Arr::Bool(shape.clone(), if shape.contains(&0) { Vec::new() } else { v.try_extract_raw_tensor::<bool>().map_err(e)?.1.to_vec() })),
        ort::TensorElementType::Int64 => value_of(&Arr::I64(shape.clone(), if shape.contains(&0) { Vec::new() } else { v.try_extract_raw_tensor::<i64>().map_err(e)?.1.to_vec() })),
        t => return Err(format!("state type {t:?}")),
    })
}

fn value_of(a: &Arr) -> ort::DynValue {
    match a.clone() {
        Arr::F32(s, v) => tensor_f32(s, v),
        Arr::Bool(s, v) => {
            if s.contains(&0) {
                return tensor_f32(s, Vec::new());
            }
            Tensor::from_array((s, v)).unwrap().into_dyn()
        }
        Arr::I64(s, v) => Tensor::from_array((s, v)).unwrap().into_dyn(),
    }
}


pub struct Engine {
    pub sp: SentencePiece,
    lut: Vec<f32>,
    spk_proj: Vec<f32>,
    bos_voice: Vec<f32>,
    emb_std: Vec<f32>,
    emb_mean: Vec<f32>,
    ldim: usize,
    dim: usize,
    l: usize,
    h: usize,
    d: usize,
    cap: usize,
    steps_per_latent: usize,
    temp: f64,
    tps_est: f64,
    gen_pad: f64,
    frame_rate: f64,
    s_flow: Session,
    s_enc: Session,
    s_dec: Session,
    dec_in: Vec<String>,
    dec_init: Vec<Arr>,
    cache_slots: Vec<usize>,
    small_slots: Vec<usize>,
    dec_out_small: Vec<usize>,
    dec_out_kv: Vec<usize>,
    voice_memo: Mutex<Vec<(String, Arc<(Vec<f32>, usize)>)>>,
    voice_build: Mutex<()>,
    pub g2p: LexiconG2P,
    pace: Mutex<HashMap<String, Vec<f64>>>,
    pitch: Mutex<HashMap<String, Vec<f64>>>,
    pub natural: std::sync::atomic::AtomicBool,
    rng: Mutex<rand::rngs::StdRng>,
}

struct Voice<'a> {
    id: &'a str,
    cache: &'a [f32],
    off: usize,
    quick: bool,
}

impl Engine {
    pub fn new(parsigo: &Path) -> Result<Engine, String> {
        let dir = parsigo.join("onnx");
        let man: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let c = &man["constants"];
        let u = |k: &str| c[k].as_u64().unwrap() as usize;
        let f = |k: &str| c[k].as_f64().unwrap();
        let w = load_npz(&dir.join("weights.npz"))?;
        let get = |k: &str| f32s(&w.iter().find(|x| x.0 == k).unwrap().1).to_vec();
        let threads = std::env::var("KHANA_TTS_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(2usize);
        // On mobile, Mimi runs 1 tiny latent frame at a time (50+ calls/sentence): 1 intra-op thread
        // eliminates ~1,500 thread-pool synchronization barriers per sentence across 2 concurrent workers!
        let dec_threads = if cfg!(shell) { 1usize } else { threads };
        let opts = |p: &Path| onnx::session(p, Some(threads)).map_err(|e| e.to_string());
        let s_dec = onnx::session(&dir.join("mimi_decoder_step_kv.onnx"), Some(dec_threads)).map_err(|e| e.to_string())?;
        let dec_in: Vec<String> = s_dec.inputs.iter().map(|i| i.name.clone()).collect();
        let dec_out: Vec<String> = s_dec.outputs.iter().map(|o| o.name.clone()).collect();
        let init = load_npz(&dir.join("decode_state_init.npz"))?;
        let n_state = dec_in.len() - 1;
        let cache_slots: Vec<usize> = init.iter().enumerate().filter(|(_, (k, _))| k.ends_with(".cache")).map(|(i, _)| i).collect();
        let small_slots: Vec<usize> = (0..n_state).filter(|i| !cache_slots.contains(i)).collect();
        let dec_out_small: Vec<usize> = dec_out.iter().enumerate().filter(|(_, n)| n.ends_with('o') && *n != "audio").map(|(i, _)| i).collect();
        let dec_out_kv: Vec<usize> = dec_out.iter().enumerate().filter(|(_, n)| n.starts_with("kv")).map(|(i, _)| i).collect();
        let base = crate::g2p::OnnxG2P::new(&dir).map_err(|e| e.to_string())?;
        Ok(Engine {
            sp: SentencePiece::load(&parsigo.join("tokenizer_ph.model")).map_err(|e| e.to_string())?,
            lut: get("lut_weight"),
            spk_proj: get("speaker_proj"),
            bos_voice: get("bos_before_voice"),
            emb_std: get("emb_std"),
            emb_mean: get("emb_mean"),
            ldim: u("ldim"),
            dim: u("dim"),
            l: u("layers"),
            h: u("heads"),
            d: u("dim_per_head"),
            cap: u("cache_capacity"),
            steps_per_latent: u("mimi_steps_per_latent"),
            temp: f("temp"),
            tps_est: f("tokens_per_second_estimate"),
            gen_pad: f("gen_seconds_padding"),
            frame_rate: f("frame_rate"),
            s_flow: opts(&dir.join("flow_lm_step.onnx"))?,
            s_enc: opts(&dir.join("mimi_encoder.onnx"))?,
            s_dec,
            dec_in,
            dec_init: init.into_iter().map(|x| x.1).collect(),
            cache_slots,
            small_slots,
            dec_out_small,
            dec_out_kv,
            voice_memo: Mutex::new(Vec::new()),
            voice_build: Mutex::new(()),
            g2p: LexiconG2P::new(base),
            pace: Mutex::new(HashMap::new()),
            pitch: Mutex::new(HashMap::new()),
            natural: std::sync::atomic::AtomicBool::new(true),
            rng: Mutex::new(rand::make_rng()),
        })
    }

    fn kv_len(&self) -> usize {
        self.l * 2 * self.cap * self.h * self.d
    }

    fn flow_step(&self, seq: &[f32], seq_len: usize, text_emb: &[f32], text_len: usize, offset: usize, noise: &[f32], cache: &mut ort::DynValue)
        -> Result<(Vec<f32>, bool, usize), String> {
        let s = seq_len + text_len;
        if offset + s > self.cap {
            return Err(format!("flow cache overflow: offset {offset} + {s} new tokens exceeds capacity {}", self.cap));
        }
        let (lat, eos, kv, kvs) = {
            let out = self
                .s_flow
                .run(vec![
                    ("sequence", ort::SessionInputValue::from(tensor_f32(vec![1, seq_len, self.ldim], seq.to_vec()))),
                    ("text_emb", tensor_f32(vec![1, text_len, self.dim], text_emb.to_vec()).into()),
                    ("offset", Tensor::from_array((Vec::<usize>::new(), vec![offset as i64])).unwrap().into_dyn().into()),
                    ("noise", Tensor::from_array(([1, self.ldim], noise.to_vec())).unwrap().into_dyn().into()),
                    ("cache", input(cache)),
                ])
                .map_err(|e| e.to_string())?;
            let lat = out[0].try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?.1.to_vec();
            let eos = match out[1].try_extract_raw_tensor::<f32>() {
                Ok((_, v)) => v.first().copied().unwrap_or(0.0) != 0.0,
                Err(_) => out[1].try_extract_raw_tensor::<bool>().map_err(|e| e.to_string())?.1.first().copied().unwrap_or(false),
            };
            let (shape, kv) = out[2].try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?;
            (lat, eos, kv.to_vec(), shape[2] as usize)
        };
        let (_, c) = cache.try_extract_raw_tensor_mut::<f32>().map_err(|e| e.to_string())?;
        let row = self.h * self.d;
        for a in 0..self.l * 2 {
            for t in 0..kvs {
                let src = (a * kvs + t) * row;
                let dst = (a * self.cap + offset + t) * row;
                c[dst..dst + row].copy_from_slice(&kv[src..src + row]);
            }
        }
        Ok((lat, eos, offset + kvs))
    }

    fn voice_cache(&self, wav: &Path) -> Result<Arc<(Vec<f32>, usize)>, String> {
        let mtime = std::fs::metadata(wav).and_then(|m| m.modified()).map(|t| format!("{t:?}")).unwrap_or_default();
        let key = format!("{}|{mtime}", wav.display());
        {
            let mut memo = self.voice_memo.lock().unwrap();
            if let Some(i) = memo.iter().position(|x| x.0 == key) {
                let hit = memo.remove(i);
                let v = hit.1.clone();
                memo.push(hit);
                return Ok(v);
            }
        }
        let _one = self.voice_build.lock().unwrap_or_else(|e| e.into_inner());
        {
            let memo = self.voice_memo.lock().unwrap();
            if let Some(hit) = memo.iter().find(|x| x.0 == key) {
                return Ok(hit.1.clone());
            }
        }
        let (mut a, sr) = audio::read_wav(wav)?;
        if !(4000..=192_000).contains(&(sr as usize)) {
            return Err(format!("unusable sample rate {sr}"));
        }
        if sr as usize != RATE {
            let g = gcd(sr as usize, RATE);
            a = audio::resample_poly(&a, RATE / g, sr as usize / g);
        }
        let mut a = audio::dehum(a, RATE);
        // Trim ONLY pure leading dead air when >0.4s (e.g. male_news.wav's 1.32s leading silence).
        // Never splice internal pauses or boost room noise in the reference audio — Mimi's causal encoder
        // needs natural continuous speech to avoid conditioning drift or digital artifacts.
        if a.len() > 2 * RATE {
            let (s0, _e0) = self.speech_bounds(&a);
            if s0 > (0.4 * RATE as f64) as usize && a.len().saturating_sub(s0) >= 2 * RATE {
                a = a[s0..].to_vec();
            }
        }
        a.truncate(5 * RATE);
        let a = trim_hot_onset(a);
        let n = a.len();
        let lat = {
            let out = self
                .s_enc
                .run(ort::inputs!["audio" => Tensor::from_array(([1usize, 1, n], a)).unwrap()].map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            let (shape, v) = out[0].try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?;
            (shape[1] as usize, v.to_vec())
        };
        let (t, lat) = lat;
        let mut text_emb = self.bos_voice.clone();
        for i in 0..t {
            for o in 0..self.dim {
                let mut s = 0f32;
                for k in 0..self.ldim {
                    s += lat[i * self.ldim + k] * self.spk_proj[o * self.ldim + k];
                }
                text_emb.push(s);
            }
        }
        let mut cache = Tensor::from_array(([self.l, 2, self.cap, self.h, self.d], vec![0f32; self.kv_len()])).unwrap().into_dyn();
        let (_, _, off) = self.flow_step(&[], 0, &text_emb, t + 1, 0, &vec![0f32; self.ldim], &mut cache)?;
        // Store only the active voice prefix (0..off) instead of the full 384-slot KV buffer (~3 MB vs ~18 MB),
        // so all built-in voices + "صدای من" stay hot in RAM and switching voices is instant.
        let row = self.h * self.d;
        let full = cache.try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?.1;
        let mut compact = Vec::with_capacity(self.l * 2 * off * row);
        for a in 0..self.l * 2 {
            let src = a * self.cap * row;
            compact.extend_from_slice(&full[src..src + off * row]);
        }
        let v = Arc::new((compact, off));
        let mut memo = self.voice_memo.lock().unwrap();
        memo.push((key, v.clone()));
        while memo.len() > 12 {
            memo.remove(0);
        }
        Ok(v)
    }

    fn decode_all(&self, latents: &[Vec<f32>], cancel: &(dyn Fn() -> bool + Sync)) -> Result<Vec<f32>, String> {
        let mut st: Vec<ort::DynValue> = self.dec_init.iter().map(value_of).collect();
        let mut out = Vec::with_capacity(latents.len() * 1920);
        let mut off = 0;
        for (idx, lt) in latents.iter().enumerate() {
            if idx % 2 == 1 && cancel() {
                return Err("cancelled".to_string());
            }
            let un: Vec<f32> = (0..self.ldim).map(|k| lt[k] * self.emb_std[k] + self.emb_mean[k]).collect();
            let mut feeds: Vec<(String, ort::SessionInputValue)> = vec![("latent".into(), Tensor::from_array(([1, 1, self.ldim], un)).unwrap().into_dyn().into())];
            for (i, s) in st.iter().enumerate() {
                feeds.push((self.dec_in[1 + i].clone(), input(s)));
            }
            let res = self.s_dec.run(feeds).map_err(|e| e.to_string())?;
            out.extend_from_slice(res[0].try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?.1);
            let mut small: Vec<(usize, ort::DynValue)> = Vec::new();
            for (j, &o) in self.dec_out_small.iter().enumerate() {
                small.push((self.small_slots[j], copy_value(&res[o])?));
            }
            let mut kvs: Vec<(usize, Vec<f32>)> = Vec::new();
            for (j, &o) in self.dec_out_kv.iter().enumerate() {
                kvs.push((self.cache_slots[j], res[o].try_extract_raw_tensor::<f32>().map_err(|e| e.to_string())?.1.to_vec()));
            }
            drop(res);
            for (slot, v) in small {
                st[slot] = v;
            }
            for (slot, kv) in kvs {
                let (shape, c) = st[slot].try_extract_raw_tensor_mut::<f32>().map_err(|e| e.to_string())?;
                let tcap = shape[2] as usize;
                let row = (shape[3] * shape[4]) as usize;
                let lead = (shape[0] * shape[1]) as usize;
                let steps = self.steps_per_latent;
                for a in 0..lead {
                    for t in 0..steps {
                        let src = (a * steps + t) * row;
                        let dst = (a * tcap + off + t) * row;
                        if dst + row <= c.len() {
                            c[dst..dst + row].copy_from_slice(&kv[src..src + row]);
                        }
                    }
                }
            }
            off += self.steps_per_latent;
        }
        Ok(out)
    }

    fn generate_chunk(&self, voice: &Voice, chunk: &str, temp_scale: f64, rng: &mut rand::rngs::StdRng, cancel: &(dyn Fn() -> bool + Sync)) -> Result<(Vec<Vec<f32>>, bool), String> {
        let tokens = self.sp.encode(chunk);
        let mut text_emb = Vec::with_capacity(tokens.len() * self.dim);
        for &t in &tokens {
            let t = t as usize;
            text_emb.extend_from_slice(&self.lut[t * self.dim..(t + 1) * self.dim]);
        }
        let sd = (self.temp * temp_scale).max(0.14).sqrt();
        let mut noise = || -> Vec<f32> { (0..self.ldim).map(|_| (rng.sample::<f64, _>(rand_distr::StandardNormal) * sd) as f32).collect() };
        let row = self.h * self.d;
        let mut buf = vec![0f32; self.kv_len()];
        for a in 0..self.l * 2 {
            let src = a * voice.off * row;
            let dst = a * self.cap * row;
            buf[dst..dst + voice.off * row].copy_from_slice(&voice.cache[src..src + voice.off * row]);
        }
        let mut cache = Tensor::from_array(([self.l, 2, self.cap, self.h, self.d], buf)).unwrap().into_dyn();
        let nan = vec![f32::NAN; self.ldim];
        let (lat, _, mut off) = self.flow_step(&nan, 1, &text_emb, tokens.len(), voice.off, &noise(), &mut cache)?;
        let mut latents = vec![lat];
        let words = split_ws(chunk).len();
        // Generous post-EOS frames + minimum step floor prevent premature EOS spikes (especially in
        // 8-bit quantized mobile FlowLM) from cutting off the final word or syllable of a chunk.
        let frames_after_eos = if words <= 4 { 6 } else { 5 };
        let min_eos_step = if tokens.len() >= 5 {
            ((tokens.len() as f64 / (self.tps_est * 1.75)) * self.frame_rate).floor() as usize
        } else {
            2
        };
        let max_gen_len = (((tokens.len() as f64 / (self.tps_est * 0.72).max(1.8) + self.gen_pad + 1.2) * self.frame_rate).ceil() as usize)
            .min(self.cap.saturating_sub(off + 1));
        let mut eos_step: Option<usize> = None;
        for step in 0..max_gen_len {
            if step % 2 == 1 && cancel() {
                return Err("cancelled".to_string());
            }
            let last = latents.last().unwrap().clone();
            let (lat, eos, o) = self.flow_step(&last, 1, &[], 0, off, &noise(), &mut cache)?;
            off = o;
            if eos && step >= min_eos_step && eos_step.is_none() {
                eos_step = Some(step);
            }
            if let Some(e) = eos_step {
                if step >= e + frames_after_eos {
                    break;
                }
            }
            latents.push(lat);
        }
        Ok((latents, eos_step.is_some()))
    }


    fn dead_air_regions(&self, p: &[f32], rel_floor: f64, win: f64, density: f64) -> Vec<(usize, usize)> {
        let r = rms(p);
        if !(r >= 1e-6) || p.len() < 3 {
            return Vec::new();
        }
        let quiet: Vec<bool> = p.iter().map(|x| (x.abs() as f64) < rel_floor * r).collect();
        let w = ((win * RATE as f64) as usize).max(1);
        let dens = window_mean(&quiet.iter().map(|q| if *q { 1.0 } else { 0.0 }).collect::<Vec<f32>>(), w);
        let solid: Vec<bool> = dens.iter().map(|d| *d as f64 >= density).collect();
        let mut out: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < solid.len() {
            if solid[i] {
                let a = i;
                while i < solid.len() && solid[i] {
                    i += 1;
                }
                let (mut s, mut e) = (a, i);
                while s > 0 && quiet[s - 1] {
                    s -= 1;
                }
                while e < p.len() && quiet[e] {
                    e += 1;
                }
                out.push((s, e));
            } else {
                i += 1;
            }
        }
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in out {
            if let Some(m) = merged.last_mut() {
                if s <= m.1 {
                    m.1 = m.1.max(e);
                    continue;
                }
            }
            merged.push((s, e));
        }
        merged
    }

    fn solid_fraction(&self, speech: &[f32]) -> f64 {
        if speech.len() < 3 {
            return 1.0;
        }
        let w = ((0.04 * RATE as f64) as usize).max(1);
        let n = speech.len() / w;
        if n == 0 {
            return 1.0;
        }
        let floor = 0.05f64.min(0.5 * rms(speech));
        (0..n).filter(|i| rms(&speech[i * w..(i + 1) * w]) >= floor).count() as f64 / n as f64
    }

    fn speech_bounds(&self, p: &[f32]) -> (usize, usize) {
        let r = rms(p);
        if !(r >= 1e-6) {
            return (0, p.len());
        }
        // Gentle boundary threshold + generous padding preserves soft Persian fricatives/plosives at chunk edges
        let loud: Vec<f32> = p.iter().map(|x| if x.abs() as f64 >= 0.018 * r { 1.0 } else { 0.0 }).collect();
        let w = ((0.025 * RATE as f64) as usize).max(1);
        let dens = window_mean(&loud, 2 * w);
        let idx: Vec<usize> = dens.iter().enumerate().filter(|(_, d)| **d >= 0.30).map(|(i, _)| i).collect();
        if idx.is_empty() {
            return (0, p.len());
        }
        let start = idx[0].saturating_sub((0.060 * RATE as f64) as usize);
        let end = (idx[idx.len() - 1] + (0.120 * RATE as f64) as usize).min(p.len());
        (start, end)
    }

    fn compress_pauses(&self, a: &[f32], max_pause: f64, keep: f64) -> Vec<f32> {
        let regions: Vec<(usize, usize)> = self
            .dead_air_regions(a, 0.03, 0.25, 0.90)
            .into_iter()
            .filter(|(s, e)| e - s > (max_pause * RATE as f64) as usize)
            .collect();
        if regions.is_empty() {
            return a.to_vec();
        }
        let f = ((0.008 * RATE as f64) as usize).max(1);
        let mut out = Vec::with_capacity(a.len());
        let mut pos = 0;
        for (s, e) in regions {
            out.extend_from_slice(&a[pos.min(a.len())..s.max(pos).min(a.len())]);
            let mut seg = a[s..(s + (keep * RATE as f64) as usize).min(e)].to_vec();
            if seg.len() > 2 * f {
                let ramp = audio::linspace(1.0, 0.0, f);
                let n = seg.len();
                for i in 0..f {
                    seg[n - f + i] *= ramp[i];
                }
            }
            out.extend(seg);
            let nxt = &a[e.min(a.len())..(e + f).min(a.len())];
            if nxt.len() == f {
                let ramp = audio::linspace(0.0, 1.0, f);
                out.extend(nxt.iter().zip(ramp.iter()).map(|(x, r)| x * r));
            } else {
                out.extend_from_slice(nxt);
            }
            pos = e + f;
        }
        if pos < a.len() {
            out.extend_from_slice(&a[pos..]);
        }
        out
    }

    fn stitch(&self, segments: Vec<(Vec<f32>, f64)>) -> Vec<f32> {
        if segments.is_empty() {
            return Vec::new();
        }
        let mut levels: Vec<f64> = segments.iter().filter(|(p, _)| !p.is_empty()).map(|(p, _)| rms(p)).collect();
        let target = if levels.is_empty() { 0.0 } else { median(&mut levels) };
        let f = ((0.008 * RATE as f64) as usize).max(1);
        let mut out = Vec::new();
        for (mut p, gap) in segments {
            let r = rms(&p);
            if r > 1e-6 && target > 1e-6 {
                let g = (target / r).clamp(0.4, 2.5) as f32;
                for x in p.iter_mut() {
                    *x *= g;
                }
            }
            if p.len() > 2 * f {
                let n = p.len();
                let denom = f as f32;
                for i in 0..f {
                    let w = 0.5 * (1.0 - (std::f32::consts::PI * i as f32 / denom).cos());
                    p[i] *= w;
                    p[n - f + i] *= 1.0 - w;
                }
            }
            if gap > 0.0 {
                out.extend(std::iter::repeat(0f32).take((gap * RATE as f64) as usize));
            }
            out.extend(p);
        }
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        if peak > 0.98 {
            let g = 0.98 / peak;
            for x in out.iter_mut() {
                *x *= g;
            }
        }
        self.compress_pauses(&out, 0.32, 0.22)
    }


    fn generate_checked(&self, voice: &Voice, chunk: &str, rng: &mut rand::rngs::StdRng, depth: usize, cancel: &(dyn Fn() -> bool + Sync)) -> Result<Vec<f32>, String> {
        let tokens = self.sp.encode(chunk).len();
        let usual = {
            let p = self.pitch.lock().unwrap();
            p.get(voice.id).filter(|v| v.len() >= 5).map(|v| median(&mut v.clone()))
        };
        let rate = {
            let p = self.pace.lock().unwrap();
            p.get(voice.id).filter(|v| v.len() >= 5).map(|v| median(&mut v.clone()))
        };
        let expected_loose = tokens as f64 / self.tps_est;
        let mut best: Option<Vec<f32>> = None;
        let mut best_key: Option<[f64; 12]> = None;
        let mut best_voiced = false;
        let mut best_no_stutter = false;
        let mut best_heard: Option<(Option<f64>, Vec<String>)> = None;
        let allow_repeat = chunk_has_reduplication(chunk);
        let mut cut_short = 0;
        let mut last_wrong: Option<(Vec<String>, bool, bool, bool)> = None;
        let rtf = read_rtf();
        let base_tries = MAX_TRIES.load(std::sync::atomic::Ordering::Relaxed);
        let adaptive_tries = if voice.quick {
            1
        } else if cfg!(shell) {
            if rtf < 1.30 {
                1
            } else if rtf < 1.85 {
                base_tries.min(2)
            } else {
                base_tries.min(3)
            }
        } else if rtf < 1.15 {
            1
        } else if rtf < 1.55 {
            base_tries.min(2)
        } else {
            base_tries
        };
        // Cap live/mobile retries at 2 so a single sentence never stalls playback for 20+ seconds;
        // clean takes still return immediately on take 0!
        let hard_cap = if voice.quick || cfg!(shell) {
            2
        } else {
            adaptive_tries.clamp(2, 3)
        };
        for take in 0..hard_cap {
            if take >= adaptive_tries && best_voiced && best_no_stutter {
                break;
            }
            PROFILE[5].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if take == 0 {
                PROFILE[4].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if cancel() {
                return Err("cancelled".to_string());
            }
            let temp_scale = match take {
                0 => 1.0,
                1 => 0.72,
                _ => 0.55,
            };
            let t = std::time::Instant::now();
            // Never sleep to cool down while starting up or refilling a low playback buffer
            if !voice.quick {
                cool_down();
            }
            let (latents, hit_eos) = self.generate_chunk(voice, chunk, temp_scale, rng, cancel)?;
            lap(0, t);
            // Fast pre-decode health check (<0.05ms): catches exploded/frozen digital-noise latents AND
            // autoregressive word/syllable loops BEFORE spending CPU on Mimi decoding!
            let bad_latents = has_bad_latents(&latents);
            let stuttered = !allow_repeat && has_latent_stutter(&latents);
            if (bad_latents || stuttered || (!hit_eos && words_count_of(chunk) <= 18)) && take + 1 < hard_cap {
                continue;
            }
            if bad_latents {
                continue;
            }
            let t = std::time::Instant::now();
            let a = self.decode_all(&latents, cancel)?;
            lap(1, t);
            let t_check = std::time::Instant::now();
            let finite_ok = !a.is_empty() && a.iter().all(|x| x.is_finite() && x.abs() < 4.0);
            let (s0, e0) = if finite_ok { self.speech_bounds(&a) } else { (0, 0) };
            let speech = if finite_ok && e0 > s0 {
                audio::settle_tail(self.compress_pauses(&a[s0..e0], 0.26, 0.11), RATE)
            } else {
                Vec::new()
            };
            let dur = speech.len() as f64 / RATE as f64;
            let r = if speech.is_empty() { 0.0 } else { rms(&speech) };
            let frac = if speech.is_empty() { 0.0 } else { self.solid_fraction(&speech) };
            let solid = frac * dur;
            let regions = if speech.is_empty() { Vec::new() } else { self.dead_air_regions(&speech, 0.03, 0.25, 0.90) };
            let dead = regions.iter().map(|(i, j)| j - i).max().map(|m| m as f64 / RATE as f64).unwrap_or(0.0);
            let loud_ok = finite_ok && r >= 0.02;
            let mut dur_ok = 0.30 * expected_loose <= solid && solid <= 1.6 * expected_loose;
            let ratio = match rate {
                Some(rt) if tokens >= 6 => solid / (tokens as f64 / rt),
                _ => 1.0,
            };
            let pace_ok = (PACE_LOW..=PACE_HIGH).contains(&ratio);
            let (pitch, voiced_frac) = if loud_ok { audio::voice_quality(&speech, RATE) } else { (None, 0.0) };
            let digital_noise = !loud_ok || audio::is_digital_noise(&speech, RATE);
            // Hard anti-noise gate: reject digital screech/buzz/clipping AND require real vocal harmonics.
            // Short 1-2 word phrases (<0.65s) pass once loud_ok && !digital_noise && !bad_latents hold.
            let voiced_ok = loud_ok && !digital_noise && (dur < 0.65 || (pitch.is_some() && voiced_frac >= 0.14));
            let same_voice = match (pitch, usual) {
                (Some(p), Some(u)) => (p / u).log2().abs() < 0.5,
                (None, Some(_)) if dur >= 0.42 => false,
                _ => voiced_ok,
            };
            lap(2, t_check);
            let no_stutter = !stuttered;
            // In quick/low-buffer or mobile live mode, accept immediately once voice is clean, non-noisy,
            // non-stuttering, finished cleanly (hit_eos), not skipped (too short), AND not bloated (repeated words!),
            // skipping the expensive Vosk ASR resample + Kaldi decode pass whose result would be unused here.
            let min_solid_frac = if tokens >= 6 { 0.48 } else { 0.36 };
            let min_pace_ratio = if tokens >= 6 { 0.66 } else { 0.58 };
            let not_skipped = solid >= min_solid_frac * expected_loose && (rate.is_none() || ratio >= min_pace_ratio);
            let not_bloated = solid <= 1.85 * expected_loose && (rate.is_none() || ratio <= 1.78);
            if (voice.quick || cfg!(shell)) && voiced_ok && no_stutter && loud_ok && (hit_eos || words_count_of(chunk) > 16) && frac >= 0.55 && not_skipped && not_bloated {
                return Ok(speech);
            }
            let t = std::time::Instant::now();
            let (heard, missing) = if voiced_ok && !voice.quick && adaptive_tries > 1 && LISTEN.load(std::sync::atomic::Ordering::Relaxed) { asr::heard_score(chunk, &speech, RATE) } else { (None, Vec::new()) };
            lap(3, t);
            let words_ok = if LISTEN_LIGHT.load(std::sync::atomic::Ordering::Relaxed) || cfg!(shell) { !asr::heard_clear_fault(heard, &missing) } else { asr::heard_ok(heard, &missing) };
            let b = |x: bool| if x { 1.0 } else { 0.0 };
            let key = [b(voiced_ok), b(no_stutter), b(loud_ok), b(same_voice), b(hit_eos), b(words_ok), b(dur_ok), b(pace_ok), -(missing.len() as f64), heard.unwrap_or(1.0),
                       -(1.0 - ratio).abs(), if dead <= 0.35 { 0.0 } else { -dead }];
            if best_key.map_or(true, |bk| key.partial_cmp(&bk) == Some(std::cmp::Ordering::Greater)) {
                best = Some(speech.clone());
                best_key = Some(key);
                best_voiced = voiced_ok;
                best_no_stutter = no_stutter;
                best_heard = Some((heard, missing.clone()));
            }
            let heard_clean = heard.is_some() && words_ok && missing.is_empty() && heard.unwrap() >= 0.9;
            if heard_clean {
                dur_ok = solid <= 1.6 * expected_loose;
            }
            let solid_ok = frac >= 0.55 || heard_clean && frac >= 0.40;
            if (voice.quick || cfg!(shell)) && voiced_ok && no_stutter && loud_ok && (hit_eos || words_count_of(chunk) > 16) && solid_ok && not_skipped && not_bloated {
                return Ok(best.unwrap());
            }
            if voiced_ok && no_stutter && loud_ok && same_voice && hit_eos && words_ok && dur_ok && (pace_ok || heard_clean) && solid_ok && dead <= 0.35 {
                if pace_ok && tokens >= 6 && solid > 0.2 {
                    let mut p = self.pace.lock().unwrap();
                    let v = p.entry(voice.id.to_string()).or_default();
                    v.push(tokens as f64 / solid);
                    let n = v.len();
                    if n > 60 {
                        v.drain(..n - 60);
                    }
                }
                if let Some(pi) = pitch {
                    if dur > 1.0 {
                        let mut p = self.pitch.lock().unwrap();
                        let v = p.entry(voice.id.to_string()).or_default();
                        v.push(pi);
                        let n = v.len();
                        if n > 40 {
                            v.drain(..n - 40);
                        }
                    }
                }
                return Ok(best.unwrap());
            }
            if missing.len() == 1 && missing[0].ends_with('$') && heard.map_or(false, |h| h >= 0.85) {
                cut_short += 1;
            }
            if cut_short >= 2 && best_voiced {
                break;
            }
            let mut miss_sorted = missing.clone();
            miss_sorted.sort();
            let wrong_now = (miss_sorted, words_ok, dur_ok, pace_ok);
            let repeats = last_wrong.as_ref() == Some(&wrong_now) && voiced_ok && loud_ok && same_voice;
            last_wrong = Some(wrong_now);
            if repeats && take >= 1 && best_voiced {
                if let Some((h, m)) = &best_heard {
                    if !asr::heard_bad(*h, m) {
                        return Ok(best.unwrap());
                    }
                }
                break;
            }
            if take >= 2 && best_voiced && best_no_stutter && best_key.unwrap()[3] == 1.0 {
                if let Some((h, m)) = &best_heard {
                    if !asr::heard_bad(*h, m) {
                        return Ok(best.unwrap());
                    }
                }
            }
        }
        let words = split_ws(chunk);
        let split_headroom = if cfg!(shell) { 2.40 } else { 1.25 };
        if !best_voiced || (!voice.quick && !cfg!(shell) && rtf >= split_headroom) {
            if let Some((h, m)) = &best_heard {
                if (!best_voiced || (h.is_some() && asr::heard_bad(*h, m))) && words.len() >= 2 && depth == 0 {
                    let half = words.len() / 2;
                    let a = self.generate_checked(voice, &words[..half].join(" "), rng, 1, cancel)?;
                    let b = self.generate_checked(voice, &words[half..].join(" "), rng, 1, cancel)?;
                    let mut joined = a;
                    joined.extend(std::iter::repeat(0f32).take((RATE as f64 * 0.06) as usize));
                    joined.extend(b);
                    if !best_voiced {
                        return Ok(joined);
                    }
                    let (heard, missing) = asr::heard_score(chunk, &joined, RATE);
                    if heard.is_some() && missing.len() <= m.len() {
                        return Ok(joined);
                    }
                }
            }
        }
        // Never blast unvoiced static/noise into the speaker if every attempt collapsed
        if !best_voiced {
            return Ok(Vec::new());
        }
        Ok(best.unwrap_or_default())
    }


    pub fn plan_text(&self, text: &str) -> Vec<(f64, Vec<(String, f64)>)> {
        self.plan_text_kinds(text).into_iter().map(|(g, p, _)| (g, p)).collect()
    }

    pub fn plan_text_kinds(&self, text: &str) -> Vec<(f64, Vec<(String, f64)>, prosody::Kind)> {
        let mut sentences = Vec::new();
        let mut last: Option<prosody::Kind> = None;
        for para in split(r"\n\s*\n", text) {
            let mut gap = last.map_or(PARAGRAPH_GAP, |k| prosody::gap_after(k, PARAGRAPH_GAP));
            for sent in planner::split_sentences(&para) {
                let plan = match planner::plan_phrases(&sent, &self.g2p, &self.sp) {
                    Ok(p) => p,
                    Err(_) => {
                        let clean = sub(r"[^؀-ۿ‌A-Za-z0-9\s.,،؛:!?؟«»()\-]", " ", &sent);
                        if strip(&clean).is_empty() {
                            continue;
                        }
                        match planner::plan_phrases(&clean, &self.g2p, &self.sp) {
                            Ok(p) => p,
                            Err(_) => continue,
                        }
                    }
                };
                if !plan.is_empty() {
                    let kind = prosody::kind_of(&sent);
                    sentences.push((gap, plan, kind));
                    gap = prosody::gap_after(kind, SENTENCE_GAP);
                    last = Some(kind);
                }
            }
        }
        sentences
    }

    pub fn synthesize_text(&self, text: &str, voice_id: &str, wav: &Path, workers: usize, quick: bool, cancel: &(dyn Fn() -> bool + Sync)) -> Result<Vec<f32>, String> {
        self.synthesize_text_in_order(text, voice_id, wav, workers, quick, None, cancel)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn synthesize_text_in_order(&self, text: &str, voice_id: &str, wav: &Path, workers: usize, quick: bool, order: Option<u64>, cancel: &(dyn Fn() -> bool + Sync)) -> Result<Vec<f32>, String> {
        let sentences = if strip(text).is_empty() { Vec::new() } else { self.plan_text_kinds(text) };
        if sentences.is_empty() {
            return Ok(Vec::new());
        }
        if cancel() {
            return Err("cancelled".to_string());
        }
        let vc = self.voice_cache(wav)?;
        if cancel() {
            return Err("cancelled".to_string());
        }
        let voice = Voice { id: voice_id, cache: &vc.0, off: vc.1, quick };
        let chunker = Chunker { tok: &self.sp };
        let mut jobs: Vec<(usize, usize, String, f64)> = Vec::new();
        for (si, (_, plan, _)) in sentences.iter().enumerate() {
            for (pi, (phrase, pgap)) in plan.iter().enumerate() {
                for (ci, chunk) in chunker.chunk_phonemes(strip(phrase)).into_iter().enumerate() {
                    let gap = if pi == 0 && ci == 0 { 0.0 } else if ci == 0 { *pgap * comma_pause() } else { CHUNK_GAP * comma_pause() };
                    jobs.push((si, pi, chunk, gap));
                }
            }
        }
        let seeds: Vec<u64> = {
            let mut r = self.rng.lock().unwrap();
            (0..jobs.len()).map(|_| r.random::<u64>()).collect()
        };
        let results: Vec<Mutex<Option<Result<Vec<f32>, String>>>> = (0..jobs.len()).map(|_| Mutex::new(None)).collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let ticket = order.unwrap_or_else(|| TICKETS.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        std::thread::scope(|s| {
            for _ in 0..workers.max(slot_count()).max(1).min(jobs.len().max(1)) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= jobs.len() {
                        break;
                    }
                    let r = match Slot::take(ticket, cancel) {
                        Err(e) => Err(e),
                        Ok(_slot) => {
                            if cancel() {
                                Err("cancelled".to_string())
                            } else {
                                let mut rng = rand::rngs::StdRng::seed_from_u64(seeds[i]);
                                self.generate_checked(&voice, &jobs[i].2, &mut rng, 0, cancel)
                            }
                        }
                    };
                    *results[i].lock().unwrap() = Some(r);
                });
            }
        });
        let mut audios = Vec::with_capacity(jobs.len());
        for r in results {
            audios.push(r.into_inner().unwrap().unwrap_or_else(|| Err("no result".into()))?);
        }
        let mut parts: Vec<f32> = Vec::new();
        let mut first = true;
        let natural = self.natural.load(std::sync::atomic::Ordering::Relaxed);
        for (si, (gap, plan, kind)) in sentences.iter().enumerate() {
            let n_phrases = plan.len();
            let mut segs: Vec<(Vec<f32>, f64)> = Vec::new();
            let mut phrase_chunks: Vec<(usize, Vec<f32>, f64)> = jobs
                .iter()
                .zip(audios.iter())
                .filter(|(j, _)| j.0 == si)
                .map(|(j, a)| (j.1, a.clone(), j.3))
                .collect();
            if phrase_chunks.is_empty() {
                continue;
            }
            if natural && n_phrases > 1 {
                // Apply subtle continuation lift to the last chunk of every non-final comma/colon phrase
                for k in 0..phrase_chunks.len() {
                    let pi = phrase_chunks[k].0;
                    let is_phrase_end = k + 1 < phrase_chunks.len() && phrase_chunks[k + 1].0 != pi;
                    if is_phrase_end && pi + 1 < n_phrases {
                        let audio_chunk = std::mem::take(&mut phrase_chunks[k].1);
                        phrase_chunks[k].1 = prosody::shape_clause(audio_chunk, RATE);
                    }
                }
            }
            for (_, a, g) in phrase_chunks {
                segs.push((a, g));
            }
            if !first {
                parts.extend(std::iter::repeat(0f32).take((gap * RATE as f64) as usize));
            }
            first = false;
            let said = self.stitch(segs);
            if natural {
                let said = prosody::shape(said, RATE, *kind);
                let pace = prosody::pace(*kind);
                parts.extend(if pace != 1.0 && !said.is_empty() { crate::stretch::stretch(&said, pace, RATE) } else { said });
            } else {
                parts.extend(said);
            }
        }
        Ok(parts)
    }

    pub fn forget_pace(&self, voice_id: &str) {
        self.pace.lock().unwrap().remove(voice_id);
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn words_count_of(chunk: &str) -> usize {
    split_ws(chunk).len()
}

/// Fast pre-decode check (<0.02 ms) catching diverged/exploded FlowLM latents or frozen
/// robotic-buzz oscillation loops before Mimi decodes them into digital noise.
fn has_bad_latents(latents: &[Vec<f32>]) -> bool {
    if latents.is_empty() {
        return true;
    }
    let mut frozen_run = 0usize;
    let mut prev_norm = 0.0f32;
    for (idx, v) in latents.iter().enumerate() {
        let mut sq = 0.0f32;
        for &x in v {
            if !x.is_finite() || x.abs() > 12.0 {
                return true;
            }
            sq += x * x;
        }
        let norm = sq.sqrt();
        if norm > 18.0 {
            return true;
        }
        if idx > 0 && norm > 1.5 && prev_norm > 1.5 {
            let dot: f32 = v.iter().zip(&latents[idx - 1]).map(|(a, b)| a * b).sum();
            let cos = dot / (norm * prev_norm);
            if cos > 0.992 {
                frozen_run += 1;
                if frozen_run >= 6 {
                    return true;
                }
            } else {
                frozen_run = 0;
            }
        } else {
            frozen_run = 0;
        }
        prev_norm = norm;
    }
    false
}

/// Returns true if the phoneme chunk itself contains an intentional adjacent repetition
/// (e.g. Persian reduplications like "کم‌کم" -> "kamkam" or "دور دور" -> "dur dur").
fn chunk_has_reduplication(chunk: &str) -> bool {
    let words = split_ws(chunk);
    for w in words.windows(2) {
        if w[0].len() >= 2 && w[0] == w[1] {
            return true;
        }
    }
    for w in words {
        let b = w.as_bytes();
        if b.len() >= 6 && b.len() % 2 == 0 && b[..b.len() / 2] == b[b.len() / 2..] {
            return true;
        }
    }
    false
}

/// Detects autoregressive syllable/word repetition loops (e.g. "شکل‌گیری‌گیری") directly in the
/// 12.5 Hz latent trajectory before decoding (~0.05 ms).
fn has_latent_stutter(latents: &[Vec<f32>]) -> bool {
    let n = latents.len();
    if n < 10 {
        return false;
    }
    let norms: Vec<f32> = latents.iter().map(|v| v.iter().map(|x| x * x).sum::<f32>().sqrt()).collect();
    let mut sorted_norms: Vec<f64> = norms.iter().map(|&x| x as f64).collect();
    let med_norm = median(&mut sorted_norms) as f32;
    if !(med_norm > 1e-4) {
        return false;
    }
    let cos = |a: usize, b: usize| -> f32 {
        let d = norms[a] * norms[b];
        if d < 1e-6 {
            return 0.0;
        }
        let dot: f32 = latents[a].iter().zip(&latents[b]).map(|(x, y)| x * y).sum();
        dot / d
    };
    for k in 4..=14usize {
        let w = k.clamp(5, 8);
        if n < k + w {
            break;
        }
        for t in 0..=(n - k - w) {
            let active = (0..w).all(|j| norms[t + j] >= 0.50 * med_norm && norms[t + k + j] >= 0.50 * med_norm);
            if !active {
                continue;
            }
            let mut internal_sim = 0.0f32;
            for j in 0..(w - 1) {
                internal_sim += cos(t + j, t + j + 1);
            }
            if internal_sim / (w - 1) as f32 > 0.86 {
                continue;
            }
            let mut min_c = 1.0f32;
            let mut sum_c = 0.0f32;
            for j in 0..w {
                let c = cos(t + j, t + k + j);
                if c < min_c {
                    min_c = c;
                }
                sum_c += c;
            }
            if sum_c / w as f32 >= 0.955 && min_c >= 0.90 {
                return true;
            }
        }
    }
    false
}

fn window_mean(x: &[f32], w: usize) -> Vec<f32> {
    let n = x.len();
    let mut c = vec![0f64; n + 1];
    for i in 0..n {
        c[i + 1] = c[i] + x[i] as f64;
    }
    let start = (w - 1) / 2;
    (0..n)
        .map(|i| {
            let k = i + start;
            let lo = (k as isize - w as isize + 1).max(0) as usize;
            let hi = (k + 1).min(n);
            ((c[hi] - c[lo.min(hi)]) / w as f64) as f32
        })
        .collect()
}

pub fn trim_hot_onset(a: Vec<f32>) -> Vec<f32> {
    let sr = RATE;
    if a.len() < sr {
        return a;
    }
    let h = ((0.01 * sr as f64) as usize).max(1);
    let nf = a.len() / h;
    let peak = a.iter().fold(0f32, |m, x| m.max(x.abs())) as f64;
    if nf < 8 || peak < 1e-6 {
        return a;
    }
    let frames: Vec<f64> = (0..nf).map(|i| rms(&a[i * h..(i + 1) * h])).collect();
    let loud: Vec<bool> = frames.iter().map(|f| *f > 0.02 * peak).collect();
    let mut speech: Vec<f64> = frames.iter().zip(&loud).filter(|(_, l)| **l).map(|(f, _)| *f).collect();
    if speech.is_empty() {
        return a;
    }
    let typical = median(&mut speech);
    let onset = loud.iter().position(|l| *l).unwrap_or(0);
    let attack = &frames[onset..(onset + 30).min(frames.len())];
    let loud_enough = typical * 10f64.powf(4.0 / 20.0);
    let attack_rms = (attack.iter().map(|x| x * x).sum::<f64>() / attack.len() as f64).sqrt();
    let attack_max = attack.iter().fold(0f64, |m, x| m.max(*x));
    let hot = attack_rms > loud_enough || (peak >= 0.99 && attack_max > loud_enough);
    if !hot {
        return a;
    }
    let drop = onset * h + (0.3 * sr as f64) as usize;
    if (a.len() as f64 - drop as f64) < 1.5 * sr as f64 {
        return a;
    }
    let mut out = a[drop..].to_vec();
    let f = ((0.02 * sr as f64) as usize).max(1);
    if out.len() > 2 * f {
        let ramp = audio::linspace(0.0, 1.0, f);
        for i in 0..f {
            out[i] *= ramp[i];
        }
    }
    out
}

pub fn voice_file(models: &Path, extra: bool, wav: &str) -> PathBuf {
    if extra { models.join("voices").join(wav) } else { models.join("parsigo/voices").join(wav) }
}

pub fn speech_clean(text: &str) -> String {
    let t = sub(r"\s+/\s+", "، ", text);
    let t = sub(r"(?<=[\x{0621}-\x{06D3}])/(?=[\x{0621}-\x{06D3}])", "، ", &t);
    let t = sub(r"_+", " ", &t);
    let t = sub(r"‌+(?=\s)|(?<=\s)‌+", "", &t);
    let t = if crate::vocab::is_finglish() {
        t
    } else {
        sub_fn(r"\s*\(([^()]*)\)", &t, |c| {
            let inner = sub(r"صص?\.?\s*[\d۰-۹]+(?:\s*[-–]\s*[\d۰-۹]+)?|یادداشت|[\d۰-۹]+", " ", g(c, 1).unwrap_or(""));
            let latin = findall(r"[A-Za-zÀ-ž]{2,}", &inner).len();
            if latin >= 2 && !search(r"[\x{0621}-\x{064A}\x{0671}-\x{06D3}]", &inner) { String::new() } else { g(c, 0).unwrap_or("").to_string() }
        })
    };
    let t = sub(r"(?<![\w\x{200c}])ص(?:\.\s*|\s+)(?=[\d۰-۹])", "صفحه\u{200c}ی ", &t);
    sub(r"(?<!\S)[^\w\s«»()]+(?!\S)", " ", &t)
}


