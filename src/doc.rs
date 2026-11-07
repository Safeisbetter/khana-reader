
use crate::consts::*;
use crate::pyre::*;
use crate::text::*;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex};

pub const TEXTUTIL_TYPES: &[&str] = &[".doc", ".rtf", ".odt", ".wordml", ".webarchive", ".html", ".htm"];

#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub reversed: bool,
    pub persian_yeh: bool,
    pub translate: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { reversed: false, persian_yeh: true, translate: true }
    }
}

pub struct Engines {
    pub ocr: Option<Box<dyn Fn(&[u8], usize, usize) -> Vec<String> + Send + Sync>>,
    pub translate: Option<Box<dyn Fn(&[String], &str) -> Vec<String> + Send + Sync>>,
}

pub struct State {
    pub pages: Vec<Vec<String>>,
    pub scanned: HashSet<usize>,
    pub notes_after: HashMap<usize, bool>,
    pub translated: HashMap<usize, Vec<String>>,
    pub partial: HashMap<usize, Vec<String>>,
    pub title_fa: Option<String>,
}

impl State {
    fn wipe(&mut self) {
        for p in self.pages.iter_mut() {
            crate::ffi::wipe::strings(p);
        }
        self.pages.clear();
        for v in self.translated.values_mut().chain(self.partial.values_mut()) {
            crate::ffi::wipe::strings(v);
        }
        self.translated.clear();
        self.partial.clear();
        if let Some(t) = self.title_fa.as_mut() {
            crate::ffi::wipe::string(t);
        }
        self.title_fa = None;
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        self.st.get_mut().unwrap_or_else(|e| e.into_inner()).wipe();
        for (px, _, _) in self.given.get_mut().unwrap_or_else(|e| e.into_inner()).values_mut() {
            crate::ffi::wipe::vec_u8(px);
        }
        self.given.get_mut().unwrap_or_else(|e| e.into_inner()).clear();
        crate::ffi::wipe::string(&mut self.name);
        for (_, s) in self.font_fix.iter_mut() {
            crate::ffi::wipe::string(s);
        }
        self.words.wipe();
    }
}

pub struct Document {
    pub id: String,
    pub name: String,
    pub n_pages: usize,
    pub had_scans: bool,
    pub pictures: HashSet<usize>,
    lang: std::sync::OnceLock<&'static str>,
    pub font_fix: Vec<(char, String)>,
    words: DocWords,
    note_marks: Vec<Vec<u32>>,
    pub st: Mutex<State>,
    ocr_lock: Mutex<()>,
    mt_lock: Mutex<()>,
    engines: Arc<Engines>,
    pub pdfjs: bool,
    given: Mutex<HashMap<usize, (Vec<u8>, usize, usize)>>,
    pub edits: Mutex<crate::edits::Edits>,
    pub edits_file: std::path::PathBuf,
}

pub fn ext_of(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rfind('.') {
        Some(i) if i > 0 && i + 1 < base.len() => base[i..].to_lowercase(),
        _ => String::new(),
    }
}

impl Document {
    fn new(name: &str, mut pages: Vec<Vec<String>>, font_fix: Vec<(char, String)>, note_marks: Vec<Vec<u32>>, engines: Arc<Engines>, images: Option<Vec<bool>>) -> Arc<Document> {
        let ocr_ready = engines.ocr.is_some();
        let pdfjs = images.is_some();
        let mut scanned: HashSet<usize> = HashSet::new();
        let has_images = |n: usize| images.as_ref().and_then(|im| im.get(n).copied()).unwrap_or(false);
        if pdfjs {
            for n in 0..pages.len() {
                if pages[n].is_empty() && ocr_ready && has_images(n) {
                    scanned.insert(n + 1);
                }
            }
        }
        let had_scans = scanned.len() * 2 > pages.len();
        let pictures = scanned.clone();
        if pdfjs && ocr_ready {
            let mut known: Vec<(usize, f64)> = Vec::new();
            for (n, page) in pages.iter().enumerate() {
                let rep: Vec<String> = page.iter().map(|p| repair_default(p)).collect();
                if let Some(k) = known_ratio(&rep) {
                    known.push((n + 1, k));
                }
            }
            if !known.is_empty() {
                let mut vals: Vec<f64> = known.iter().map(|x| x.1).collect();
                vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
                if vals[known.len() / 2] < 0.82 {
                    scanned.extend(known.iter().map(|x| x.0));
                } else {
                    scanned.extend(known.iter().filter(|x| x.1 < 0.4).map(|x| x.0));
                }
            }
            for (n, page) in pages.iter().enumerate() {
                if latin_gibberish(page) {
                    scanned.insert(n + 1);
                }
            }
        }
        move_credits(&mut pages, &scanned);
        drop_imprint_pages(&mut pages);
        drop_addresses(&mut pages);
        let n_pages = pages.len();
        let words = DocWords::count(&pages);
        let edits_file = crate::edits::file_for(name, n_pages, &crate::edits::fingerprint(&pages));
        let doc = Arc::new(Document {
            id: new_id(),
            name: name.to_string(),
            n_pages,
            had_scans,
            pictures,
            lang: std::sync::OnceLock::new(),
            font_fix,
            words,
            note_marks,
            st: Mutex::new(State { pages, scanned, notes_after: HashMap::new(), translated: HashMap::new(), partial: HashMap::new(), title_fa: None }),
            ocr_lock: Mutex::new(()),
            mt_lock: Mutex::new(()),
            engines,
            pdfjs,
            given: Mutex::new(HashMap::new()),
            edits: Mutex::new(crate::edits::Edits::load(&edits_file)),
            edits_file,
        });
        let mut first: Vec<usize> = doc.st.lock().unwrap().scanned.iter().copied().collect();
        first.sort();
        for n in first.into_iter().take(3) {
            if doc.raw(n, true).iter().any(|p| !p.is_empty()) {
                break;
            }
        }
        let lang = {
            let st = doc.st.lock().unwrap();
            let view: Vec<Vec<String>> = st
                .pages
                .iter()
                .enumerate()
                .map(|(n, p)| if st.scanned.contains(&(n + 1)) { Vec::new() } else { p.clone() })
                .collect();
            detect_language(&view)
        };
        let _ = doc.lang.set(lang);
        doc
    }

    pub fn raw(self: &Arc<Self>, n: usize, ahead: bool) -> Vec<String> {
        let is_scanned = self.st.lock().unwrap().scanned.contains(&n);
        if is_scanned {
            {
                let _g = self.ocr_lock.lock().unwrap();
                if self.st.lock().unwrap().scanned.contains(&n) {
                    let mut paras = match &self.engines.ocr {
                        Some(ocr) if self.pdfjs => match self.given.lock().unwrap().remove(&n) {
                            Some((gray, w, h)) => ocr(&gray, w, h),
                            None => return Vec::new(),
                        },
                        _ => Vec::new(),
                    };
                    let words: usize = paras.iter().map(|p| split_ws(p).len()).sum();
                    if self.pictures.contains(&n) && !self.had_scans && words < 60
                        && (n <= 2 || n + 1 >= self.n_pages || known_ratio(&paras).unwrap_or(0.0) < 0.85)
                    {
                        paras = Vec::new();
                    }
                    let mut st = self.st.lock().unwrap();
                    st.pages[n - 1] = paras;
                    st.scanned.remove(&n);
                }
            }
            if ahead && self.st.lock().unwrap().scanned.contains(&(n + 1)) {
                let d = self.clone();
                std::thread::spawn(move || {
                    d.raw(n + 1, false);
                });
            }
        }
        self.st.lock().unwrap().pages[n - 1].clone()
    }

    pub fn title(&self) -> String {
        let base = self.name.rsplit('/').next().unwrap_or(&self.name);
        let ext = ext_of(base);
        let stem = &base[..base.len() - ext.len()];
        split_ws(&sub(r"[-_]+", " ", stem)).join(" ")
    }

    pub fn title_is_said(&self) -> bool {
        name_is_sayable(&self.title(), self.lang())
    }

    pub fn spoken_title(self: &Arc<Self>, opts: Opts) -> String {
        let t = self.title();
        if !(opts.translate && self.can_translate()) || findall(r"[A-Za-zÀ-ž]{2,}", &t).len() < 2 {
            return t;
        }
        if let Some(fa) = self.st.lock().unwrap().title_fa.clone() {
            return fa;
        }
        let out = {
            let _g = self.mt_lock.lock().unwrap();
            crate::translate::translate_title(&t, self.lang())
        };
        let fa = repair_default(&out);
        let fa = if fa.is_empty() { t } else { fa };
        self.st.lock().unwrap().title_fa = Some(fa.clone());
        fa
    }

    pub fn needs_picture(&self, n: usize) -> bool {
        self.pdfjs && self.engines.ocr.is_some() && self.st.lock().unwrap().scanned.contains(&n) && !self.given.lock().unwrap().contains_key(&n)
    }

    pub fn give_picture(&self, n: usize, gray: Vec<u8>, w: usize, h: usize) {
        self.given.lock().unwrap().insert(n, (gray, w, h));
    }

    pub fn lang(&self) -> &'static str {
        self.lang.get().copied().unwrap_or("fa")
    }

    pub fn has_text(&self) -> bool {
        let st = self.st.lock().unwrap();
        !st.scanned.is_empty() || st.pages.iter().any(|pg| pg.iter().any(|p| !strip(p).is_empty()))
    }

    pub fn can_translate(&self) -> bool {
        self.lang() != "fa" && self.engines.translate.is_some()
    }

    fn translated_page(self: &Arc<Self>, n: usize) -> Vec<String> {
        loop {
            {
                let mut st = self.st.lock().unwrap();
                if let Some(r) = st.translated.get(&n) {
                    return r.clone();
                }
                if !st.partial.contains_key(&n) {
                    st.partial.insert(n, Vec::new());
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let paras = self.raw(n, true);
        let mut done: Vec<String> = Vec::with_capacity(paras.len());
        let mut i = 0;
        while i < paras.len() {
            let mut j = i + 1;
            if i > 0 {
                let mut size = len(&paras[i]);
                while j < paras.len() && size + len(&paras[j]) <= 1500 {
                    size += len(&paras[j]);
                    j += 1;
                }
            }
            let out = {
                let _g = self.mt_lock.lock().unwrap();
                (self.engines.translate.as_ref().unwrap())(&paras[i..j], self.lang())
            };
            done.extend(out);
            self.st.lock().unwrap().partial.insert(n, done.clone());
            i = j;
        }
        {
            let mut st = self.st.lock().unwrap();
            st.partial.remove(&n);
            st.translated.insert(n, done.clone());
        }
        if n < self.n_pages && !self.st.lock().unwrap().translated.contains_key(&(n + 1)) {
            let d = self.clone();
            std::thread::spawn(move || {
                if Arc::strong_count(&d) > 1 {
                    d.translated_page(n + 1);
                }
            });
        }
        done
    }

    pub fn page_text_so_far(self: &Arc<Self>, n: usize) -> (Vec<String>, bool) {
        let (paras, done) = {
            let st = self.st.lock().unwrap();
            match (st.translated.get(&n), st.partial.get(&n)) {
                (Some(t), _) => (Some(t.clone()), true),
                (None, Some(p)) => (Some(p.clone()), false),
                _ => (None, false),
            }
        };
        let paras = paras.unwrap_or_else(|| {
            let d = self.clone();
            std::thread::spawn(move || {
                d.translated_page(n);
            });
            Vec::new()
        });
        (self.edits.lock().unwrap().apply(paras.iter().map(|p| repair_default(p)).filter(|t| !t.is_empty()).collect()), done)
    }

    pub fn page_text(self: &Arc<Self>, n: usize, opts: Opts) -> Vec<String> {
        let base = self.page_text_base(n, opts);
        self.edits.lock().unwrap().apply(base)
    }

    pub fn page_text_base(self: &Arc<Self>, n: usize, opts: Opts) -> Vec<String> {
        if opts.translate && self.can_translate() {
            let raw = self.translated_page(n);
            return raw.iter().map(|p| repair_default(p)).filter(|t| !t.is_empty()).collect();
        }
        let raw = self.raw(n, true);
        let joined = |p: &String| if opts.reversed { p.clone() } else { join_split_words(p, &self.words) };
        let mut paras: Vec<String> = raw.iter().map(|p| repair(&joined(p), opts.reversed, opts.persian_yeh)).filter(|t| !t.is_empty()).collect();
        paras = paras.iter().map(|p| drop_glosses(p)).collect();
        if n <= 15 {
            paras = tidy_contents(&paras);
        }
        let marks: Vec<u32> = self.note_marks.iter().skip(n - 1).take(2).flatten().copied().collect();
        paras = paras.iter().map(|p| drop_note_marks(p, &marks)).collect();
        if n <= 2 {
            paras = paras.iter().map(|p| drop_repeated_phrase(p)).collect();
        }
        if self.lang() == "fa" {
            paras.retain(|p| rtl(p) || !search(CITATION_LINE, p));
            paras.retain(|p| !latin_only_paragraph(p));
            paras.retain(|p| rtl(p) || !(split_ws(p).len() >= 25 || search(r"^\s*(?:Title|Abstract|Keywords|Key words)\s*:", p)));
        }
        let mut notes = std::mem::take(&mut self.st.lock().unwrap().notes_after);
        let me = self.clone();
        let mut prev = move |k: usize| -> (Vec<String>, NoteHints) {
            let r = me.raw(k, true);
            (r.iter().map(|p| repair_default(p)).collect(), note_hints(&r))
        };
        let out = skip_notes_hinted(&mut notes, n, &paras, note_hints(&raw), &mut prev);
        {
            let mut st = self.st.lock().unwrap();
            let mut merged = std::mem::take(&mut st.notes_after);
            merged.extend(notes);
            st.notes_after = merged;
        }
        out.into_iter().filter(|p| has_letters(p) && !garbled(p)).collect()
    }
}

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    format!("{:032x}{}", u128::from_le_bytes(b) ^ t, "").chars().take(32).collect()
}


fn local(n: &roxmltree::Node) -> String {
    n.tag_name().name().to_string()
}

fn t_text(n: roxmltree::Node) -> String {
    n.descendants()
        .filter(|d| d.is_element() && d.tag_name().name() == "t" && d.tag_name().namespace().is_some())
        .map(|d| d.text().unwrap_or(""))
        .collect()
}

fn zip_read(z: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

pub fn docx_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let bad = || "این فایل Word باز نشد. شاید خراب باشد.".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| bad())?;
    let mut main = "word/document.xml".to_string();
    if let Some(rels) = zip_read(&mut z, "_rels/.rels") {
        if let Ok(x) = roxmltree::Document::parse(&rels) {
            for r in x.descendants().filter(|n| n.tag_name().name() == "Relationship") {
                if r.attribute("Type").unwrap_or("").ends_with("/officeDocument") {
                    main = r.attribute("Target").unwrap_or("").trim_start_matches('/').to_string();
                }
            }
        }
    }
    let xml = zip_read(&mut z, &main).ok_or_else(bad)?;
    let opt = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let x = roxmltree::Document::parse_with_options(&xml, opt).map_err(|_| bad())?;
    let body = x.descendants().find(|n| n.is_element() && n.tag_name().name() == "body").ok_or_else(bad)?;
    let mut out = Vec::new();
    for child in body.children().filter(|n| n.is_element()) {
        let tag = local(&child);
        if tag == "p" {
            let text = t_text(child);
            if !strip(&text).is_empty() {
                out.push(strip(&text).to_string());
            }
        } else if tag == "tbl" {
            for row in child.descendants().filter(|n| n.is_element() && n.tag_name().name() == "tr") {
                let mut cells = Vec::new();
                for cell in row.children().filter(|n| n.is_element() && n.tag_name().name() == "tc") {
                    let ct = strip(&t_text(cell)).to_string();
                    if !ct.is_empty() {
                        cells.push(ct);
                    }
                }
                if !cells.is_empty() {
                    out.push(cells.join(" | "));
                }
            }
        }
    }
    Ok(out)
}

pub fn pptx_pages(data: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|_| "این فایل پاورپوینت باز نشد. فایل را در PowerPoint یا Keynote با فرمت pptx دوباره ذخیره کنید.".to_string())?;
    let mut total = 0u64;
    let mut names: HashSet<String> = HashSet::new();
    for i in 0..z.len() {
        if let Ok(f) = z.by_index_raw(i) {
            let fname = f.name().map(|s| s.to_string()).unwrap_or_default();
            if fname.ends_with(".xml") {
                total += f.size();
            }
            names.insert(fname);
        }
    }
    if total > 200 * 1024 * 1024 {
        return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
    }
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
    let mut slides: Vec<String> = Vec::new();
    (|| -> Option<()> {
        let rels_xml = zip_read(&mut z, "ppt/_rels/presentation.xml.rels")?;
        let pres_xml = zip_read(&mut z, "ppt/presentation.xml")?;
        let rels = roxmltree::Document::parse(&rels_xml).ok()?;
        let pres = roxmltree::Document::parse(&pres_xml).ok()?;
        let map: HashMap<String, String> = rels
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .map(|r| (r.attribute("Id").unwrap_or("").to_string(), r.attribute("Target").unwrap_or("").to_string()))
            .collect();
        for s in pres.descendants().filter(|n| n.is_element() && n.tag_name().name() == "sldId" && n.tag_name().namespace() == Some(P)) {
            let target = s.attribute((R, "id")).and_then(|id| map.get(id)).cloned().unwrap_or_default();
            let path = if !target.starts_with("/ppt/") {
                format!("ppt/{}", target.trim_start_matches('/').replacen("ppt/", "", 1))
            } else {
                target[1..].to_string()
            };
            if names.contains(&path) {
                slides.push(path);
            }
        }
        Some(())
    })();
    if slides.is_empty() {
        let mut s: Vec<(u64, String)> = names
            .iter()
            .filter_map(|n| fullmatch_groups(r"ppt/slides/slide(\d+)\.xml", n).and_then(|g| Some((g[1].as_ref()?.parse().ok()?, n.clone()))))
            .collect();
        s.sort();
        slides = s.into_iter().map(|x| x.1).collect();
    }
    let is = |n: &roxmltree::Node, tag: &str| n.is_element() && n.tag_name().name() == tag && n.tag_name().namespace() == Some(A);
    let ptext = |p: roxmltree::Node| -> String {
        p.descendants().filter(|t| is(t, "t")).map(|t| t.text().unwrap_or("")).collect()
    };
    let mut pages = Vec::new();
    for path in slides {
        let xml = zip_read(&mut z, &path).unwrap_or_default();
        let Ok(root) = roxmltree::Document::parse(&xml) else {
            return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
        };
        let mut rows = Vec::new();
        let mut in_table: HashSet<roxmltree::NodeId> = HashSet::new();
        for tr in root.descendants().filter(|n| is(n, "tr")) {
            let cells: Vec<String> = tr
                .children()
                .filter(|c| is(c, "tc"))
                .map(|c| strip(&c.descendants().filter(|p| is(p, "p")).map(|p| ptext(p)).collect::<Vec<_>>().join(" ")).to_string())
                .collect();
            if cells.iter().any(|c| !c.is_empty()) {
                rows.push(cells.iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>().join(" | "));
            }
            for p in tr.descendants().filter(|p| is(p, "p")) {
                in_table.insert(p.id());
            }
        }
        let paras: Vec<(String, Option<roxmltree::NodeId>, bool)> = root
            .descendants()
            .filter(|p| is(p, "p") && !in_table.contains(&p.id()))
            .map(|p| {
                let bullet = p.descendants().any(|n| is(&n, "buChar") || is(&n, "buAutoNum"));
                (strip(&ptext(p)).to_string(), p.parent().map(|b| b.id()), bullet)
            })
            .filter(|p| !p.0.is_empty())
            .collect();
        let mut body: Vec<String> = Vec::new();
        for group in paras.chunk_by(|a, b| a.1 == b.1) {
            let label = (2..=3).contains(&group.len())
                && group.iter().all(|(t, _, bullet)| !bullet && split_ws(t).len() <= 3 && !search(r"[.!?؟:؛]$", t))
                && group.iter().map(|(t, _, _)| split_ws(t).len()).sum::<usize>() <= 6;
            if label {
                body.push(group.iter().map(|(t, _, _)| t.as_str()).collect::<Vec<_>>().join(" "));
            } else {
                body.extend(group.iter().map(|(t, _, _)| t.clone()));
            }
        }
        body.extend(rows);
        pages.push(body);
    }
    Ok(pages)
}

pub fn textutil_paragraphs(data: &[u8], ext: &str) -> Result<Vec<String>, String> {
    let tmp = std::env::temp_dir().join(format!("khana-{}", new_id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let src = tmp.join(format!("input{ext}"));
    let dst = tmp.join("output.txt");
    let res = (|| {
        std::fs::write(&src, data).map_err(|e| e.to_string())?;
        let st = std::process::Command::new("/usr/bin/textutil")
            .args(["-convert", "txt", "-encoding", "UTF-8", "-output"])
            .arg(&dst)
            .arg(&src)
            .output()
            .map_err(|_| "برای خواندن این نوع فایل، ابزار textutil مک لازم است.".to_string())?;
        if !st.status.success() {
            return Err("این فایل Word باز نشد. شاید خراب باشد.".to_string());
        }
        let b = std::fs::read(&dst).map_err(|e| e.to_string())?;
        Ok(split_paragraphs(&String::from_utf8_lossy(&b)))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

pub fn open_document_layout(name: &str, pages: Vec<(Vec<LayoutItem>, f64, bool)>, engines: Arc<Engines>) -> Arc<Document> {
    let images: Vec<bool> = pages.iter().map(|p| p.2).collect();
    let mut lines: Vec<(Vec<LLine>, f64)> = pages.iter().map(|(items, h, _)| (layout_lines(items, *h), *h)).collect();
    drop_running_lines(&mut lines);
    let marks: Vec<Vec<u32>> = lines.iter_mut().map(|(l, h)| drop_footnote_lines(l, *h)).collect();
    let mut paras: Vec<Vec<String>> = lines.iter().map(|(l, _)| paragraphs_from_lines(l)).collect();
    drop_imprint_pages(&mut paras);
    drop_back_matter(&mut paras);
    let (paras, fix) = fix_font_symbols(join_across_pages(paras));
    Document::new(name, paras, fix, marks, engines, Some(images))
}

pub fn open_document(name: &str, data: &[u8], engines: Arc<Engines>) -> Result<Arc<Document>, String> {
    let ext = ext_of(name);
    if ext == ".pdf" {
        return Err("PDF در پنجره با pdf.js باز می\u{200c}شود.".into());
    }
    if ext == ".epub" {
        return Ok(Document::new(name, chunk_text_pages(crate::ebook::epub_paragraphs(data)?), Vec::new(), Vec::new(), engines, None));
    }
    if ext == ".fb2" {
        return Ok(Document::new(name, chunk_text_pages(crate::ebook::fb2_paragraphs(data)?), Vec::new(), Vec::new(), engines, None));
    }
    if ext == ".pptx" {
        return Ok(Document::new(name, pptx_pages(data)?, Vec::new(), Vec::new(), engines, None));
    }
    if ext == ".docx" {
        return Ok(Document::new(name, chunk_text_pages(docx_paragraphs(data)?), Vec::new(), Vec::new(), engines, None));
    }
    if TEXTUTIL_TYPES.contains(&ext.as_str()) {
        return Ok(Document::new(name, chunk_text_pages(textutil_paragraphs(data, &ext)?), Vec::new(), Vec::new(), engines, None));
    }
    if [".txt", ".md", ".text", ""].contains(&ext.as_str()) {
        return Ok(Document::new(name, chunk_text_pages(split_paragraphs(&decode_text(data))), Vec::new(), Vec::new(), engines, None));
    }
    Err("این نوع فایل پشتیبانی نمی\u{200c}شود. PDF، Word (doc/docx)، PowerPoint (pptx)، EPUB، FB2، RTF، ODT یا TXT باز کنید.".into())
}

pub fn open_pages(name: &str, pages: Vec<Vec<String>>, engines: Arc<Engines>) -> Arc<Document> {
    Document::new(name, pages, Vec::new(), Vec::new(), engines, None)
}

pub fn name_is_sayable(title: &str, lang: &str) -> bool {
    lang != "fa" || title.chars().any(|c| ('\u{600}'..='\u{6FF}').contains(&c))
}

