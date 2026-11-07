// A check of the core without a window (run by the Windows build on GitHub, and usable anywhere): a text is opened and read, the voice
// speaks, a WAV is saved, a picture is read. Prints one line per step and ends with a non-zero code if a step failed.
use serde_json::{json, Value};

fn main() {
    // (the folder finished files go to may not exist on a new machine)
    let _ = std::fs::create_dir_all(khana::platform::save_dir());
    let app = khana::app::App::new(khana::engines::engines());
    let web = khana::vocab::web_dir();
    let tok = app.token.clone();
    let mut failed = 0;
    let mut step = |name: &str, ok: bool, what: String| {
        println!("{} {name}: {what}", if ok { "OK  " } else { "FAIL" });
        if !ok {
            failed += 1;
        }
    };
    let call = |method: &str, path: &str, q: &str, body: &[u8]| app.handle(method, path, q, Some(&tok), body, &web);

    let r = call("GET", "/api/init", "", &[]);
    let v: Value = serde_json::from_slice(&r.body).unwrap_or_default();
    let voices = v["voices"].as_array().map_or(0, |a| a.len());
    step("voices", voices >= 1, format!("{voices} voice(s), status {}", r.status));

    let text = "این یک آزمایش است. متن فارسی خوانده می‌شود.\n\nپاراگراف دوم برای آزمایش صدا و ذخیره‌ی فایل است.";
    let r = call("POST", "/api/open", "name=check.txt", text.as_bytes());
    let d: Value = serde_json::from_slice(&r.body).unwrap_or_default();
    let id = d["id"].as_str().unwrap_or("").to_string();
    step("open", r.status == 200 && !id.is_empty(), format!("pages {}, language {}", d["pages"], d["lang"]));

    let r = call("GET", "/api/page", &format!("doc={id}&n=1&rev=false&yeh=true&tr=false"), &[]);
    let p: Value = serde_json::from_slice(&r.body).unwrap_or_default();
    let n = p["paragraphs"].as_array().map_or(0, |a| a.len());
    step("page", n == 2, format!("{n} paragraph(s)"));

    let body = json!({"text": "سلام، این یک آزمایش است.", "voice": "khabari", "speed": 1.0}).to_string();
    let t = std::time::Instant::now();
    let r = call("POST", "/api/tts", "", body.as_bytes());
    step("speech", r.status == 200 && r.body.len() > 20000, format!("status {}, {} bytes of WAV in {:.1} s", r.status, r.body.len(), t.elapsed().as_secs_f64()));

    let body = json!({"doc": id, "first": 1, "last": 1, "voice": "khabari", "speed": 1.0, "format": "wav"}).to_string();
    let r = call("POST", "/api/mp3", "", body.as_bytes());
    let j: Value = serde_json::from_slice(&r.body).unwrap_or_default();
    let jid = j["id"].as_str().unwrap_or("").to_string();
    let mut last = Value::Null;
    for _ in 0..240 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let r = call("GET", "/api/job", &format!("id={jid}"), &[]);
        last = serde_json::from_slice(&r.body).unwrap_or_default();
        if last["status"] != "running" {
            break;
        }
    }
    step("save WAV", last["status"] == "done", format!("{} {} saved {}", last["status"], last["error"], last["saved"]));

    // a picture: white, with a black bar (nothing to read: the recognizer must run and answer without an error)
    let (w, h) = (800usize, 300usize);
    let mut img = Vec::with_capacity(8 + w * h);
    img.extend((w as u32).to_le_bytes());
    img.extend((h as u32).to_le_bytes());
    for y in 0..h {
        for _ in 0..w {
            img.push(if (100..140).contains(&y) { 0 } else { 255 });
        }
    }
    let r = call("POST", "/api/ocr_image", "", &img);
    step("picture", r.status == 200, format!("status {} {}", r.status, String::from_utf8_lossy(&r.body).chars().take(80).collect::<String>()));

    if failed > 0 {
        std::process::exit(1);
    }
}
