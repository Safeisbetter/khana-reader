
use crate::ffi::wipe;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};


const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
    0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
    0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
    0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
    0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

#[derive(Clone)]
struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    fill: usize,
    total: u64,
}

impl Sha256 {
    fn new() -> Sha256 {
        Sha256 { state: H0, buf: [0; 64], fill: 0, total: 0 }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.fill > 0 {
            let take = (64 - self.fill).min(data.len());
            self.buf[self.fill..self.fill + take].copy_from_slice(&data[..take]);
            self.fill += take;
            data = &data[take..];
            if self.fill == 64 {
                let block = self.buf;
                compress(&mut self.state, &block);
                self.fill = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            compress(&mut self.state, &block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.fill = data.len();
        }
    }

    fn finish(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        let mut pad = [0u8; 72];
        pad[0] = 0x80;
        let zeros = (119 - self.fill) % 64;
        let n = 1 + zeros;
        pad[n..n + 8].copy_from_slice(&bits.to_be_bytes());
        self.update(&pad[..n + 8]);
        let mut out = [0u8; 32];
        for (i, v) in self.state.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
        }
        out
    }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finish()
}

struct Hmac {
    inner: Sha256,
    outer: Sha256,
}

impl Hmac {
    fn new(key: &[u8]) -> Hmac {
        let mut block = [0u8; 64];
        if key.len() > 64 {
            block[..32].copy_from_slice(&sha256(key));
        } else {
            block[..key.len()].copy_from_slice(key);
        }
        let (mut ipad, mut opad) = ([0u8; 64], [0u8; 64]);
        for i in 0..64 {
            ipad[i] = block[i] ^ 0x36;
            opad[i] = block[i] ^ 0x5c;
        }
        let (mut inner, mut outer) = (Sha256::new(), Sha256::new());
        inner.update(&ipad);
        outer.update(&opad);
        wipe::bytes(&mut block);
        wipe::bytes(&mut ipad);
        wipe::bytes(&mut opad);
        Hmac { inner, outer }
    }

    fn mac(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut i = self.inner.clone();
        for p in parts {
            i.update(p);
        }
        let mut o = self.outer.clone();
        o.update(&i.finish());
        o.finish()
    }
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    Hmac::new(key).mac(&[data])
}

pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]) {
    let h = Hmac::new(password);
    for (index, chunk) in out.chunks_mut(32).enumerate() {
        let mut u = h.mac(&[salt, &(index as u32 + 1).to_be_bytes()]);
        let mut t = u;
        for _ in 1..iterations.max(1) {
            u = h.mac(&[&u]);
            for (a, b) in t.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }
        chunk.copy_from_slice(&t[..chunk.len()]);
        wipe::bytes(&mut u);
        wipe::bytes(&mut t);
    }
}

fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}


const ITERATIONS: u32 = 300_000;
pub const MIN_CHARS: usize = 4;
pub const MAX_CHARS: usize = 128;
const MAX_BYTES: usize = 2048;
pub const TIMEOUTS: [i64; 5] = [-1, 60, 300, 1800, 3600];
const DEFAULT_TIMEOUT: i64 = 300;
const FREE_TRIES: u32 = 5;

fn delay(fails: u32) -> u64 {
    match fails {
        0..=4 => 0,
        5 => 30,
        6 => 60,
        7 => 120,
        8 => 300,
        9 => 900,
        _ => 3600,
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Stored {
    v: u32,
    salt: String,
    iter: u32,
    hash: String,
    timeout: i64,
    fails: u32,
    until: u64,
}

impl Stored {
    fn valid(&self) -> bool {
        self.v == 1
            && from_hex(&self.salt).is_some_and(|s| s.len() == 16)
            && from_hex(&self.hash).is_some_and(|h| h.len() == 32)
            && (1..=10_000_000).contains(&self.iter)
            && TIMEOUTS.contains(&self.timeout)
    }
}

#[derive(Debug, PartialEq)]
pub enum LockError {
    Wrong { wait: u64, left: u32 },
    Wait(u64),
    Short,
    Long,
    NeedCurrent,
    NotEnabled,
    BadTimeout,
    Io(String),
}

fn fa(n: u64) -> String {
    n.to_string().chars().map(|c| char::from_u32('۰' as u32 + c as u32 - '0' as u32).unwrap_or(c)).collect()
}

fn waiting(secs: u64) -> String {
    if secs < 60 { format!("{} ثانیه", fa(secs.max(1))) } else { format!("{} دقیقه", fa(secs.div_ceil(60))) }
}

impl LockError {
    pub fn status(&self) -> u16 {
        match self {
            LockError::Wrong { .. } => 403,
            LockError::Wait(_) => 429,
            LockError::Io(_) => 500,
            _ => 400,
        }
    }

    pub fn message(&self) -> String {
        match self {
            LockError::Wrong { wait, left } if *wait > 0 => format!("رمز درست نیست. چند بار اشتباه شد؛ {} صبر کنید.", waiting(*wait)),
            LockError::Wrong { left, .. } if *left > 0 && *left < FREE_TRIES => {
                format!("رمز درست نیست. {} بار دیگر می‌توانید امتحان کنید، بعد باید صبر کنید.", fa(*left as u64))
            }
            LockError::Wrong { .. } => "رمز درست نیست.".to_string(),
            LockError::Wait(w) => format!("چند بار رمز اشتباه وارد شد؛ {} صبر کنید.", waiting(*w)),
            LockError::Short => format!("رمز باید دست‌کم {} نویسه باشد.", fa(MIN_CHARS as u64)),
            LockError::Long => format!("رمز بیشتر از {} نویسه نمی‌تواند باشد.", fa(MAX_CHARS as u64)),
            LockError::NeedCurrent => "رمز فعلی را وارد کنید.".to_string(),
            LockError::NotEnabled => "قفل روشن نیست.".to_string(),
            LockError::BadTimeout => "این زمان درست نیست.".to_string(),
            LockError::Io(_) => "قفل ذخیره نشد.".to_string(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Status {
    pub enabled: bool,
    pub locked: bool,
    pub wait: u64,
    pub timeout: i64,
}

struct State {
    rec: Option<Stored>,
    corrupt: bool,
    locked: bool,
    last: u64,
}

pub struct Lock {
    path: PathBuf,
    st: Mutex<State>,
    gate: Mutex<()>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn normalize(pw: &str) -> String {
    crate::pyre::nfkc(pw)
        .chars()
        .map(|c| match c {
            '۰'..='۹' => char::from_u32('0' as u32 + c as u32 - '۰' as u32).unwrap_or(c),
            '٠'..='٩' => char::from_u32('0' as u32 + c as u32 - '٠' as u32).unwrap_or(c),
            _ => c,
        })
        .collect()
}

fn derive(pw: &str, salt: &[u8], iter: u32) -> [u8; 32] {
    let mut n = normalize(pw);
    let mut out = [0u8; 32];
    pbkdf2_sha256(n.as_bytes(), salt, iter, &mut out);
    wipe::string(&mut n);
    out
}

impl Lock {
    pub fn new(path: PathBuf, now: u64) -> Lock {
        let (rec, corrupt) = match std::fs::read(&path) {
            Ok(b) => match serde_json::from_slice::<Stored>(&b) {
                Ok(s) if s.valid() => (Some(s), false),
                _ => (None, true),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, false),
            Err(_) => (None, true),
        };
        let locked = rec.is_some() || corrupt;
        Lock { path, st: Mutex::new(State { rec, corrupt, locked, last: now }), gate: Mutex::new(()) }
    }

    fn st(&self) -> MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn save(&self, rec: &Stored) -> Result<(), LockError> {
        let json = serde_json::to_vec(rec).map_err(|e| LockError::Io(e.to_string()))?;
        crate::private_fs::atomic_write(&self.path, &json).map_err(|e| LockError::Io(e.to_string()))
    }

    pub fn enabled(&self) -> bool {
        let s = self.st();
        s.rec.is_some() || s.corrupt
    }

    pub fn is_locked(&self) -> bool {
        self.st().locked
    }

    pub fn tick(&self, now: u64) {
        let mut s = self.st();
        if s.locked {
            return;
        }
        let last = s.last;
        if let Some(r) = &s.rec {
            if r.timeout > 0 && (now < last || now - last >= r.timeout as u64) {
                s.locked = true;
            }
        }
    }

    pub fn touch(&self, now: u64) {
        let mut s = self.st();
        if !s.locked {
            s.last = now;
        }
    }

    pub fn status(&self, now: u64) -> Status {
        let s = self.st();
        Status {
            enabled: s.rec.is_some() || s.corrupt,
            locked: s.locked,
            wait: s.rec.as_ref().map_or(0, |r| r.until.saturating_sub(now).min(3600)),
            timeout: s.rec.as_ref().map_or(-1, |r| r.timeout),
        }
    }

    pub fn lock_now(&self) {
        let mut s = self.st();
        if s.rec.is_some() || s.corrupt {
            s.locked = true;
        }
    }

    fn attempt(&self, pw: &str, now: u64) -> Result<(), LockError> {
        let _one_at_a_time = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let rec = {
            let s = self.st();
            match &s.rec {
                Some(r) => {
                    let wait = r.until.saturating_sub(now).min(3600);
                    if wait > 0 {
                        return Err(LockError::Wait(wait));
                    }
                    r.clone()
                }
                None => return Err(LockError::Wrong { wait: 0, left: 0 }),
            }
        };
        let salt = from_hex(&rec.salt).unwrap_or_default();
        let want = from_hex(&rec.hash).unwrap_or_default();
        let mut got = if pw.len() <= MAX_BYTES { derive(pw, &salt, rec.iter) } else { [0u8; 32] };
        let ok = same_bytes(&got, &want);
        wipe::bytes(&mut got);
        let ok = ok && pw.len() <= MAX_BYTES;
        let mut s = self.st();
        let Some(cur) = s.rec.as_mut() else { return Err(LockError::NotEnabled) };
        if ok {
            if cur.fails != 0 || cur.until != 0 {
                cur.fails = 0;
                cur.until = 0;
                let snapshot = cur.clone();
                let _ = self.save(&snapshot);
            }
            Ok(())
        } else {
            cur.fails = cur.fails.saturating_add(1);
            let wait = delay(cur.fails);
            cur.until = now + wait;
            let (fails, snapshot) = (cur.fails, cur.clone());
            let _ = self.save(&snapshot);
            Err(LockError::Wrong { wait, left: FREE_TRIES.saturating_sub(fails) })
        }
    }

    pub fn unlock(&self, pw: &str, now: u64) -> Result<(), LockError> {
        self.attempt(pw, now)?;
        let mut s = self.st();
        s.locked = false;
        s.last = now;
        Ok(())
    }

    pub fn set_password(&self, pw: &str, current: Option<&str>, now: u64) -> Result<(), LockError> {
        if pw.len() > MAX_BYTES {
            return Err(LockError::Long);
        }
        let n = normalize(pw);
        let chars = n.chars().count();
        if chars < MIN_CHARS {
            return Err(LockError::Short);
        }
        if chars > MAX_CHARS {
            return Err(LockError::Long);
        }
        let had = self.enabled();
        if had {
            self.attempt(current.ok_or(LockError::NeedCurrent)?, now)?;
        }
        let salt = crate::app::random_bytes::<16>();
        let mut hash = derive(pw, &salt, ITERATIONS);
        let timeout = self.st().rec.as_ref().map_or(DEFAULT_TIMEOUT, |r| r.timeout);
        let rec = Stored { v: 1, salt: to_hex(&salt), iter: ITERATIONS, hash: to_hex(&hash), timeout, fails: 0, until: 0 };
        wipe::bytes(&mut hash);
        self.save(&rec)?;
        let mut s = self.st();
        s.rec = Some(rec);
        s.corrupt = false;
        s.locked = false;
        s.last = now;
        Ok(())
    }

    pub fn disable(&self, pw: &str, now: u64) -> Result<(), LockError> {
        if !self.enabled() {
            return Err(LockError::NotEnabled);
        }
        self.attempt(pw, now)?;
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(LockError::Io(e.to_string())),
        }
        let mut s = self.st();
        s.rec = None;
        s.corrupt = false;
        s.locked = false;
        Ok(())
    }

    pub fn set_timeout(&self, secs: i64, now: u64) -> Result<(), LockError> {
        if !TIMEOUTS.contains(&secs) {
            return Err(LockError::BadTimeout);
        }
        let mut s = self.st();
        let Some(r) = s.rec.as_mut() else { return Err(LockError::NotEnabled) };
        r.timeout = secs;
        let snapshot = r.clone();
        self.save(&snapshot)?;
        s.last = now;
        Ok(())
    }

    pub fn erase(&self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("tmp"));
        let mut s = self.st();
        s.rec = None;
        s.corrupt = false;
        s.locked = false;
    }
}

