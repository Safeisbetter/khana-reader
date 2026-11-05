
use crate::app::App;
use crate::platform::{set_dirs, Dirs};
use std::path::PathBuf;
use std::sync::OnceLock;

static APP: OnceLock<App> = OnceLock::new();

fn app() -> &'static App {
    APP.get_or_init(|| App::new(crate::engines::engines()))
}

pub fn init(data: String, save: String, web: String, models: String, ort: String) {
    set_dirs(Dirs { data: data.into(), save: save.into(), web: web.into(), models: models.into(), ort: ort.into() });
    crate::ffi::lower_priority();
    app();
}

pub fn token() -> String {
    app().token.clone()
}

pub fn handle(method: &str, path: &str, query: &str, token: Option<&str>, body: &[u8]) -> Vec<u8> {
    let web: PathBuf = crate::platform::web_dir();
    let r = app().handle(method, path, query, token, body, &web);
    let mut headers: Vec<(&str, &str)> = vec![
        ("Content-Type", r.ctype),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "no-referrer"),
    ];
    headers.extend(r.headers.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    let mut out = Vec::with_capacity(r.body.len() + 512);
    out.extend((r.status as u32).to_le_bytes());
    out.extend((headers.len() as u32).to_le_bytes());
    for (k, v) in headers {
        for s in [k, v] {
            out.extend((s.len() as u32).to_le_bytes());
            out.extend(s.as_bytes());
        }
    }
    out.extend(r.body);
    out
}

pub fn failure() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(500u32.to_le_bytes());
    out.extend(1u32.to_le_bytes());
    for s in ["Content-Type", "text/plain"] {
        out.extend((s.len() as u32).to_le_bytes());
        out.extend(s.as_bytes());
    }
    out
}

pub fn shutdown() {
    if let Some(a) = APP.get() {
        a.shutdown();
    }
}
