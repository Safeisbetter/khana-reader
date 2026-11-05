
use crate::pyre::*;
use std::io::Read;

const PART_LIMIT: u64 = 64 * 1024 * 1024;

fn part(z: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Option<String> {
    let f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.take(PART_LIMIT).read_to_string(&mut s).ok()?;
    Some(s)
}

fn xml_safe(html: &str) -> String {
    let named = [("&nbsp;", "&#160;"), ("&zwnj;", "&#8204;"), ("&zwj;", "&#8205;"), ("&lrm;", "&#8206;"), ("&rlm;", "&#8207;"),
                 ("&laquo;", "&#171;"), ("&raquo;", "&#187;"), ("&ndash;", "&#8211;"), ("&mdash;", "&#8212;"), ("&hellip;", "&#8230;"),
                 ("&lsquo;", "&#8216;"), ("&rsquo;", "&#8217;"), ("&ldquo;", "&#8220;"), ("&rdquo;", "&#8221;"), ("&shy;", "")];
    let mut s = html.to_string();
    for (a, b) in named {
        s = s.replace(a, b);
    }
    sub(r"&(?!#\d+;|#x[0-9A-Fa-f]+;|amp;|lt;|gt;|quot;|apos;)[A-Za-z][A-Za-z0-9]*;", "", &s)
}

const BLOCKS: &[&str] = &["p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "blockquote", "td", "th", "dd", "dt", "pre", "title", "subtitle", "v", "text-author"];

fn blocks(xml: &str) -> Vec<String> {
    let opt = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let safe = xml_safe(xml);
    let Ok(doc) = roxmltree::Document::parse_with_options(&safe, opt) else { return Vec::new() };
    let mut out = Vec::new();
    for n in doc.descendants().filter(|n| n.is_element() && BLOCKS.contains(&n.tag_name().name())) {
        let mut t = String::new();
        for d in n.descendants() {
            if d.is_element() && ["br"].contains(&d.tag_name().name()) {
                t.push(' ');
            } else if d.is_text() && !d.ancestors().any(|a| a.is_element() && ["script", "style", "rt"].contains(&a.tag_name().name())) {
                let inner = d.ancestors().take_while(|a| *a != n).any(|a| a.is_element() && BLOCKS.contains(&a.tag_name().name()));
                if !inner {
                    t.push_str(d.text().unwrap_or(""));
                }
            }
        }
        let t = split_ws(&t).join(" ");
        if !t.is_empty() {
            out.push(t);
        }
    }
    out
}

pub fn epub_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let bad = || "این کتاب EPUB باز نشد. شاید خراب باشد.".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| bad())?;
    let container = part(&mut z, "META-INF/container.xml").ok_or_else(bad)?;
    let c = roxmltree::Document::parse(&container).map_err(|_| bad())?;
    let opf_path = c.descendants().find(|n| n.tag_name().name() == "rootfile").and_then(|n| n.attribute("full-path")).ok_or_else(bad)?.to_string();
    let base = opf_path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
    let opf = part(&mut z, &opf_path).ok_or_else(bad)?;
    let o = roxmltree::Document::parse_with_options(&opf, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }).map_err(|_| bad())?;
    let items: Vec<(String, String)> = o
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|n| Some((n.attribute("id")?.to_string(), n.attribute("href")?.to_string())))
        .collect();
    let mut out = Vec::new();
    for r in o.descendants().filter(|n| n.tag_name().name() == "itemref") {
        let Some(href) = r.attribute("idref").and_then(|id| items.iter().find(|i| i.0 == id)).map(|i| i.1.clone()) else { continue };
        let mut path: Vec<&str> = Vec::new();
        let full = format!("{base}{}", href.split('#').next().unwrap_or(""));
        for seg in full.split('/') {
            match seg {
                ".." => {
                    path.pop();
                }
                "" | "." => {}
                s => path.push(s),
            }
        }
        let name = percent_decode(&path.join("/"));
        if let Some(x) = part(&mut z, &name) {
            out.extend(blocks(&x));
        }
    }
    if out.is_empty() {
        return Err("در این کتاب EPUB متنی پیدا نشد.".into());
    }
    Ok(out)
}

pub fn fb2_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let text = String::from_utf8_lossy(data);
    let body: String = findall(r"(?s)<body[^>]*>.*?</body>", &text).join("\n");
    let out = blocks(&format!("<fb>{body}</fb>"));
    if out.is_empty() {
        return Err("در این کتاب FB2 متنی پیدا نشد.".into());
    }
    Ok(out)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

