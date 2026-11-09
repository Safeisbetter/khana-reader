
#![forbid(unsafe_code)]

use khana::doc::{open_document, Engines, Opts};
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("dump") => dump(&args[2], &args[3..]),
        Some("plans") => plans(&args[2], &args[3]),
        Some("say") => say(&args[2], &args[3], args.get(4).map(|s| s.as_str()).unwrap_or("male_news.wav")),
        Some("hear") => hear(&args[2]),
        Some("api") => api_test(&args[2]),
        Some("finish") => {
            let pages: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
            let out: Vec<Vec<String>> = pages.iter().map(|p| {
                let lines: Vec<khana::ocr::Line> = p["lines"].as_array().map(|ls| ls.iter().map(|l| khana::ocr::Line {
                    t: l["t"].as_str().unwrap().to_string(), x0: l["x0"].as_f64().unwrap(), x1: l["x1"].as_f64().unwrap(),
                    top: l["top"].as_f64().unwrap(), bot: l["bot"].as_f64().unwrap() }).collect()).unwrap_or_default();
                if lines.is_empty() { return Vec::new(); }
                let paras: Vec<String> = khana::ocr::ocr_paragraphs(&lines).into_iter()
                    .map(|q| if khana::text::rtl(&q) { khana::ocr::fix_ocr_farsi(&q) } else { q }).collect();
                if paras.iter().map(|q| khana::pyre::split_ws(q).len()).sum::<usize>() <= 2 { Vec::new() } else { paras }
            }).collect();
            println!("{}", serde_json::to_string(&out).unwrap());
        }
        Some("fix") => {
            let items: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
            let out: Vec<String> = items.iter().map(|t| khana::ocr::fix_ocr_farsi(t)).collect();
            println!("{}", serde_json::to_string(&out).unwrap());
        }
        Some("pages") => {
            let data = std::fs::read(&args[2]).unwrap();
            let name = std::path::Path::new(&args[2]).file_name().unwrap().to_string_lossy().into_owned();
            let d = khana::doc::open_document(&name, &data, std::sync::Arc::new(khana::engines::engines())).unwrap();
            let (a, b): (usize, usize) = (args[3].parse().unwrap(), args[4].parse().unwrap());
            let out: Vec<Vec<String>> = (a..=b.min(d.n_pages)).map(|n| d.page_text(n, khana::doc::Opts { reversed: false, persian_yeh: true, translate: false })).collect();
            println!("{}", serde_json::to_string(&out).unwrap());
        }
        Some("sp") => {
            for w in &args[2..] {
                println!("{w} {:?} vocab={}", khana::vocab::nllb_sp().unwrap().encode(w), khana::vocab::in_vocab(w));
            }
        }
        Some("vocab") => {
            let mut w: Vec<&String> = khana::vocab::farsi_words().iter().collect();
            w.sort();
            for x in w { println!("{x}"); }
        }
        Some("tr") => {
            let data = std::fs::read(&args[2]).unwrap();
            let name = std::path::Path::new(&args[2]).file_name().unwrap().to_string_lossy().into_owned();
            let d = khana::doc::open_document(&name, &data, std::sync::Arc::new(khana::engines::engines())).unwrap();
            let n: usize = args[3].parse().unwrap();
            let t0 = std::time::Instant::now();
            let out = d.page_text(n, khana::doc::Opts { reversed: false, persian_yeh: true, translate: true });
            eprintln!("lang {} in {:.1} s", d.lang(), t0.elapsed().as_secs_f64());
            println!("{}", serde_json::to_string(&out).unwrap());
        }
        _ => eprintln!("usage: khana-cli dump OUT_DIR files..."),
    }
}

fn dump(out_dir: &str, files: &[String]) {
    std::fs::create_dir_all(out_dir).unwrap();
    let engines = Arc::new(Engines { ocr: Some(Box::new(|_, _, _| vec!["<<OCR>>".to_string()])), translate: None });
    let jobs: usize = std::env::var("JOBS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let queue = std::sync::Mutex::new(files.to_vec());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let Some(path) = queue.lock().unwrap().pop() else { break };
                let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
                let data = std::fs::read(&path).unwrap_or_default();
                let res = match open_document(&name, &data, engines.clone()) {
                    Ok(doc) => {
                        let st = doc.st.lock().unwrap();
                        let mut scanned: Vec<usize> = st.scanned.iter().copied().collect();
                        scanned.sort();
                        let pages_raw = st.pages.clone();
                        drop(st);
                        let opts = Opts { reversed: false, persian_yeh: true, translate: false };
                        let text: Vec<Vec<String>> = (1..=doc.n_pages).map(|n| doc.page_text(n, opts)).collect();
                        let fix: serde_json::Map<String, serde_json::Value> =
                            doc.font_fix.iter().map(|(c, r)| (c.to_string(), serde_json::Value::String(r.clone()))).collect();
                        serde_json::json!({"name": name, "lang": doc.lang(), "scanned": scanned, "had_scans": doc.had_scans,
                                           "pages_raw": pages_raw, "font_fix": fix, "text": text})
                    }
                    Err(e) => serde_json::json!({"name": name, "error": e}),
                };
                std::fs::write(format!("{out_dir}/{name}.json"), serde_json::to_string(&res).unwrap()).unwrap();
                println!("{name}");
            });
        }
    });
}

fn plans(input: &str, output: &str) {
    use khana::planner::{plan_phrases, split_sentences, Chunker};
    let models = khana::vocab::models_dir().join("parsigo");
    let eng = khana::tts::Engine::new(&models.join("model")).unwrap();
    eng.g2p.set_lexicon(&[]);
    let items: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(input).unwrap()).unwrap();
    let ch = Chunker { tok: &eng.sp };
    let t0 = std::time::Instant::now();
    let mut out = Vec::new();
    for it in &items {
        let text = khana::tts::speech_clean(it["text"].as_str().unwrap());
        let mut sents = Vec::new();
        for para in khana::pyre::split(r"\n\s*\n", &text) {
            for sent in split_sentences(&para) {
                let (plan, err) = match plan_phrases(&sent, &eng.g2p, &eng.sp) {
                    Ok(p) => (p, serde_json::Value::Null),
                    Err(_) => {
                        let clean = khana::pyre::sub(r"[^\u0600-\u06FF\u200cA-Za-z0-9\s.,،؛:!?؟«»()\-]", " ", &sent);
                        let r = if khana::pyre::strip(&clean).is_empty() { Ok(Vec::new()) } else { plan_phrases(&clean, &eng.g2p, &eng.sp) };
                        match r {
                            Ok(p) => (p, "cleaned".into()),
                            Err(_) => (Vec::new(), "skipped".into()),
                        }
                    }
                };
                let chunks: Vec<Vec<String>> = plan.iter().map(|(p, _)| ch.chunk_phonemes(khana::pyre::strip(p))).collect();
                sents.push(serde_json::json!({"sent": sent, "err": err, "plan": plan, "chunks": chunks}));
            }
        }
        out.push(serde_json::json!({"doc": it["doc"], "page": it["page"], "text": it["text"], "sents": sents}));
    }
    eprintln!("{} paragraphs in {:.1} s", items.len(), t0.elapsed().as_secs_f64());
    std::fs::write(output, serde_json::to_string(&out).unwrap()).unwrap();
}

fn say(text: &str, out: &str, voice: &str) {
    let models = khana::vocab::models_dir();
    let eng = khana::tts::Engine::new(&models.join("parsigo/model")).unwrap();
    eng.g2p.set_lexicon(&[]);
    let wav = if models.join("voices").join(voice).is_file() { models.join("voices").join(voice) } else { models.join("parsigo/voices").join(voice) };
    let t0 = std::time::Instant::now();
    let a = eng.synthesize_text(&khana::tts::speech_clean(text), voice, &wav, 5, &|| false).unwrap();
    let secs = a.len() as f64 / 24000.0;
    eprintln!("{secs:.1} s of speech in {:.1} s ({:.2}x real time)", t0.elapsed().as_secs_f64(), secs / t0.elapsed().as_secs_f64());
    std::fs::write(out, khana::audio::wav_bytes(&a, 24000)).unwrap();
    let pcm = khana::audio::resample_poly(&a, 2, 3);
    eprintln!("heard: {}", khana::asr::transcribe16(&khana::audio::to_pcm16(&pcm), None));
}

fn hear(path: &str) {
    let (a, sr) = khana::audio::read_wav(std::path::Path::new(path)).unwrap();
    let pcm = if sr == 24000 { khana::audio::resample_poly(&a, 2, 3) } else { a };
    println!("{}", khana::asr::transcribe16(&khana::audio::to_pcm16(&pcm), None));
}

fn api_test(path: &str) {
    let app = khana::app::App::new(khana::engines::engines());
    let web = std::path::PathBuf::from(".").join("web");
    let tok = app.token.clone();
    let r = app.handle("GET", "/api/init", "", Some(&tok), &[], &web);
    println!("init {} {}", r.status, String::from_utf8_lossy(&r.body).chars().take(300).collect::<String>());
    let name = std::path::Path::new(path).file_name().unwrap().to_string_lossy().into_owned();
    let data = std::fs::read(path).unwrap();
    let r = app.handle("POST", "/api/open", &format!("name={name}"), Some(&tok), &data, &web);
    let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    println!("open {} {v}", r.status);
    let id = v["id"].as_str().unwrap_or("");
    let r = app.handle("GET", "/api/page", &format!("doc={id}&n=1&rev=false&yeh=true&tr=false"), Some(&tok), &[], &web);
    let p: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
    println!("page {} {}", r.status, p.to_string().chars().take(300).collect::<String>());
    let r = app.handle("GET", "/api/render", &format!("doc={id}&n=1&w=800"), Some(&tok), &[], &web);
    println!("render {} {} bytes {}", r.status, r.body.len(), r.ctype);
    let text = p["paragraphs"][0].as_str().unwrap_or("سلام");
    let body = serde_json::json!({"text": text.chars().take(120).collect::<String>(), "voice": "khabari", "speed": 1.0}).to_string();
    let t0 = std::time::Instant::now();
    let r = app.handle("POST", "/api/tts", "", Some(&tok), body.as_bytes(), &web);
    println!("tts {} {} bytes in {:.1} s", r.status, r.body.len(), t0.elapsed().as_secs_f64());
    let r = app.handle("GET", "/api/init", "", Some("wrong"), &[], &web);
    println!("bad token -> {}", r.status);
}

