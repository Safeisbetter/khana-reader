
use std::path::{Path, PathBuf};

pub use imp::*;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;

    pub fn data_dir() -> PathBuf {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/tmp"));
        home.join("Library/Application Support").join(crate::app::APP_NAME)
    }

    pub fn save_dir() -> PathBuf {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
        home.join("Downloads")
    }

    pub fn awake(reason: &std::ffi::CStr) -> Option<crate::ffi::power::Awake> {
        crate::ffi::power::Awake::new(reason)
    }

    pub fn saved(_path: &Path, _kind: &str) {}

    pub fn reveal(p: &Path) {
        crate::mac::reveal(p)
    }

    pub fn to_wav24k(src: &Path, dst: &Path) -> bool {
        std::process::Command::new("/usr/bin/afconvert")
            .args(["-f", "WAVE", "-d", "LEI16@24000", "-c", "1"])
            .arg(src)
            .arg(dst)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn farsi_pdf(blocks: &[crate::ffi::typeset::Block], font: &[u8], path: &Path) -> Result<(), String> {
        let pdf = crate::ffi::typeset::farsi_pdf(blocks, font)?;
        std::fs::write(path, pdf).map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    pub struct Dirs {
        pub data: PathBuf,
        pub save: PathBuf,
        pub web: PathBuf,
        pub models: PathBuf,
        pub ort: PathBuf,
    }

    static DIRS: OnceLock<Dirs> = OnceLock::new();

    pub fn set_dirs(d: Dirs) {
        crate::vocab::set_models_dir(d.models.clone());
        let _ = DIRS.set(d);
    }

    fn dirs() -> &'static Dirs {
        DIRS.get().expect("the app's folders are set at start")
    }

    pub fn data_dir() -> PathBuf {
        dirs().data.clone()
    }

    pub fn save_dir() -> PathBuf {
        let d = dirs().save.clone();
        let _ = std::fs::create_dir_all(&d);
        d
    }

    pub fn web_dir() -> PathBuf {
        dirs().web.clone()
    }

    pub fn ort_library() -> PathBuf {
        dirs().ort.clone()
    }


    static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static RUNNING: AtomicUsize = AtomicUsize::new(0);

    fn event(kind: &str, path: &Path) {
        let p = path.to_string_lossy().replace(['\n', '\r', '\t'], " ");
        EVENTS.lock().unwrap_or_else(|e| e.into_inner()).push(format!("{kind}\t{p}"));
    }

    pub fn take_events() -> String {
        std::mem::take(&mut *EVENTS.lock().unwrap_or_else(|e| e.into_inner())).join("\n")
    }

    pub fn busy() -> bool {
        RUNNING.load(Ordering::SeqCst) > 0
    }

    pub struct Awake(());

    impl Drop for Awake {
        fn drop(&mut self) {
            RUNNING.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn awake(_reason: &std::ffi::CStr) -> Option<Awake> {
        RUNNING.fetch_add(1, Ordering::SeqCst);
        Some(Awake(()))
    }

    pub fn saved(path: &Path, kind: &str) {
        event(kind, path);
    }

    pub fn reveal(p: &Path) {
        event("reveal", p);
    }

    pub fn to_wav24k(src: &Path, dst: &Path) -> bool {
        let Ok((a, rate)) = crate::audio::read_wav(src) else { return false };
        let rate = rate as usize;
        if rate == 0 {
            return false;
        }
        let g = gcd(rate, 24000);
        let a = if rate == 24000 { a } else { crate::audio::resample_poly(&a, 24000 / g, rate / g) };
        std::fs::write(dst, crate::audio::wav_bytes(&a, 24000)).is_ok()
    }

    fn gcd(a: usize, b: usize) -> usize {
        if b == 0 { a } else { gcd(b, a % b) }
    }

    pub fn farsi_pdf(blocks: &[crate::ffi::typeset::Block], _font: &[u8], path: &Path) -> Result<(), String> {
        std::fs::write(path, crate::ffi::typeset::records(blocks)).map_err(|e| e.to_string())
    }
}
