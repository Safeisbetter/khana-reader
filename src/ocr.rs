
use crate::consts::*;
use crate::difflib::{Op, SequenceMatcher};
use crate::onnx;
use crate::phon::pronounce_word;
use crate::pyre::*;
use crate::text::rtl;
use crate::vocab::{farsi_words, models_dir, nllb_sp};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct Line {
    pub t: String,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bot: f64,
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn vision_lines(gray: &[u8], w: usize, h: usize) -> Vec<Line> {
    let Some(s) = crate::ffi::vision::recognize(gray, w, h) else { return Vec::new() };
    let mut out = Vec::new();
    for l in s.lines() {
        let f: Vec<&str> = l.splitn(5, '\t').collect();
        if f.len() < 5 {
            continue;
        }
        let n = |i: usize| f[i].parse::<f64>().unwrap_or(0.0);
        let (x, y, bw, bh) = (n(0), n(1), n(2), n(3));
        let text = strip(f[4]).to_string();
        if !text.is_empty() && !fullmatch(r"[\d۰-۹٠-٩\s\-–.]+", &text) {
            out.push(Line { t: text, x0: x, x1: x + bw, top: 1.0 - y - bh, bot: 1.0 - y });
        }
    }
    out
}


pub fn find_lines(gray: &[u8], w: usize, h: usize) -> Vec<Line> {
    if w < 16 || h < 16 || gray.len() != w * h {
        return Vec::new();
    }
    let thr = otsu(gray);
    if thr == 0 {
        return Vec::new();
    }
    let (sw, sh) = (w / 2, h / 2);
    let mut ink = vec![false; sw * sh];
    for y in 0..sh {
        for x in 0..sw {
            let at = |dy: usize, dx: usize| gray[(2 * y + dy) * w + 2 * x + dx] < thr;
            ink[y * sw + x] = at(0, 0) || at(0, 1) || at(1, 0) || at(1, 1);
        }
    }
    let page = Ink { ink: &ink, w: sw };
    let total = ink.iter().filter(|v| **v).count();
    if total * 3 > sw * sh {
        return Vec::new();
    }
    let lh = {
        let mut hs: Vec<f64> = runs(&page.rows(0, sw, 0, sh), |c| c > 0).into_iter().map(|(a, b)| (b - a) as f64).filter(|v| *v >= 3.0).collect();
        if hs.is_empty() {
            return Vec::new();
        }
        hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        hs[hs.len() * 3 / 5].max(4.0)
    };
    let gutter = (sw as f64 * 0.02).max(0.8 * lh).ceil() as usize;
    let mut boxes = Vec::new();
    page.cut(Rect { x0: 0, x1: sw, y0: 0, y1: sh }, gutter, &mut boxes, 0);
    let boxes = mend_lines(&page, boxes, lh);
    let (fw, fh) = (sw as f64, sh as f64);
    boxes
        .into_iter()
        .filter(|b| (b.y1 - b.y0) as f64 >= 0.35 * lh && (b.x1 - b.x0) as f64 >= 0.5 * lh && page.count(*b) >= 12)
        .map(|b| Line { t: String::new(), x0: b.x0 as f64 / fw, x1: b.x1 as f64 / fw, top: b.y0 as f64 / fh, bot: b.y1 as f64 / fh })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
}

struct Ink<'a> {
    ink: &'a [bool],
    w: usize,
}

impl Ink<'_> {
    fn rows(&self, x0: usize, x1: usize, y0: usize, y1: usize) -> Vec<usize> {
        (y0..y1).map(|y| self.ink[y * self.w + x0..y * self.w + x1].iter().filter(|v| **v).count()).collect()
    }

    fn cols(&self, x0: usize, x1: usize, y0: usize, y1: usize) -> Vec<usize> {
        let mut c = vec![0usize; x1 - x0];
        for y in y0..y1 {
            for (i, v) in self.ink[y * self.w + x0..y * self.w + x1].iter().enumerate() {
                c[i] += *v as usize;
            }
        }
        c
    }

    fn count(&self, r: Rect) -> usize {
        self.rows(r.x0, r.x1, r.y0, r.y1).iter().sum()
    }

    fn trim(&self, r: Rect) -> Option<Rect> {
        let rows = self.rows(r.x0, r.x1, r.y0, r.y1);
        let y0 = r.y0 + rows.iter().position(|c| *c > 0)?;
        let y1 = r.y0 + rows.iter().rposition(|c| *c > 0)? + 1;
        let cols = self.cols(r.x0, r.x1, y0, y1);
        let x0 = r.x0 + cols.iter().position(|c| *c > 0)?;
        let x1 = r.x0 + cols.iter().rposition(|c| *c > 0)? + 1;
        Some(Rect { x0, x1, y0, y1 })
    }

    fn cut(&self, r: Rect, gutter: usize, out: &mut Vec<Rect>, depth: usize) {
        let Some(r) = self.trim(r) else { return };
        if depth > 40 {
            out.push(r);
            return;
        }
        let cols = self.cols(r.x0, r.x1, r.y0, r.y1);
        let gaps: Vec<(usize, usize)> = runs(&cols, |c| c == 0).into_iter().filter(|(a, b)| b - a >= gutter).collect();
        if !gaps.is_empty() {
            let mut x = r.x0;
            for (a, b) in gaps {
                self.cut(Rect { x0: x, x1: r.x0 + a, ..r }, gutter, out, depth + 1);
                x = r.x0 + b;
            }
            self.cut(Rect { x0: x, ..r }, gutter, out, depth + 1);
            return;
        }
        let rows = self.rows(r.x0, r.x1, r.y0, r.y1);
        let strips = runs(&rows, |c| c > 0);
        if strips.len() > 1 {
            for (a, b) in strips {
                self.cut(Rect { y0: r.y0 + a, y1: r.y0 + b, ..r }, gutter, out, depth + 1);
            }
            return;
        }
        out.push(r);
    }
}

fn runs(v: &[usize], pred: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in v.iter().enumerate() {
        match (pred(*c), start) {
            (true, None) => start = Some(i),
            (false, Some(a)) => {
                out.push((a, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        out.push((a, v.len()));
    }
    out
}

fn mend_lines(page: &Ink, mut boxes: Vec<Rect>, lh: f64) -> Vec<Rect> {
    let overlap = |a: &Rect, b: &Rect| (a.x1.min(b.x1) as isize - a.x0.max(b.x0) as isize).max(0) as f64;
    let vgap = |a: &Rect, b: &Rect| if a.y1 <= b.y0 { (b.y0 - a.y1) as f64 } else if b.y1 <= a.y0 { (a.y0 - b.y1) as f64 } else { 0.0 };
    loop {
        let mut best: Option<(usize, usize, f64)> = None;
        for (i, t) in boxes.iter().enumerate() {
            if (t.y1 - t.y0) as f64 >= 0.5 * lh {
                continue;
            }
            for (j, l) in boxes.iter().enumerate() {
                if i == j || (l.y1 - l.y0) as f64 <= (t.y1 - t.y0) as f64 {
                    continue;
                }
                let d = vgap(t, l);
                if d <= 0.6 * lh && overlap(t, l) >= 0.5 * (t.x1 - t.x0) as f64 && best.map_or(true, |b| d < b.2) {
                    best = Some((i, j, d));
                }
            }
        }
        let Some((i, j, _)) = best else { break };
        let (t, l) = (boxes[i], boxes[j]);
        boxes[j] = Rect { x0: t.x0.min(l.x0), x1: t.x1.max(l.x1), y0: t.y0.min(l.y0), y1: t.y1.max(l.y1) };
        boxes.remove(i);
    }
    let mut out = Vec::with_capacity(boxes.len());
    for b in boxes {
        let hgt = (b.y1 - b.y0) as f64;
        let k = (hgt / lh).round() as usize;
        if hgt < 1.7 * lh || k < 2 {
            out.push(b);
            continue;
        }
        let rows = page.rows(b.x0, b.x1, b.y0, b.y1);
        let step = hgt / k as f64;
        let mut cuts = vec![0usize];
        for i in 1..k {
            let mid = (i as f64 * step) as usize;
            let win = (0.3 * step) as usize;
            let (lo, hi) = (mid.saturating_sub(win).max(cuts[cuts.len() - 1] + 1), (mid + win).min(rows.len() - 1));
            if lo >= hi {
                continue;
            }
            let at = (lo..=hi).min_by_key(|y| rows[*y]).unwrap_or(mid);
            cuts.push(at);
        }
        cuts.push(rows.len());
        for p in cuts.windows(2) {
            if let Some(r) = page.trim(Rect { y0: b.y0 + p[0], y1: b.y0 + p[1], ..b }) {
                out.push(r);
            }
        }
    }
    out
}

fn otsu(gray: &[u8]) -> u8 {
    let mut hist = [0u64; 256];
    for v in gray {
        hist[*v as usize] += 1;
    }
    let n = gray.len() as f64;
    let sum: f64 = hist.iter().enumerate().map(|(i, c)| i as f64 * *c as f64).sum();
    let (mut wb, mut sb, mut best, mut thr) = (0f64, 0f64, 0f64, 0u8);
    for t in 0..256 {
        wb += hist[t] as f64;
        if wb == 0.0 {
            continue;
        }
        let wf = n - wb;
        if wf == 0.0 {
            break;
        }
        sb += t as f64 * hist[t] as f64;
        let (mb, mf) = (sb / wb, (sum - sb) / wf);
        let between = wb * wf * (mb - mf) * (mb - mf);
        if between > best {
            best = between;
            thr = (t + 1).min(255) as u8;
        }
    }
    thr
}


struct Rec {
    sess: ort::Session,
    chars: Vec<String>,
}

fn rec() -> Option<&'static Rec> {
    static R: OnceLock<Option<Rec>> = OnceLock::new();
    R.get_or_init(|| {
        let dir = models_dir().join("ocr/arabic_PP-OCRv5_mobile_rec");
        let yml = std::fs::read_to_string(dir.join("inference.yml")).ok()?;
        let mut chars = vec!["<blank>".to_string()];
        chars.extend(character_dict(&yml));
        chars.push(" ".into());
        let sess = onnx::session(&dir.join("inference.onnx"), Some(2)).ok()?;
        Some(Rec { sess, chars })
    })
    .as_ref()
}

pub fn character_dict(yml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in yml.lines() {
        if line.trim_end() == "  character_dict:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        let Some(item) = line.strip_prefix("  - ") else { break };
        let v = if item.len() >= 2 && item.starts_with('\'') && item.ends_with('\'') {
            item[1..item.len() - 1].replace("''", "'")
        } else if item.len() >= 2 && item.starts_with('"') && item.ends_with('"') {
            item[1..item.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\")
        } else {
            item.to_string()
        };
        out.push(v);
    }
    out
}

fn zoom_bilinear(img: &[f32], h: usize, w: usize, oh: usize, ow: usize) -> Vec<f32> {
    let fy = if oh > 1 { (h as f64 - 1.0) / (oh as f64 - 1.0) } else { 0.0 };
    let fx = if ow > 1 { (w as f64 - 1.0) / (ow as f64 - 1.0) } else { 0.0 };
    let mut out = vec![0f32; oh * ow];
    for oy in 0..oh {
        let y = oy as f64 * fy;
        if y > (h - 1) as f64 {
            continue;
        }
        let y0 = (y.floor() as usize).min(h - 1);
        let y1 = (y0 + 1).min(h - 1);
        let wy = y - y0 as f64;
        for ox in 0..ow {
            let x = ox as f64 * fx;
            if x > (w - 1) as f64 {
                continue;
            }
            let x0 = (x.floor() as usize).min(w - 1);
            let x1 = (x0 + 1).min(w - 1);
            let wx = x - x0 as f64;
            let v = (1.0 - wy) * ((1.0 - wx) * img[y0 * w + x0] as f64 + wx * img[y0 * w + x1] as f64)
                + wy * ((1.0 - wx) * img[y1 * w + x0] as f64 + wx * img[y1 * w + x1] as f64);
            out[oy * ow + ox] = v as f32;
        }
    }
    out
}

pub fn fa_read_line(gray: &[u8], h: usize, w: usize) -> String {
    let Some(r) = rec() else { return String::new() };
    if h == 0 || w == 0 {
        return String::new();
    }
    let ink: Vec<f64> = (0..h).map(|y| gray[y * w..(y + 1) * w].iter().map(|v| 255.0 - *v as f64).sum()).collect();
    let n = ink.len();
    let smooth: Vec<f64> = (0..n)
        .map(|i| {
            let mut s = 0.0;
            for k in 0..9isize {
                let j = i as isize + 4 - k;
                if j >= 0 && (j as usize) < n {
                    s += ink[j as usize] * (1.0 / 9.0);
                }
            }
            s
        })
        .collect();
    let argmax = |v: &[f64]| v.iter().enumerate().fold((0, f64::NEG_INFINITY), |b, (i, x)| if *x > b.1 { (i, *x) } else { b }).0;
    let q = n / 4;
    let peak = if n > 8 { q + argmax(&smooth[q..n - q]) } else { argmax(&smooth) };
    let thr = ink.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 0.06;
    let (mut top, mut bot) = (peak, peak);
    while top > 0 && ink[top - 1] > thr {
        top -= 1;
    }
    while bot + 1 < n && ink[bot + 1] > thr {
        bot += 1;
    }
    let pad = ((bot - top) / 6).max(4);
    let a = top.saturating_sub(pad);
    let b = (bot + pad + 1).min(n);
    let crop: Vec<f32> = gray[a * w..b * w].iter().map(|v| *v as f32).collect();
    let h2 = b - a;
    if h2 < 8 || w < 8 {
        return String::new();
    }
    let nw = ((w as f64 * 48.0 / h2 as f64).round_ties_even() as usize).max(16);
    let oh = (h2 as f64 * (48.0 / h2 as f64)).round_ties_even() as usize;
    let ow = (w as f64 * (nw as f64 / w as f64)).round_ties_even() as usize;
    let im = zoom_bilinear(&crop, h2, w, oh, ow);
    if std::env::var("KHANA_OCRDEBUG").is_ok() {
        let gs: u64 = gray.iter().map(|v| *v as u64).sum();
        eprintln!("line h={h} w={w} graysum={gs} peak={peak} top={top} bot={bot} a={a} b={b} nw={nw} oh={oh} ow={ow} imsum={}", im.iter().map(|v| *v as f64).sum::<f64>());
    }
    let (oh2, ow2) = (oh.min(48), ow.min(nw));
    let mut x = vec![0f32; 3 * oh2 * ow2];
    for c in 0..3 {
        for y in 0..oh2 {
            for xx in 0..ow2 {
                x[(c * oh2 + y) * ow2 + xx] = ((im[y * ow + xx] / 255.0 - 0.5) / 0.5) as f32;
            }
        }
    }
    let name = r.sess.inputs[0].name.clone();
    let Ok(out) = r.sess.run(vec![(name, ort::SessionInputValue::from(ort::Tensor::from_array(([1usize, 3, oh2, ow2], x)).unwrap().into_dyn()))]) else {
        return String::new();
    };
    let Ok((shape, prob)) = out[0].try_extract_raw_tensor::<f32>() else { return String::new() };
    let (t, c) = (shape[1] as usize, shape[2] as usize);
    let mut ids = Vec::with_capacity(t);
    let mut maxes = Vec::with_capacity(t);
    for k in 0..t {
        let row = &prob[k * c..(k + 1) * c];
        let mut best = 0;
        for (i, v) in row.iter().enumerate() {
            if *v > row[best] {
                best = i;
            }
        }
        ids.push(best);
        maxes.push(row[best] as f64);
    }
    let keep: Vec<usize> = ids.iter().enumerate().filter(|(k, i)| **i != 0 && (*k == 0 || **i != ids[k - 1])).map(|(_, i)| *i).collect();
    let conf: Vec<f64> = ids.iter().zip(&maxes).filter(|(i, _)| **i != 0).map(|(_, m)| *m).collect();
    let mean = if conf.is_empty() { f64::NAN } else { (conf.iter().map(|v| *v as f32).sum::<f32>() / conf.len() as f32) as f64 };
    if keep.is_empty() || !(mean >= 0.6) {
        return String::new();
    }
    let text: String = keep.iter().filter(|i| **i < r.chars.len()).map(|i| r.chars[*i].as_str()).collect();
    let text: String = text.chars().rev().collect();
    let text = sub_fn(LTR_RUN_RE, &text, |c| c[0].chars().rev().collect());
    strip(&translate_map(&text, MIRROR)).to_string()
}


fn fa_digits(s: &str) -> String {
    translate_chars(s, "٠١٢٣٤٥٦٧٨٩", "۰۱۲۳۴۵۶۷۸۹")
}

fn skel(ws: &[String]) -> String {
    translate_chars(&sub(r"[^\w]", "", &ws.concat()), "پچژگیب", "بجزکبب")
}

fn core_w(w: &str) -> String {
    sub(r"[^\w‌]", "", w)
}

fn sp_len(x: &str) -> usize {
    nllb_sp().map(|s| s.encode(x).len()).unwrap_or(0)
}

pub fn merge_ocr(apple: &str, farsi: &str) -> String {
    let norm = |t: &str| fa_digits(&translate_map(&nfkc(t), YEH_KAF).replace('ة', "ه"));
    let a: Vec<String> = split_ws(&norm(apple)).iter().map(|s| s.to_string()).collect();
    let b: Vec<String> = split_ws(&norm(farsi)).iter().map(|s| s.to_string()).collect();
    if b.is_empty() {
        return apple.to_string();
    }
    if crate::difflib::ratio_str(&skel(&a), &skel(&b), false) < 0.75 {
        return apple.to_string();
    }
    let vocab = farsi_words();
    let have_sp = nllb_sp().is_some();
    let cost = |ws: &[String]| -> f64 {
        ws.iter()
            .filter(|w| !core_w(w).is_empty())
            .map(|w| {
                let c = core_w(w);
                (if have_sp { sp_len(&c) as f64 } else { 1.0 }) - if vocab.contains(&c.replace('\u{200c}', "")) { 0.6 } else { 0.0 }
            })
            .sum()
    };
    let with_punct = |new: &str, old: &str| -> String {
        let c = core_w(new);
        if c.is_empty() {
            return old.to_string();
        }
        let tail = findall(r"[^\w‌]+$", old).into_iter().next().unwrap_or_default();
        format!("{c}{tail}")
    };
    let mut out: Vec<String> = Vec::new();
    for (op, i1, i2, j1, j2) in SequenceMatcher::new(&a, &b, false).get_opcodes() {
        match op {
            Op::Equal => out.extend(a[i1..i2].iter().cloned()),
            Op::Replace
                if i2 - i1 == j2 - j1
                    && (0..i2 - i1).all(|k| crate::difflib::ratio_str(&skel(&a[i1 + k..i1 + k + 1]), &skel(&b[j1 + k..j1 + k + 1]), true) >= 0.6) =>
            {
                for k in 0..i2 - i1 {
                    let (x, y) = (&a[i1 + k], &b[j1 + k]);
                    let real = vocab.contains(&core_w(y).replace('\u{200c}', ""));
                    out.push(if real && cost(std::slice::from_ref(y)) <= cost(std::slice::from_ref(x)) { with_punct(y, x) } else { x.clone() });
                }
            }
            Op::Replace => {
                if cost(&b[j1..j2]) < cost(&a[i1..i2]) - 0.5 {
                    out.extend(b[j1..j2].iter().cloned())
                } else {
                    out.extend(a[i1..i2].iter().cloned())
                }
            }
            Op::Delete => out.extend(a[i1..i2].iter().cloned()),
            Op::Insert => {}
        }
    }
    out.join(" ")
}


pub fn ocr_columns(lines: &[Line]) -> Vec<Vec<Line>> {
    if lines.len() < 10 {
        return vec![lines.to_vec()];
    }
    let is_rtl = lines.iter().filter(|l| rtl(&l.t)).count() * 2 >= lines.len();
    let n = ((0.651f64 - 0.35) / 0.01).ceil() as usize;
    for k in 0..n {
        let g = 0.35 + k as f64 * 0.01;
        let left: Vec<Line> = lines.iter().filter(|l| l.x1 <= g).cloned().collect();
        let right: Vec<Line> = lines.iter().filter(|l| l.x0 >= g).cloned().collect();
        if left.len() + right.len() == lines.len() && left.len() >= 5 && right.len() >= 5 {
            let cols = if is_rtl { [right, left] } else { [left, right] };
            let mut out = Vec::new();
            for mut col in cols {
                col.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));
                let width = col.iter().map(|l| l.x1).fold(f64::NEG_INFINITY, f64::max) - col.iter().map(|l| l.x0).fold(f64::INFINITY, f64::min);
                while !col.is_empty() && col[0].top < 0.1 && (search(r"[\d۰-۹٠-٩]", &col[0].t) || col[0].x1 - col[0].x0 < 0.6 * width) {
                    col.remove(0);
                }
                out.push(col);
            }
            return out;
        }
    }
    vec![lines.to_vec()]
}

fn median(v: &mut Vec<f64>) -> f64 {
    crate::audio::median(v)
}

pub fn drop_footnotes(col: &[Line]) -> Vec<Line> {
    let mut col = col.to_vec();
    col.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));
    if col.len() < 6 {
        return col;
    }
    let gaps: Vec<f64> = col.windows(2).map(|p| p[1].top - p[0].top).collect();
    let usual = median(&mut gaps.clone());
    let width = median(&mut col.iter().map(|l| l.x1 - l.x0).collect());
    for i in (1..col.len()).rev() {
        let rest = col.len() - i;
        if rest > 22 || col[i].top < 0.45 {
            break;
        }
        if gaps[i - 1] > 2.0 * usual && col[i].x1 - col[i].x0 >= 0.6 * width && i >= 5
            && !col[i..].iter().any(|l| search(r"[\[\]]\s*[\d۰-۹٠-٩]{1,3}\s*[\[\]]", &l.t))
        {
            col.truncate(i);
            return col;
        }
    }
    col
}

pub fn ocr_paragraphs(lines: &[Line]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let cols = ocr_columns(lines);
    if cols.len() > 1 {
        return cols.iter().flat_map(|c| ocr_paragraphs_one(drop_footnotes(c))).collect();
    }
    ocr_paragraphs_one(drop_footnotes(lines))
}

fn ocr_paragraphs_one(mut lines: Vec<Line>) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    lines.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));
    let is_rtl = lines.iter().filter(|l| rtl(&l.t)).count() * 2 >= lines.len();
    let left = lines.iter().map(|l| l.x0).fold(f64::INFINITY, f64::min);
    let right = lines.iter().map(|l| l.x1).fold(f64::NEG_INFINITY, f64::max);
    let width = (right - left).max(1e-6);
    let mut hs: Vec<f64> = lines.iter().map(|l| l.bot - l.top).collect();
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let height = hs[lines.len() / 2];
    let mut gaps: Vec<f64> = lines.windows(2).map(|p| p[1].top - p[0].bot).collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if gaps.is_empty() {
        gaps.push(0.0);
    }
    let usual_gap = gaps[gaps.len() / 2];
    let mut paras = Vec::new();
    let mut cur = String::new();
    for i in 0..lines.len() {
        let l = &lines[i];
        if !cur.is_empty() {
            let prev = &lines[i - 1];
            let gap = l.top - prev.bot;
            let short = (if is_rtl { prev.x0 - left } else { right - prev.x1 }) > 0.15 * width;
            if gap > f64::max(0.8 * height, usual_gap + 0.5 * height) || short {
                paras.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() && search(r"[A-Za-z]-$", &cur) && is_match_start(r"[a-z]", &l.t) {
            cur.pop();
            cur.push_str(&l.t);
        } else {
            cur = strip(&format!("{cur} {}", l.t)).to_string();
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }
    paras
}

#[derive(Default, Debug, serde::Serialize)]
pub struct Trace {
    pub apple: Vec<String>,
    pub fa: Vec<String>,
    pub merged: Vec<String>,
    pub lines: Vec<String>,
}

pub fn ocr_gray(gray: &[u8], w: usize, h: usize) -> Vec<String> {
    ocr_gray_traced(gray, w, h, &mut Trace::default())
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn ocr_gray_traced(gray: &[u8], w: usize, h: usize, tr: &mut Trace) -> Vec<String> {
    ocr_found_lines(gray, w, h, tr)
}

pub fn ocr_found_lines(gray: &[u8], w: usize, h: usize, tr: &mut Trace) -> Vec<String> {
    let mut lines = Vec::new();
    for mut l in find_lines(gray, w, h) {
        let read = read_box(gray, w, h, &l);
        tr.fa.push(read.clone());
        if !read.is_empty() && !fullmatch(r"[\d۰-۹٠-٩\s\-–.]+", &read) {
            l.t = read;
            lines.push(l);
        }
    }
    finish_page(lines, tr)
}

fn read_box(gray: &[u8], w: usize, h: usize, l: &Line) -> String {
    let (y0, y1) = ((l.top * h as f64) as isize, (l.bot * h as f64) as isize);
    let (x0, x1) = ((l.x0 * w as f64) as isize, (l.x1 * w as f64) as isize);
    let pad = (y1 - y0).div_euclid(4);
    let ys = (y0 - pad).max(0) as usize;
    let ye = ((y1 + pad).max(0) as usize).min(h);
    let xs = (x0 - pad).max(0) as usize;
    let xe = ((x1 + pad).max(0) as usize).min(w);
    if ye <= ys || xe <= xs {
        return String::new();
    }
    let cw = xe - xs;
    let mut crop = Vec::with_capacity((ye - ys) * cw);
    for y in ys..ye {
        crop.extend_from_slice(&gray[y * w + xs..y * w + xe]);
    }
    fa_read_line(&crop, ye - ys, cw)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn ocr_gray_traced(gray: &[u8], w: usize, h: usize, tr: &mut Trace) -> Vec<String> {
    let mut lines = vision_lines(&gray, w, h);
    for l in lines.iter_mut() {
        if rtl(&l.t) {
            let read = read_box(gray, w, h, l);
            tr.fa.push(read.clone());
            tr.apple.push(l.t.clone());
            l.t = merge_ocr(&l.t, &read);
            tr.merged.push(l.t.clone());
        }
    }
    finish_page(lines, tr)
}

pub fn drop_note_block(mut lines: Vec<Line>) -> Vec<Line> {
    if ocr_columns(&lines).len() > 1 {
        return lines;
    }
    lines.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));
    let gaps: Vec<f64> = lines.windows(2).map(|p| p[1].top - p[0].top).collect();
    for i in 5..lines.len() {
        if lines[i].top < 0.3 {
            continue;
        }
        let above = median(&mut gaps[..i - 1].to_vec());
        let closer = i + 1 == lines.len() || (gaps[i].min(*gaps.get(i + 1).unwrap_or(&gaps[i])) < 0.85 * above && median(&mut gaps[i..].to_vec()) < 0.85 * above);
        let numbered = lines[i..].iter().any(|l| is_match_start(r"\s*[\d۰-۹٠-٩*]", &l.t));
        let height = |v: &[Line]| median(&mut v.iter().map(|l| l.bot - l.top).collect());
        let width = |v: &[Line]| v.iter().map(|l| l.x1).fold(f64::MIN, f64::max) - v.iter().map(|l| l.x0).fold(f64::MAX, f64::min);
        let small = i + 1 < lines.len() && height(&lines[i..]) < 0.9 * height(&lines[..i]) && width(&lines[i..]) >= 0.9 * width(&lines[..i]);
        if closer && gaps[i - 1] > 1.25 * above && (numbered || small) {
            lines.truncate(i);
            break;
        }
    }
    lines
}

pub fn finish_page(lines: Vec<Line>, tr: &mut Trace) -> Vec<String> {
    let mut lines = drop_note_block(lines);
    if lines.iter().filter(|l| rtl(&l.t)).count() * 2 >= lines.len() {
        lines.retain(|l| rtl(&l.t));
    }
    lines.retain(|l| split_ws(&l.t).iter().map(|w| len(w)).max().unwrap_or(0) > 1);
    tr.lines = lines.iter().map(|l| l.t.clone()).collect();
    let paras: Vec<String> = ocr_paragraphs(&lines).into_iter().map(|p| if rtl(&p) { fix_ocr_farsi(&p) } else { p }).collect();
    if paras.iter().map(|p| split_ws(p).len()).sum::<usize>() <= 2 {
        return Vec::new();
    }
    paras
}


fn lk<'a>(m: &'a [(&str, &str)], k: &str) -> Option<&'a str> {
    m.iter().find(|(a, _)| *a == k).map(|(_, v)| *v)
}

fn in_v(w: &str) -> bool {
    farsi_words().contains(&w.replace('\u{200c}', ""))
}

fn pieces(x: &str) -> usize {
    if nllb_sp().is_some() { sp_len(x) } else { 9 }
}

fn rank(c: &str, core: &str) -> (bool, f64, bool) {
    let n = if nllb_sp().is_some() { sp_len(c) as f64 } else { 0.0 };
    (!OCR_PREFER.contains(&c), n - if in_v(c) { 0.5 } else { 0.0 }, c != core)
}

fn rank_lt(a: &(bool, f64, bool), b: &(bool, f64, bool)) -> bool {
    a.partial_cmp(b) == Some(std::cmp::Ordering::Less)
}

fn min_rank(items: &[String], core: &str) -> Option<String> {
    let mut best: Option<(&String, (bool, f64, bool))> = None;
    for it in items {
        let r = rank(it, core);
        if best.as_ref().map_or(true, |(_, br)| rank_lt(&r, br)) {
            best = Some((it, r));
        }
    }
    best.map(|b| b.0.clone())
}

fn product(options: &[Vec<char>]) -> Vec<String> {
    let mut out = vec![String::new()];
    for o in options {
        let mut next = Vec::with_capacity(out.len() * o.len());
        for p in &out {
            for c in o {
                let mut s = p.clone();
                s.push(*c);
                next.push(s);
            }
        }
        out = next;
    }
    out
}

fn swap_options(x: &str) -> Vec<Vec<char>> {
    x.chars().map(|c| lk(OCR_SWAP, &c.to_string()).map(|s| s.chars().collect()).unwrap_or_else(|| vec![c])).collect()
}

fn best_swap(x: &str) -> String {
    let options = swap_options(x);
    if options.iter().map(|o| o.len()).product::<usize>() > 32 {
        return x.to_string();
    }
    min_rank(&product(&options), x).unwrap_or_else(|| x.to_string())
}

fn word_core(w: &str) -> String {
    sub(r"^[^\w]+|[^\w‌]+$", "", w)
}

fn replace_all(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        let mut out = String::from(new);
        for c in s.chars() {
            out.push(c);
            out.push_str(new);
        }
        return out;
    }
    s.replace(old, new)
}

fn char_slice(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}

const OCR_LOOKALIKES: &[(char, &[char])] = &[('ک', &['گ']), ('پ', &['ب', 'چ']), ('ج', &['چ']), ('ر', &['ژ']), ('ز', &['ژ']), ('چ', &['ج'])];
const OCR_TAILS: &[&str] = &["شناسی", "شناسانه", "شناسان", "شناس", "نگاری", "نگار", "نگاران", "نگارانی", "پرداز", "پردازی", "محور", "محوری", "محورانه", "سازی", "بندی", "گرایی", "پذیری", "طلبی", "نویس", "نویسان", "نویسی", "گذاری", "زدایی", "کشی", "شکنان", "انکار", "انکارهای", "مقالات"];
const OCR_GLUED_TAILS: &[&str] = &["اش", "شان", "مان", "تر", "تری", "ام", "اند", "ایم", "شدن", "شده", "شدگی", "کردن", "دادن", "زدن", "نمودن", "کننده", "یافته", "آوردن", "داشتن", "ورزی"];
const OCR_STEMS: &[&str] = &["رفت", "گرفت", "ساخت", "داشت", "کرد", "بود", "آمد", "داد", "دید", "گفت", "نوشت", "یافت", "برد", "خورد", "شناخت"];
const OCR_NOT_BE: &[&str] = &["بهرام", "بهشتی", "بهشت", "بهروز", "بهمن", "بهزاد", "بهنام", "بهرنگ", "بهار", "بهاری", "بهره", "بهتر", "بهترین", "بهانه", "بهداشت", "بهبود", "بهایی", "بهای", "بهینه", "بهنود", "بهزیستی"];

fn mend_word(w: &str) -> String {
    let c = crate::vocab::word_costs();
    let cost = |x: &str| c.get(x).map(|v| v.cost).unwrap_or(99.0);
    let n = w.chars().count();
    if n < 3 || ["های", "هایی", "ها", "هایش", "هایشان"].contains(&w) {
        return w.to_string();
    }
    let whole = cost(w);
    let known = whole >= 99.0 && (in_v(w) || nllb_sp().is_some() && sp_len(w) <= 1);
    if whole >= 15.0 && !known {
        let chars: Vec<char> = w.chars().collect();
        let mut best: Option<(f32, String)> = None;
        for i in 0..n {
            for (a, to) in OCR_LOOKALIKES {
                if chars[i] != *a {
                    continue;
                }
                for b in to.iter() {
                    let mut d = chars.clone();
                    d[i] = *b;
                    let t: String = d.iter().collect();
                    let k = cost(&t);
                    if k <= 13.0 && whole - k >= 4.0 && best.as_ref().is_none_or(|x| k < x.0) {
                        best = Some((k, t));
                    }
                }
            }
        }
        if let Some((_, t)) = best {
            return t;
        }
    }
    let common = |x: &str, most: f32| x.chars().count() >= 3 && cost(x) <= most;
    if whole >= 15.0 && !known {
        for head in ["م", "فی"] {
            if let Some(rest) = w.strip_prefix(head) {
                if search(r"(?:د|ند|م|یم)$", rest) && cost(&format!("می{rest}")).min(cost(rest)) <= 13.0 && rest.chars().count() >= 2 {
                    return format!("می\u{200c}{rest}");
                }
            }
        }
    }
    if let Some(head) = w.strip_suffix('ی') {
        if head.ends_with('ه') && head.chars().count() >= 4 && whole >= 15.0 {
            return format!("{}\u{200c}ی", mend_word(head));
        }
    }
    if whole >= 15.0 && !known {
        for tail in OCR_GLUED_TAILS {
            if let Some(head) = w.strip_suffix(tail) {
                if common(head, 13.5) {
                    return format!("{head}\u{200c}{tail}");
                }
            }
        }
    }
    if whole >= 11.5 && !known {
        let chars: Vec<char> = w.chars().collect();
        for i in (3..n.saturating_sub(2)).rev() {
            if chars[i - 1] == 'ه' {
                let (h, r): (String, String) = (chars[..i].iter().collect(), chars[i..].iter().collect());
                if common(&h, 13.0) && common(&r, 13.0) {
                    return format!("{h}\u{200c}{r}");
                }
            }
        }
    }
    if let Some(head) = w.strip_suffix("ترین") {
        if common(head, 13.0) {
            return format!("{head}\u{200c}ترین");
        }
    }
    if let Some(rest) = w.strip_prefix('و') {
        if whole >= 15.0 && !known && common(rest, 13.0) {
            return format!("و {rest}");
        }
    }
    for tail in ["هایشان", "هایش", "هایی", "های", "ها"] {
        if let Some(head) = w.strip_suffix(tail) {
            if tail == "های" && cost(&format!("{head}ه")) < cost(head).min(13.0) {
                return format!("{head}ه\u{200c}ای");
            }
            if common(head, 13.0) && !OCR_STEMS.contains(&head) {
                return format!("{head}\u{200c}{tail}");
            }
        }
    }
    if let Some(head) = w.strip_suffix('ی') {
        if head.ends_with('ه') && common(head, 13.0) && whole >= 11.0 {
            return format!("{head}\u{200c}ی");
        }
    }
    if whole >= 11.5 {
        for tail in OCR_TAILS {
            if let Some(head) = w.strip_suffix(tail) {
                if common(head, 13.0) {
                    return format!("{head}\u{200c}{tail}");
                }
            }
        }
    }
    if let Some(head) = w.strip_suffix("هشده") {
        if common(&format!("{head}ه"), 13.0) {
            return format!("{head}ه\u{200c}شده");
        }
    }
    if let Some(rest) = w.strip_prefix("به") {
        if !OCR_NOT_BE.contains(&w) && whole >= 11.4 && common(rest, 13.0) {
            return format!("به\u{200c}{rest}");
        }
    }
    w.to_string()
}

fn mend_ocr_page(text: &str) -> String {
    let mut t = sub(r#"(?<=[ء-یَ-ْ!؟])['"’”°*]+(?=[\s،.؛:)»]|$)|(?<![^\s(«])['"’”°*]+\s*(?=[ء-ی])"#, "", text);
    t = sub(r"\]([^\[\]]{1,60})\[", "[\\1]", &t);
    t = sub(r"(?<=[۰-۹])\((?=[\s،.؛]|$)", ")", &t);
    for (a, b) in OCR_MENDED {
        t = sub(&format!(r"(?<![\p{{L}}]){a}"), b, &t);
    }
    t = sub(r"(?<=\p{L}) مود(?=[\s.،؛:)]|$)", " نمود", &t);
    t = sub(r"(?<![\p{L}\x{200c}])فی (?=\p{L}+(?:د|ند|یم|م|ان)(?![\p{L}]))", "می\u{200c}", &t);
    t = sub(r"(?<![\p{L}\x{200c}])مشد(?![\p{L}\x{200c}])", "می\u{200c}شد", &t);
    t = sub(r"(?<=\p{L}{3})ک(رانه|رایانه|رایان|رایی|راترین)(?=ی?(?![\p{L}]))", "گ\\1", &t);
    t = sub(r"(?<![\p{L}])فمی(?=[\s\x{200c}]\p{L}+(?:د|ند|یم|م|ان)(?![\p{L}]))", "نمی", &t);
    t = sub(r"(?<![\p{L}])(اجما|احتما)لً", "\\1لاً", &t);
    t = sub(r"نزاد", "نژاد", &t);
    t = sub(r"(?<![\p{L}])استرات[رز]", "استراتژ", &t);
    t = crate::normfa::join_compounds(&t);
    t = sub(r"(?<=\p{L}) (ها|های|هایی|هایشان|هایش|ترین)(?![\p{L}])", "\u{200c}\\1", &t);
    t = sub(r"(?<![\p{L}\x{200c}])(ن?می) (?=\p{L})", "\\1\u{200c}", &t);
    sub_fn(r"[\p{L}\x{200c}]+", &t, |m| m[0].split('\u{200c}').map(mend_word).collect::<Vec<_>>().join("\u{200c}"))
}

pub fn fix_ocr_farsi(text: &str) -> String {
    mend_ocr_page(&fix_ocr_farsi_first(text))
}

fn fix_ocr_farsi_first(text: &str) -> String {
    let mut t = translate_map(&nfkc(text), YEH_KAF).replace('ة', "ه");
    t = fa_digits(&t);
    t = sub_fn(r"([^\W\d]+) ?([۰-۹]+)([^\W\d]+)\b", &t, |m| {
        if len(&m[3]) == 1 && &m[3] != "و" { format!("{}{} {}", &m[1], &m[3], &m[2]) } else { m[0].to_string() }
    });
    t = sub("ک\u{064e}", "گ", &t);
    t = sub(r"(?<![\d۰-۹])\+|\+(?![\d۰-۹])", "", &t);
    if t.contains('«') && !t.contains('»') {
        t = sub_n(r"[()](?=[^«]*$)", "»", &t, 1);
    }
    t = sub(r"\(\(|[%!][%!,\s]*(?=[^\W\d])", "«", &t);
    t = sub(r"\((?=[^()«»]{1,60}»)", "«", &t);
    t = sub(r"[%]+", "", &t);
    t = sub(r"[@/]{2,}", " ", &t);
    t = sub(r"(?<![\w‌])ب(?=آ)", "به ", &t);
    t = sub_fn(r"([ء-ی]+)-([ء-ی]+)", &t, |m| if len(&m[1]).min(len(&m[2])) <= 2 { format!("{}{}", &m[1], &m[2]) } else { m[0].to_string() });
    t = sub(r"(?<!\S)-+[^\s-]{0,3}-+(?!\S)", " ", &t);
    t = sub(r"(?:[-'’]{1,2}){2,}[ء-ی]?(?=\s|$)", "", &t);
    t = sub(r"[-'’]{3,}", " ", &t);
    t = sub(r"(?<=[ء-ی])\((?=[ء-ی])", " ", &t);
    t = sub(r"(?<=[ء-ی]) [۰-۹]{1,2} (?!و [۰-۹])(?=(?:برای|و|در|که|به|از|را|با|نیز|هم|بود|است|این|آن|تا|اما|یا)(?![ء-ی]))", " ", &t);
    t = sub(r"\(\s*»|«\s*\)", " ", &t);
    t = sub(r"(?<=[ء-ی])(?<!ه)ء(?=\s|$)", "", &t);
    t = sub(r"»(?=ای(?![\w]))", "\u{200c}", &t);
    t = sub(r"(?<=[ء-ی]ه) (اند|ایم|اید)(?![\w])", "\u{200c}\\1", &t);
    t = sub(r"(?<=[ء-ی])[۰-۹]{1,2}(?=[\s،.؛:»)]|$)", "", &t);
    t = sub(r"(?<=[ء-ی]ه) ی(?!\S)", "\u{200c}ی", &t);
    t = sub_fn(r"(?<!\S)[ء-ًیآا](?!\S)", &t, |m| if &m[0] == "و" { "و".into() } else { String::new() });
    t = sub(r"(?<![\w])أ(?=\w)", "ا", &t);
    t = sub(r"(?<!\S)[A-Za-z]{1,2}(?!\S)|[<>|]", "", &t);
    if t.matches('(').count() > t.matches(')').count() {
        t = sub(r"\((?![^()]*\))", "", &t);
    }
    t = strip(&sub(r"\s{2,}", " ", &t)).to_string();
    t = sub(r"(?<![\w‌])بس از(?![\w‌])", "پس از", &t);
    for (a, b) in OCR_PHRASES {
        t = t.replace(a, b);
    }
    t = sub_fn(r"[\w‌]+", &t, |m| lk(OCR_GLUED, &m[0]).unwrap_or(&m[0]).to_string());
    t = sub(r"(?<=[ء-ی])ه(اند|ایم|اید)(?![\w])", "ه\u{200c}\\1", &t);
    t = sub(r"(?<=[ء-ی]{4})ه(است)(?![\w])", "ه\u{200c}\\1", &t);
    let vocab = farsi_words();
    if vocab.is_empty() {
        return t;
    }
    let have_sp = nllb_sp().is_some();

    let mut out: Vec<String> = Vec::new();
    for w in t.split(' ') {
        let core = word_core(w);
        let options = swap_options(&core);
        if core.is_empty() || options.iter().map(|o| o.len()).product::<usize>() > 64
            || (options.iter().all(|o| o.len() == 1) && !core.chars().any(|c| lk(OCR_CONFUSE, &c.to_string()).is_some()))
        {
            out.push(w.to_string());
            continue;
        }
        let cands = product(&options);
        let mut best = min_rank(&cands, &core).unwrap();
        if best != core && !OCR_PREFER.contains(&best.as_str()) && !in_v(&best) {
            best = core.clone();
        }
        if best != core && !OCR_PREFER.contains(&best.as_str()) && in_v(&core) && have_sp && sp_len(&best) >= sp_len(&core) {
            best = core.clone();
        }
        if best != core && best.contains('پ') && in_v(&core) && have_sp && sp_len(&best) >= sp_len(&core) {
            best = core.clone();
        }
        let listed = in_v(&best);
        if (!listed || (have_sp && sp_len(&best) >= 3)) && !OCR_PREFER.contains(&best.as_str()) {
            let cc: Vec<char> = core.chars().collect();
            let mut alt: Vec<String> = Vec::new();
            for (i, c) in cc.iter().enumerate() {
                for r in lk(OCR_CONFUSE, &c.to_string()).unwrap_or("").chars() {
                    let mut x: String = cc[..i].iter().collect();
                    x.push(r);
                    x.extend(cc[i + 1..].iter());
                    alt.push(x);
                }
            }
            let br = rank(&best, &core).1;
            alt.retain(|a| in_v(a) && rank(a, &core).1 < br && (!listed || sp_len(a) == 1));
            if !alt.is_empty() {
                best = min_rank(&alt, &core).unwrap();
            }
        }
        out.push(replace_all(w, &core, &best));
    }

    let glue = ["را", "از", "که", "به", "با", "در", "هیچ", "هیج", "این", "آن"];
    let mut k = 0;
    while k < out.len() {
        let core = word_core(&out[k]);
        let cc: Vec<char> = core.chars().collect();
        let mut parts: Option<Vec<String>> = None;
        if cc.len() >= 5 && !in_v(&core) {
            for g in glue {
                let rest = char_slice(&core, len(g), cc.len());
                if core.starts_with(g) && len(&rest) >= 2
                    && (in_v(&best_swap(&rest)) || (["از", "را", "که"].contains(&g) && len(&rest) >= 3 && pieces(&rest) < pieces(&core)))
                {
                    parts = Some(vec![best_swap(g), best_swap(&rest)]);
                    break;
                }
            }
            if parts.is_none() && cc[0] == 'ا' && vocab.contains(&char_slice(&core, 1, cc.len())) {
                parts = Some(vec![char_slice(&core, 1, cc.len())]);
            }
        }
        if parts.is_none() && core.starts_with("را") && cc.len() >= 4 && have_sp {
            let rest = best_swap(&char_slice(&core, 2, cc.len()));
            let listed = in_v(&core);
            let (pc, pr) = (sp_len(&core), sp_len(&rest));
            if (["با", "به", "از", "در", "هم", "نیز", "که"].contains(&rest.as_str()) || (in_v(&rest) && pr < pc && (!listed || pc >= 3)))
                && !["رایج", "راست", "راحت", "رابطه", "راهی", "رازی", "رامش"].contains(&core.as_str())
            {
                parts = Some(vec!["را".into(), rest]);
            }
        }
        if parts.is_none() && cc.len() >= 6 && !in_v(&core) && have_sp {
            for cut in 2..cc.len() - 1 {
                let left = best_swap(&char_slice(&core, 0, cut));
                let right = best_swap(&char_slice(&core, cut, cc.len()));
                if len(&left) >= 3 && len(&right) >= 3 && vocab.contains(&left) && vocab.contains(&right)
                    && sp_len(&left) + sp_len(&right) < sp_len(&core) && sp_len(&left).max(sp_len(&right)) <= 2
                {
                    parts = Some(vec![left, right]);
                    break;
                }
            }
        }
        if parts.is_none() && cc.len() >= 5 && !in_v(&core) && have_sp {
            for g in ["به", "را", "از", "با"] {
                let head = char_slice(&core, 0, cc.len() - len(g));
                if core.ends_with(g) && len(&head) >= 3 && vocab.contains(&head) && sp_len(&head) <= 2 {
                    parts = Some(vec![head, g.to_string()]);
                    break;
                }
            }
        }
        if parts.is_none() && cc.len() >= 4 && core.ends_with("که") && have_sp {
            let head = char_slice(&core, 0, cc.len() - 2);
            if vocab.contains(&head) && sp_len(&head) == 1 && sp_len(&core) >= 2 {
                parts = Some(vec![head, "که".into()]);
            }
        }
        if let Some(parts) = parts {
            let word = out[k].clone();
            let (head, tail) = match word.find(&core) {
                Some(i) if !core.is_empty() => (word[..i].to_string(), word[i + core.len()..].to_string()),
                _ => (String::new(), String::new()),
            };
            let mut repl = vec![format!("{head}{}", parts[0])];
            if parts.len() > 1 {
                repl.extend(parts[1..parts.len() - 1].iter().cloned());
                repl.push(format!("{}{tail}", parts[parts.len() - 1]));
            }
            if parts.len() == 1 {
                repl[0].push_str(&tail);
            }
            out.splice(k..k + 1, repl);
            continue;
        }
        k += 1;
    }

    let silent = |b: &str| pronounce_word(b).ends_with('e') && b != "اله";
    for i in 0..out.len() {
        let w = out[i].clone();
        let mut core = word_core(&w);
        let cc: Vec<char> = core.chars().collect();
        let rest = if cc.len() >= 4 { best_swap(&char_slice(&core, 1, cc.len())) } else { char_slice(&core, 1, cc.len()) };
        let head = core.split('\u{200c}').next().unwrap_or("").to_string();
        let tail1 = char_slice(&core, 1, cc.len());
        if cc.first() == Some(&'و')
            && (["از", "با", "در", "به", "بر", "این", "آن"].contains(&tail1.as_str())
                || (!(core.contains('\u{200c}') && len(&head) >= 3 && vocab.contains(&head))
                    && cc.len() >= 4
                    && in_v(&rest)
                    && !OCR_PREFER.contains(&core.as_str())
                    && (!in_v(&core) || pieces(&core) > pieces(&rest) || (pieces(&core) == pieces(&rest) && pieces(&rest) >= 3 && rest != tail1))))
        {
            out[i] = replace_all(&w, &core, &format!("و {rest}"));
            core = rest;
        }
        let mut m2 = fullmatch_groups(r"(.+ه)(ای|است)", &core);
        if let Some(g) = &m2 {
            if !silent(g[1].as_deref().unwrap()) {
                m2 = None;
            }
        }
        let plural = fullmatch_groups(r"(.+ها)(ی|ست)", &core);
        if let (Some(g2), Some(p)) = (&m2, &plural) {
            let (s2, sp) = (g2[1].clone().unwrap(), p[1].clone().unwrap());
            if pieces(&sp) < pieces(&s2) || (pieces(&sp) == pieces(&s2) && vocab.contains(&sp)) {
                m2 = None;
            }
        }
        let segs: Vec<String> = nllb_sp().map(|s| s.encode_pieces(&core)).unwrap_or_default();
        if let (Some(g2), Some(p)) = (&m2, &plural) {
            let (s2, e2, sp) = (g2[1].clone().unwrap(), g2[2].clone().unwrap(), p[1].clone().unwrap());
            if e2 == "ای" && vocab.contains(&sp) {
                let last_id = nllb_sp().map(|s| s.encode(&s2).last().copied().unwrap_or(0)).unwrap_or(0);
                if segs.last().map(|x| x.as_str()) == Some("های") || (segs.len() == 1 && last_id > 120000) {
                    m2 = None;
                }
            }
        }
        if let Some(g2) = &m2 {
            let (s2, e2) = (g2[1].clone().unwrap(), g2[2].clone().unwrap());
            if vocab.contains(&s2) {
                out[i] = replace_all(&out[i], &core, &format!("{s2}\u{200c}{e2}"));
                continue;
            }
        }
        if let Some(m) = fullmatch_groups(r"(.+ه)(ی|ها|های|هایی)", &core) {
            let (s, e) = (m[1].clone().unwrap(), m[2].clone().unwrap());
            let last2: String = s.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
            if silent(&s) && in_v(&s)
                && (!in_v(&core) || pieces(&core) > pieces(&s) || (pieces(&core) == pieces(&s) && !["ده", "جه", "به", "قه", "گه"].contains(&last2.as_str())))
            {
                out[i] = replace_all(&out[i], &core, &format!("{s}\u{200c}{e}"));
            }
        }
    }

    let fragment = |x: &str| -> bool {
        let n = len(x);
        if n == 1 {
            return x != "و";
        }
        if n == 2 && !OCR_SHORT_WORDS.contains(&x) && have_sp {
            let ids = nllb_sp().unwrap().encode(x);
            return ids.len() > 1 || ids.first().copied().unwrap_or(0) > 120000;
        }
        false
    };
    let mut k = 0;
    while k + 1 < out.len() {
        let mut joined = false;
        for n in [3usize, 2] {
            if k + n > out.len() {
                continue;
            }
            let parts = &out[k..k + n];
            let whole: String = parts.concat();
            if fullmatch(r"[\w‌]+", &whole) && vocab.contains(&whole) && parts.iter().any(|x| fragment(x)) {
                out.splice(k..k + n, [whole]);
                joined = true;
                break;
            }
        }
        if !joined {
            k += 1;
        }
    }
    let glued = |w: &str| -> String {
        let core = word_core(w);
        match lk(OCR_GLUED, &core) {
            Some(g) => replace_all(w, &core, g),
            None => w.to_string(),
        }
    };
    let mut t = out.iter().map(|w| glued(w)).collect::<Vec<_>>().join(" ");
    for (a, b) in OCR_PHRASES {
        t = t.replace(a, b);
    }
    t = sub(r"(?<=[ء-ی]ه) (اند|ایم|اید)(?![\w])", "\u{200c}\\1", &t);
    sub(r"(?<=[.؟!»])\s*[()](?=\s|$)", "", &t)
}

pub fn ready() -> bool {
    true
}

pub static OCR_LOCK: Mutex<()> = Mutex::new(());

