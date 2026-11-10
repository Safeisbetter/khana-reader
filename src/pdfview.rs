
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const MAX_ANSWER: usize = 256 * 1024 * 1024;
const POLL: Duration = Duration::from_secs(20);
const PATIENCE: Duration = Duration::from_secs(600);

struct Answer {
    kind: &'static str,
    again: bool,
    body: Vec<u8>,
}

#[derive(Default)]
struct Inner {
    jobs: VecDeque<String>,
    bytes: Option<Vec<u8>>,
    waiters: HashMap<u64, Sender<Answer>>,
    next_id: u64,
    session: String,
}

#[derive(Default)]
pub struct Bridge {
    inner: Mutex<Inner>,
    wake: Condvar,
}

pub type Out = (u16, &'static str, Vec<(String, String)>, Vec<u8>);

fn plain(status: u16, text: &str) -> Out {
    (status, "text/plain; charset=utf-8", Vec::new(), text.as_bytes().to_vec())
}

fn number(q: &HashMap<String, String>, key: &str, min: u64, max: u64) -> Option<u64> {
    q.get(key)?.parse::<u64>().ok().filter(|v| (min..=max).contains(v))
}

impl Bridge {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn call(&self, q: &HashMap<String, String>, body: &[u8]) -> Out {
        let op = q.get("op").map(String::as_str).unwrap_or("");
        let (tx, rx) = channel();
        let id;
        {
            let mut g = self.lock();
            let job = match op {
                "open" => {
                    if let Some(mut old) = g.bytes.replace(body.to_vec()) {
                        crate::ffi::wipe::vec_u8(&mut old);
                    }
                    g.next_id += 1;
                    serde_json::json!({"op": "open", "id": g.next_id})
                }
                "text" | "render" | "gray" => {
                    let Some(n) = number(q, "n", 1, 1_000_000) else { return plain(400, "bad page") };
                    g.next_id += 1;
                    match op {
                        "text" => serde_json::json!({"op": "text", "id": g.next_id, "n": n}),
                        "render" => {
                            let Some(w) = number(q, "width", 1, 20_000) else { return plain(400, "bad width") };
                            serde_json::json!({"op": "render", "id": g.next_id, "n": n, "width": w})
                        }
                        _ => {
                            let Some(dpi) = number(q, "dpi", 36, 600) else { return plain(400, "bad dpi") };
                            serde_json::json!({"op": "gray", "id": g.next_id, "n": n, "dpi": dpi})
                        }
                    }
                }
                "password" => {
                    let Some(open_id) = number(q, "id", 1, u64::MAX) else { return plain(400, "bad id") };
                    if open_id > g.next_id {
                        return plain(400, "bad id");
                    }
                    g.waiters.insert(open_id, tx.clone());
                    let job = if q.get("cancel").is_some_and(|v| v == "1") {
                        serde_json::json!({"op": "password", "cancel": true})
                    } else {
                        serde_json::json!({"op": "password", "value": String::from_utf8_lossy(body)})
                    };
                    g.jobs.push_back(job.to_string());
                    self.wake.notify_all();
                    drop(g);
                    return self.wait(open_id, rx);
                }
                _ => return plain(400, "unknown request"),
            };
            id = g.next_id;
            g.waiters.insert(id, tx);
            g.jobs.push_back(job.to_string());
            self.wake.notify_all();
        }
        self.wait(id, rx)
    }

    fn wait(&self, id: u64, rx: std::sync::mpsc::Receiver<Answer>) -> Out {
        match rx.recv_timeout(PATIENCE) {
            Ok(a) => {
                let mut headers = vec![("X-Khana-Pdf".to_string(), a.kind.to_string()), ("X-Khana-Pdf-Id".to_string(), id.to_string())];
                if a.again {
                    headers.push(("X-Khana-Pdf-Again".to_string(), "1".to_string()));
                }
                let ctype = match a.kind {
                    "json" => "application/json; charset=utf-8",
                    "png" => "image/png",
                    "gray" => "application/octet-stream",
                    _ => "text/plain; charset=utf-8",
                };
                (200, ctype, headers, a.body)
            }
            Err(_) => {
                self.lock().waiters.remove(&id);
                (200, "text/plain; charset=utf-8", vec![("X-Khana-Pdf".to_string(), "error".to_string())], b"no answer".to_vec())
            }
        }
    }

    pub fn view(&self, method: &str, path: &str, q: &HashMap<String, String>, body: &[u8]) -> Out {
        match (method, path) {
            ("GET", "/view/next") => {
                let s = q.get("s").cloned().unwrap_or_default();
                if s.is_empty() || s.len() > 64 {
                    return plain(400, "");
                }
                let until = Instant::now() + POLL;
                let mut g = self.lock();
                if g.session != s {
                    g.session = s.clone();
                    self.wake.notify_all();
                }
                loop {
                    if g.session != s {
                        return (200, "application/json", Vec::new(), b"{}".to_vec());
                    }
                    if let Some(job) = g.jobs.pop_front() {
                        return (200, "application/json", Vec::new(), job.into_bytes());
                    }
                    let left = until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return (200, "application/json", Vec::new(), b"{}".to_vec());
                    }
                    g = self.wake.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
                }
            }
            ("GET", "/view/bytes") => match self.lock().bytes.take() {
                Some(b) => (200, "application/octet-stream", Vec::new(), b),
                None => plain(404, ""),
            },
            ("POST", "/view/reply") => {
                let kind = match q.get("kind").map(String::as_str) {
                    Some("json") => "json",
                    Some("png") => "png",
                    Some("gray") => "gray",
                    Some("password") => "password",
                    Some("error") => "error",
                    _ => return plain(400, ""),
                };
                let Some(id) = number(q, "id", 1, u64::MAX) else { return plain(400, "") };
                if body.len() > MAX_ANSWER {
                    return plain(413, "");
                }
                if let Some(tx) = self.lock().waiters.remove(&id) {
                    let body = if kind == "error" { body[..body.len().min(400)].to_vec() } else { body.to_vec() };
                    let _ = tx.send(Answer { kind, again: q.get("again").is_some_and(|v| v == "1"), body });
                }
                plain(200, "")
            }
            _ => plain(404, ""),
        }
    }

    pub fn reset(&self) {
        let mut g = self.lock();
        for (_, tx) in g.waiters.drain() {
            let _ = tx.send(Answer { kind: "error", again: false, body: b"pdf.js stopped".to_vec() });
        }
        for mut j in g.jobs.drain(..) {
            crate::ffi::wipe::string(&mut j);
        }
        if let Some(mut b) = g.bytes.take() {
            crate::ffi::wipe::vec_u8(&mut b);
        }
    }
}

