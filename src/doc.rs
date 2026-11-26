
use crate::consts::*;
use crate::pyre::*;
use crate::text::*;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex};

pub const TEXTUTIL_TYPES: &[&str] = &[".doc", ".rtf"];

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
        self.edits.get_mut().unwrap_or_else(|e| e.into_inner()).wipe();
        if let Some(previous) = self.edit_undo.get_mut().unwrap_or_else(|e| e.into_inner()) { previous.wipe(); }
    }
}

pub const PICTURE_PAGE: &str = "در این صفحه فقط تصویر هست.";

pub struct Document {
    pub id: String,
    pub name: String,
    pub is_typed: std::sync::atomic::AtomicBool,
    pub is_finglish: std::sync::atomic::AtomicBool,
    pub n_pages: usize,
    pub had_scans: bool,
    pub pictures: HashSet<usize>,
    lang: std::sync::OnceLock<&'static str>,
    figs: std::sync::OnceLock<Vec<(usize, usize, String)>>,
    pub font_fix: Vec<(char, String)>,
    words: DocWords,
    note_marks: Vec<Vec<u32>>,
    pub st: Mutex<State>,
    ocr_lock: Mutex<()>,
    mt_lock: Mutex<()>,
    mt_prefetch: std::sync::atomic::AtomicBool,
    engines: Arc<Engines>,
    pub pdfjs: bool,
    given: Mutex<HashMap<usize, (Vec<u8>, usize, usize)>>,
    pub edits: Mutex<crate::edits::Edits>,
    pub edit_undo: Mutex<Option<crate::edits::Edits>>,
    pub edits_file: std::path::PathBuf,
    para_boxes: Vec<(f64, f64, Vec<Vec<[f64; 5]>>)>,
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
            is_typed: std::sync::atomic::AtomicBool::new(false),
            is_finglish: std::sync::atomic::AtomicBool::new(false),
            n_pages,
            had_scans,
            pictures,
            lang: std::sync::OnceLock::new(),
            figs: std::sync::OnceLock::new(),
            font_fix,
            words,
            note_marks,
            st: Mutex::new(State { pages, scanned, notes_after: HashMap::new(), translated: HashMap::new(), partial: HashMap::new(), title_fa: None }),
            ocr_lock: Mutex::new(()),
            mt_lock: Mutex::new(()),
            mt_prefetch: std::sync::atomic::AtomicBool::new(false),
            engines,
            pdfjs,
            given: Mutex::new(HashMap::new()),
            edits: Mutex::new(crate::edits::Edits::load_book(&edits_file)),
            edit_undo: Mutex::new(None),
            edits_file,
            para_boxes: Vec::new(),
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

    pub fn page_boxes(&self, n: usize) -> Option<&(f64, f64, Vec<Vec<[f64; 5]>>)> {
        self.para_boxes.get(n.saturating_sub(1)).filter(|b| !b.2.is_empty())
    }

    /// Returns exact 1-to-1 paragraph boxes aligned with translated paragraphs (so if any source
    /// paragraph translated to empty and was skipped, subsequent paragraph indices never drift!).
    pub fn translated_boxes(&self, n: usize, raw_tr: &[String]) -> Option<(f64, f64, Vec<Vec<[f64; 5]>>)> {
        let (w, h, all) = self.page_boxes(n)?;
        let mut aligned = Vec::with_capacity(raw_tr.len());
        for (i, p) in raw_tr.iter().enumerate() {
            if !repair_default(p).is_empty() {
                aligned.push(all.get(i).cloned().unwrap_or_default());
            }
        }
        Some((*w, *h, aligned))
    }

    pub fn raw(self: &Arc<Self>, n: usize, ahead: bool) -> Vec<String> {
        let is_scanned = self.st.lock().unwrap().scanned.contains(&n);
        if is_scanned {
            {
                let _g = self.ocr_lock.lock().unwrap();
                if self.st.lock().unwrap().scanned.contains(&n) {
                    let picture = if self.pdfjs && self.engines.ocr.is_some() { self.given.lock().unwrap().remove(&n) } else { None };
                    let mut paras = match &self.engines.ocr {
                        Some(ocr) if self.pdfjs => match picture {
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
        let stem = if ext_of(base).is_empty() { base } else { base.rfind('.').map_or(base, |i| &base[..i]) };
        split_ws(&sub(r"[-_]+", " ", stem)).join(" ")
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
        let done = self.translate_page_only(n);
        let needed = {
            let st = self.st.lock().unwrap();
            n < self.n_pages && !st.translated.contains_key(&(n + 1)) && !st.partial.contains_key(&(n + 1))
        };
        if needed && !self.mt_prefetch.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let d = self.clone();
            std::thread::spawn(move || {
                if Arc::strong_count(&d) > 1 { d.translate_page_only(n + 1); }
                d.mt_prefetch.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        }
        done
    }

    fn translate_page_only(self: &Arc<Self>, n: usize) -> Vec<String> {
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
        struct Unmark<'a>(&'a Mutex<State>, usize, bool);
        impl Drop for Unmark<'_> {
            fn drop(&mut self) {
                if !self.2 { self.0.lock().unwrap_or_else(|e| e.into_inner()).partial.remove(&self.1); }
            }
        }
        let mut unmark = Unmark(&self.st, n, false);
        let paras = self.raw(n, true);
        if paras.is_empty() && self.pdfjs && self.st.lock().unwrap().scanned.contains(&n) {
            return Vec::new();
        }
        let batch_cap = if cfg!(shell) { 450 } else { 1500 };
        let mut done: Vec<String> = Vec::with_capacity(paras.len());
        let mut i = 0;
        while i < paras.len() {
            let mut j = i + 1;
            if i > 0 {
                let mut size = len(&paras[i]);
                while j < paras.len() && size + len(&paras[j]) <= batch_cap {
                    size += len(&paras[j]);
                    j += 1;
                }
            }
            let out = {
                let _g = self.mt_lock.lock().unwrap_or_else(|e| e.into_inner());
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
        unmark.2 = true;
        done
    }

    pub fn page_text_so_far(self: &Arc<Self>, n: usize) -> (Vec<String>, bool, usize, Option<(f64, f64, Vec<Vec<[f64; 5]>>)>) {
        let (paras, done, total) = {
            let st = self.st.lock().unwrap();
            let total = st.pages.get(n.saturating_sub(1)).map_or(0, |p| p.len());
            match (st.translated.get(&n), st.partial.get(&n)) {
                (Some(t), _) => (Some(t.clone()), true, total.max(t.len())),
                (None, Some(p)) => (Some(p.clone()), false, total),
                _ => (None, false, total),
            }
        };
        let paras = paras.unwrap_or_else(|| {
            let d = self.clone();
            std::thread::spawn(move || {
                d.translated_page(n);
            });
            Vec::new()
        });
        let boxes = self.translated_boxes(n, &paras);
        (self.edits.lock().unwrap().apply(paras.iter().map(|p| repair_default(p)).filter(|t| !t.is_empty()).collect()), done, total, boxes)
    }

    pub fn page_text(self: &Arc<Self>, n: usize, opts: Opts) -> Vec<String> {
        let base = self.page_text_base(n, opts);
        if base.is_empty() && !self.had_scans && self.pictures.contains(&n) {
            return vec![PICTURE_PAGE.to_string()];
        }
        self.edits.lock().unwrap().apply(base)
    }

    pub fn figures(self: &Arc<Self>) -> Vec<(usize, usize, String)> {
        if let Some(f) = self.figs.get() {
            return f.clone();
        }
        let o = Opts { translate: false, ..Opts::default() };
        let mut out = Vec::new();
        for n in 1..=self.n_pages {
            let base = self.page_text_base(n, o);
            if base.is_empty() && !self.had_scans && self.pictures.contains(&n) {
                out.push((n, 0, PICTURE_PAGE.to_string()));
                continue;
            }
            for (i, p) in base.iter().enumerate() {
                if crate::text::is_figure_caption(p) {
                    out.push((n, i, p.chars().take(60).collect()));
                }
            }
        }
        let _ = self.figs.set(out.clone());
        out
    }

    pub fn page_text_base(self: &Arc<Self>, n: usize, opts: Opts) -> Vec<String> {
        if self.is_typed.load(std::sync::atomic::Ordering::Relaxed) {
            let raw = self.raw(n, false);
            if self.is_finglish.load(std::sync::atomic::Ordering::Relaxed) {
                return raw.into_iter().map(|p| strip(&p).to_string()).filter(|t| !t.is_empty()).collect();
            }
            return raw.into_iter().map(|p| repair(&p, opts.reversed, opts.persian_yeh)).filter(|t| !t.is_empty()).collect();
        }
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
        paras.retain(|p| !crate::text::price_line(p));
        paras.retain(|p| !crate::text::archive_path(p));
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
    let b = crate::app::random_bytes::<16>();
    format!("{:032x}{}", u128::from_le_bytes(b) ^ t, "").chars().take(32).collect()
}


fn local(n: &roxmltree::Node) -> String {
    n.tag_name().name().to_string()
}

fn t_text(n: roxmltree::Node, remaining: &mut usize) -> Result<String, String> {
    let mut text = String::new();
    for node in n.descendants().filter(|d| d.is_element() && d.tag_name().name() == "t" && d.tag_name().namespace().is_some()) {
        let fragment = node.text().unwrap_or("");
        *remaining = remaining.checked_sub(fragment.len())
            .ok_or_else(|| "متن فایل Word خیلی بزرگ است.".to_string())?;
        text.push_str(fragment);
    }
    Ok(text)
}

const ZIP_PART_LIMIT: u64 = 32 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PPTX_SLIDES: usize = 1_000;

fn zip_read(z: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<String> {
    if name.split('/').any(|s| s == "..") {
        return None;
    }
    let f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.take(ZIP_PART_LIMIT).read_to_string(&mut s).ok()?;
    Some(s)
}

pub fn docx_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let bad = || "این فایل Word باز نشد. شاید خراب باشد.".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| bad())?;
    if z.len() > crate::ebook::MAX_ZIP_ENTRIES {
        return Err("این فایل Word خیلی بزرگ یا غیرعادی است.".into());
    }
    let mut main = "word/document.xml".to_string();
    if let Some(rels) = zip_read(&mut z, "_rels/.rels") {
        if let Ok(x) = roxmltree::Document::parse_with_options(&rels, crate::ebook::xml_options(false)) {
            for r in x.descendants().filter(|n| n.tag_name().name() == "Relationship") {
                if r.attribute("Type").unwrap_or("").ends_with("/officeDocument") {
                    main = r.attribute("Target").unwrap_or("").trim_start_matches('/').to_string();
                }
            }
        }
    }
    let xml = zip_read(&mut z, &main).ok_or_else(bad)?;
    let opt = crate::ebook::xml_options(false);
    let x = roxmltree::Document::parse_with_options(&xml, opt).map_err(|_| bad())?;
    if crate::ebook::too_deep(&x) {
        return Err(bad());
    }
    let body = x.descendants().find(|n| n.is_element() && n.tag_name().name() == "body").ok_or_else(bad)?;
    let mut out = Vec::new();
    let mut remaining = MAX_TEXT_BYTES;
    for child in body.children().filter(|n| n.is_element()) {
        let tag = local(&child);
        if tag == "p" {
            let text = t_text(child, &mut remaining)?;
            if !strip(&text).is_empty() {
                out.push(strip(&text).to_string());
            }
        } else if tag == "tbl" {
            for row in child.descendants().filter(|n| n.is_element() && n.tag_name().name() == "tr") {
                let mut cells = Vec::new();
                for cell in row.children().filter(|n| n.is_element() && n.tag_name().name() == "tc") {
                    let ct = strip(&t_text(cell, &mut remaining)?).to_string();
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
    if z.len() > crate::ebook::MAX_ZIP_ENTRIES {
        return Err("این فایل پاورپوینت خیلی بزرگ یا غیرعادی است.".into());
    }
    let mut total = 0u64;
    let mut names: HashSet<String> = HashSet::new();
    for i in 0..z.len() {
        if let Ok(f) = z.by_index_raw(i) {
            let fname = f.name().map(|s| s.to_string()).unwrap_or_default();
            if fname.ends_with(".xml") {
                total = total.saturating_add(f.size());
            }
            names.insert(fname);
        }
    }
    if total > 80 * 1024 * 1024 {
        return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
    }
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
    let mut slides: Vec<String> = Vec::new();
    let mut referenced = HashSet::new();
    (|| -> Option<()> {
        let rels_xml = zip_read(&mut z, "ppt/_rels/presentation.xml.rels")?;
        let pres_xml = zip_read(&mut z, "ppt/presentation.xml")?;
        let rels = roxmltree::Document::parse_with_options(&rels_xml, crate::ebook::xml_options(false)).ok()?;
        let pres = roxmltree::Document::parse_with_options(&pres_xml, crate::ebook::xml_options(false)).ok()?;
        let map: HashMap<String, String> = rels
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .map(|r| (r.attribute("Id").unwrap_or("").to_string(), r.attribute("Target").unwrap_or("").to_string()))
            .collect();
        for s in pres.descendants().filter(|n| n.is_element() && n.tag_name().name() == "sldId" && n.tag_name().namespace() == Some(P)) {
            let Some(target) = s.attribute((R, "id")).and_then(|id| map.get(id)) else { continue };
            if target.len() > 4096 { continue; }
            let path = if !target.starts_with("/ppt/") {
                format!("ppt/{}", target.trim_start_matches('/').replacen("ppt/", "", 1))
            } else {
                target[1..].to_string()
            };
            if names.contains(&path) && referenced.insert(path.clone()) {
                slides.push(path);
                if slides.len() > MAX_PPTX_SLIDES { break; }
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
    if slides.len() > MAX_PPTX_SLIDES {
        return Err("این فایل پاورپوینت خیلی بزرگ است (بیش از ۱۰۰۰ اسلاید).".into());
    }
    let is = |n: &roxmltree::Node, tag: &str| n.is_element() && n.tag_name().name() == tag && n.tag_name().namespace() == Some(A);
    let ptext = |p: roxmltree::Node| -> String {
        p.descendants().filter(|t| is(t, "t")).map(|t| t.text().unwrap_or("")).collect()
    };
    let mut pages = Vec::new();
    let mut read_total = 0u64;
    for path in slides {
        let xml = zip_read(&mut z, &path).unwrap_or_default();
        read_total += xml.len() as u64;
        if read_total > 80 * 1024 * 1024 {
            return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
        }
        let Ok(root) = roxmltree::Document::parse_with_options(&xml, crate::ebook::xml_options(false)) else {
            return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
        };
        if crate::ebook::too_deep(&root) {
            return Err("این فایل پاورپوینت باز نشد. شاید خراب باشد.".into());
        }
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

fn decode_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(semi) = b[i + 1..b.len().min(i + 16)].iter().position(|&c| c == b';') {
                let ent = &s[i + 1..i + 1 + semi];
                let rep: Option<char> = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some(' '),
                    "zwnj" => Some('\u{200c}'),
                    "zwj" => Some('\u{200d}'),
                    "laquo" => Some('«'),
                    "raquo" => Some('»'),
                    "ndash" => Some('–'),
                    "mdash" => Some('—'),
                    "hellip" => Some('…'),
                    "lsquo" => Some('‘'),
                    "rsquo" => Some('’'),
                    "ldquo" => Some('“'),
                    "rdquo" => Some('”'),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
                    }
                    _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                if let Some(ch) = rep {
                    out.push(ch);
                    i += semi + 2;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Pure-Rust offline HTML text extractor: never invokes WebKit, textutil, or any network/file loader.
pub fn html_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    if data.len() > MAX_TEXT_BYTES {
        return Err("این فایل HTML خیلی بزرگ است (بیشتر از ۶۴ مگابایت).".into());
    }
    let raw = decode_text(data);
    let mut out = String::with_capacity(raw.len().min(4 * 1024 * 1024));
    let bytes = raw.as_bytes();
    let mut i = 0;
    let mut skip_end: Option<&[u8]> = None;
    while i < bytes.len() {
        if let Some(end_tag) = skip_end {
            if bytes[i] == b'<' && i + end_tag.len() <= bytes.len()
                && bytes[i..i + end_tag.len()].eq_ignore_ascii_case(end_tag)
            {
                i += end_tag.len();
                while i < bytes.len() && bytes[i] != b'>' {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
                skip_end = None;
                continue;
            }
            i += 1;
            continue;
        }
        if bytes[i] == b'<' {
            if bytes[i..].starts_with(b"<!--") {
                i += 4;
                while i + 2 < bytes.len() && &bytes[i..i + 3] != b"-->" {
                    i += 1;
                }
                i = (i + 3).min(bytes.len());
                continue;
            }
            let start = i + 1;
            let mut j = start;
            let mut quote: u8 = 0;
            while j < bytes.len() {
                let c = bytes[j];
                if quote != 0 {
                    if c == quote {
                        quote = 0;
                    }
                } else if c == b'"' || c == b'\'' {
                    quote = c;
                } else if c == b'>' {
                    break;
                }
                j += 1;
            }
            let tag_raw = raw[start..j].trim_start();
            let closing = tag_raw.starts_with('/');
            let name_part = tag_raw.trim_start_matches('/');
            let name_end = name_part
                .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
                .unwrap_or(name_part.len());
            let tag = name_part[..name_end].to_ascii_lowercase();
            if !closing {
                match tag.as_str() {
                    "script" => skip_end = Some(b"</script"),
                    "style" => skip_end = Some(b"</style"),
                    "head" => skip_end = Some(b"</head"),
                    "noscript" => skip_end = Some(b"</noscript"),
                    "svg" => skip_end = Some(b"</svg"),
                    "br" | "hr" => out.push_str("\n\n"),
                    _ => {}
                }
            }
            if matches!(
                tag.as_str(),
                "p" | "div" | "li" | "tr" | "blockquote" | "pre" | "section" | "article" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            ) {
                out.push_str("\n\n");
            }
            i = if j < bytes.len() { j + 1 } else { bytes.len() };
            continue;
        }
        let next_lt = bytes[i..].iter().position(|&c| c == b'<').map_or(bytes.len(), |off| i + off);
        out.push_str(&raw[i..next_lt]);
        i = next_lt;
    }
    let decoded = decode_html_entities(&out);
    let paras = split_paragraphs(&decoded);
    if paras.is_empty() {
        return Err("در این فایل HTML متنی پیدا نشد.".into());
    }
    Ok(paras)
}

fn odt_text(
    node: roxmltree::Node<'_, '_>,
    text: &mut String,
    remaining: &mut usize,
) -> Result<(), String> {
    // The XML depth check bounds recursion. Nested blocks are extracted in their own pass.
    for child in node.children() {
        let fragment = match child.tag_name().name() {
            "p" | "h" => continue,
            "s" | "tab" | "line-break" => " ",
            _ if child.is_text() => child.text().unwrap_or(""),
            _ => {
                odt_text(child, text, remaining)?;
                continue;
            }
        };
        *remaining = remaining.checked_sub(fragment.len())
            .ok_or_else(|| "این فایل ODT خیلی بزرگ است.".to_string())?;
        text.push_str(fragment);
        if child.is_element() {
            odt_text(child, text, remaining)?;
        }
    }
    Ok(())
}

/// Pure-Rust offline ODT extractor: parses `content.xml` without invoking external tools.
pub fn odt_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let bad = || "این فایل ODT باز نشد. شاید خراب باشد.".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| bad())?;
    if z.len() > crate::ebook::MAX_ZIP_ENTRIES {
        return Err("این فایل ODT خیلی بزرگ یا غیرعادی است.".into());
    }
    let xml = zip_read(&mut z, "content.xml").ok_or_else(bad)?;
    let x = roxmltree::Document::parse_with_options(&xml, crate::ebook::xml_options(false)).map_err(|_| bad())?;
    if crate::ebook::too_deep(&x) {
        return Err(bad());
    }
    let mut out = Vec::new();
    let mut remaining = MAX_TEXT_BYTES;
    for node in x.descendants().filter(|n| n.is_element() && matches!(n.tag_name().name(), "p" | "h")) {
        let mut t = String::new();
        odt_text(node, &mut t, &mut remaining)?;
        let mut normalized = String::new();
        for word in t.split(is_space).filter(|word| !word.is_empty()) {
            if !normalized.is_empty() {
                normalized.push(' ');
            }
            normalized.push_str(word);
        }
        if !normalized.is_empty() {
            out.push(normalized);
            if out.len() >= crate::ebook::MAX_PARAGRAPHS {
                break;
            }
        }
    }
    if out.is_empty() {
        return Err("در این فایل ODT متنی پیدا نشد.".into());
    }
    Ok(out)
}

#[cfg(test)]
mod odt_tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    #[test]
    fn odt_preserves_inline_text_and_extracts_nested_paragraph_once() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("content.xml", SimpleFileOptions::default()).unwrap();
        zip.write_all(br#"<body><h>Title</h><p>Hello<span> world</span><tab/>again<frame><p>Caption</p></frame></p></body>"#)
            .unwrap();
        let data = zip.finish().unwrap().into_inner();
        assert_eq!(odt_paragraphs(&data).unwrap(), ["Title", "Hello world again", "Caption"]);
    }

    #[test]
    fn odt_checks_remaining_bytes_before_appending() {
        let xml = roxmltree::Document::parse("<p>hello</p>").unwrap();
        let mut text = String::new();
        let mut remaining = 4;
        assert!(odt_text(xml.root_element(), &mut text, &mut remaining).is_err());
        assert!(text.is_empty());
        assert_eq!(remaining, 4);
    }
}

pub fn textutil_paragraphs(data: &[u8], ext: &str) -> Result<Vec<String>, String> {
    // Strictly verify binary/RTF magic bytes and pass an explicit `-format` flag so macOS
    // `textutil` can never sniff disguised HTML/WebArchive content or invoke WebKit importers.
    let fmt = match ext {
        ".doc" => {
            const OLE2_MAGIC: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
            if !data.starts_with(OLE2_MAGIC) {
                return Err("این فایل Word معتبر نیست (قالب باینری doc ندارد).".to_string());
            }
            "doc"
        }
        ".rtf" => {
            let trimmed = data.iter().position(|b| !b.is_ascii_whitespace()).map_or(&[][..], |i| &data[i..]);
            if !trimmed.starts_with(b"{\\rtf") {
                return Err("این فایل RTF معتبر نیست.".to_string());
            }
            "rtf"
        }
        _ => return Err("این نوع فایل پشتیبانی نمی‌شود.".to_string()),
    };
    let tmp = crate::private_fs::TempDir::new().map_err(|e| e.to_string())?;
    let src = tmp.join(format!("input{ext}"));
    let dst = tmp.join("output.txt");
    let res = (|| {
        std::fs::write(&src, data).map_err(|e| e.to_string())?;
        let st = std::process::Command::new("/usr/bin/textutil")
            .args(["-format", fmt, "-convert", "txt", "-encoding", "UTF-8", "-output"])
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
    res
}

pub fn open_document_layout(name: &str, mut pages: Vec<(Vec<LayoutItem>, f64, f64, bool)>, engines: Arc<Engines>) -> Arc<Document> {
    let mut items: Vec<Vec<LayoutItem>> = pages.iter_mut().map(|p| std::mem::take(&mut p.0)).collect();
    crate::text::mend_reversed_numbers(&mut items);
    for page in items.iter_mut() {
        crate::text::drop_vertical_text(page);
    }
    for (p, it) in pages.iter_mut().zip(items) {
        p.0 = it;
    }
    let dims: Vec<(f64, f64)> = pages.iter().map(|p| (p.1, p.2)).collect();
    let images: Vec<bool> = pages.iter().map(|p| p.3).collect();
    let mut lines: Vec<(Vec<LLine>, f64)> = pages.iter().map(|(items, _w, h, _)| (layout_lines(items, *h), *h)).collect();
    drop_running_lines(&mut lines);
    let marks: Vec<Vec<u32>> = lines.iter_mut().map(|(l, h)| drop_footnote_lines(l, *h)).collect();
    let mut paras: Vec<Vec<String>> = Vec::with_capacity(lines.len());
    let mut boxes: Vec<Vec<Vec<[f64; 5]>>> = Vec::with_capacity(lines.len());
    for (l, _) in &lines {
        let (p, b) = paragraphs_with_boxes(l);
        paras.push(p);
        boxes.push(b);
    }
    drop_imprint_pages(&mut paras);
    drop_back_matter(&mut paras);
    for (p, b) in paras.iter().zip(boxes.iter_mut()) {
        if p.is_empty() {
            b.clear();
        }
    }
    let (paras, boxes) = join_across_pages_with_boxes(paras, boxes);
    let (paras, fix) = fix_font_symbols(paras);
    let para_boxes: Vec<(f64, f64, Vec<Vec<[f64; 5]>>)> = dims.into_iter().zip(boxes).map(|((w, h), b)| (w, h, b)).collect();
    let mut doc = Document::new(name, paras, fix, marks, engines, Some(images));
    if let Some(d) = Arc::get_mut(&mut doc) {
        d.para_boxes = para_boxes;
    }
    doc
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
    if ext == ".odt" {
        return Ok(Document::new(name, chunk_text_pages(odt_paragraphs(data)?), Vec::new(), Vec::new(), engines, None));
    }
    if ext == ".html" || ext == ".htm" {
        return Ok(Document::new(name, chunk_text_pages(html_paragraphs(data)?), Vec::new(), Vec::new(), engines, None));
    }
    if TEXTUTIL_TYPES.contains(&ext.as_str()) {
        return Ok(Document::new(name, chunk_text_pages(textutil_paragraphs(data, &ext)?), Vec::new(), Vec::new(), engines, None));
    }
    if [".txt", ".md", ".text", ""].contains(&ext.as_str()) {
        if data.len() > MAX_TEXT_BYTES {
            return Err("این فایل متنی خیلی بزرگ است (بیشتر از ۶۴ مگابایت).".into());
        }
        return Ok(Document::new(name, chunk_text_pages(split_paragraphs(&decode_text(data))), Vec::new(), Vec::new(), engines, None));
    }
    Err("این نوع فایل پشتیبانی نمی\u{200c}شود. PDF، Word (doc/docx)، PowerPoint (pptx)، EPUB، FB2، RTF، ODT یا TXT باز کنید.".into())
}

/// Opens user-typed text from "نوشتن متن" verbatim (preserving every line and Finglish text
/// without book-cleanup filters like drop_imprint_pages, drop_addresses, or latin_only_paragraph).
pub fn open_typed(name: &str, text: &str, finglish: bool, engines: Arc<Engines>) -> Arc<Document> {
    let paras: Vec<String> = text
        .lines()
        .map(|l| strip(l).to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let pages = chunk_text_pages(paras);
    let n_pages = pages.len();
    let words = DocWords::count(&pages);
    let edits_file = crate::edits::file_for(name, n_pages, &crate::edits::fingerprint(&pages));
    let lang = if finglish { "fa" } else { detect_language(&pages) };
    let doc = Arc::new(Document {
        id: new_id(),
        name: name.to_string(),
        is_typed: std::sync::atomic::AtomicBool::new(true),
        is_finglish: std::sync::atomic::AtomicBool::new(finglish),
        n_pages,
        had_scans: false,
        pictures: HashSet::new(),
        lang: std::sync::OnceLock::new(),
        figs: std::sync::OnceLock::new(),
        font_fix: Vec::new(),
        words,
        note_marks: Vec::new(),
        st: Mutex::new(State { pages, scanned: HashSet::new(), notes_after: HashMap::new(), translated: HashMap::new(), partial: HashMap::new(), title_fa: None }),
        ocr_lock: Mutex::new(()),
        mt_lock: Mutex::new(()),
        mt_prefetch: std::sync::atomic::AtomicBool::new(false),
        engines,
        pdfjs: false,
        given: Mutex::new(HashMap::new()),
        edits: Mutex::new(crate::edits::Edits::default()),
        edit_undo: Mutex::new(None),
        edits_file,
        para_boxes: Vec::new(),
    });
    let _ = doc.lang.set(lang);
    doc
}

pub fn open_pages(name: &str, pages: Vec<Vec<String>>, engines: Arc<Engines>) -> Arc<Document> {
    Document::new(name, pages, Vec::new(), Vec::new(), engines, None)
}




