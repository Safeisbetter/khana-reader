
use ort::{GraphOptimizationLevel, Session};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(windows)]
fn dylib() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    match exe.parent().map(|d| d.join("onnxruntime.dll")) {
        Some(f) if f.is_file() => f,
        _ => PathBuf::from("onnxruntime.dll"),
    }
}

#[cfg(all(not(shell), not(windows)))]
fn dylib() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    if let Some(c) = exe.parent().and_then(|p| p.parent()) {
        let f = c.join("Frameworks/libonnxruntime.dylib");
        if f.is_file() {
            return f;
        }
    }
    PathBuf::from(".").join("vendor/lib/libonnxruntime.dylib")
}

#[cfg(shell)]
fn dylib() -> PathBuf {
    crate::platform::ort_library()
}

pub fn init() {
    static I: OnceLock<()> = OnceLock::new();
    I.get_or_init(|| {
        let _ = ort::init_from(dylib().to_string_lossy()).with_name("khana").commit();
    });
}

pub fn session(path: &Path, intra: Option<usize>) -> ort::Result<Session> {
    init();
    let mut b = Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?;
    if let Some(n) = intra {
        b = b.with_intra_threads(n)?.with_inter_threads(1)?;
    }
    b.commit_from_file(path)
}
