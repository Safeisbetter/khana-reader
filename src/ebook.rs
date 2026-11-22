
use crate::pyre::*;
use std::io::Read;

const PART_LIMIT: u64 = 64 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 128 * 1024 * 1024;
pub const MAX_PARAGRAPHS: usize = 2_000_000;
const MAX_XML_NODES: u32 = 4_000_000;

pub fn xml_options<'a>(allow_dtd: bool) -> roxmltree::ParsingOptions<'a> {
    roxmltree::ParsingOptions { allow_dtd, nodes_limit: MAX_XML_NODES, ..Default::default() }
}

const MAX_XML_DEPTH: usize = 256;

pub fn too_deep(doc: &roxmltree::Document) -> bool {
    doc.descendants().any(|n| n.is_element() && n.ancestors().take(MAX_XML_DEPTH + 1).count() > MAX_XML_DEPTH)
}

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
    let opt = xml_options(true);
    let safe = xml_safe(xml);
    let Ok(doc) = roxmltree::Document::parse_with_options(&safe, opt) else { return Vec::new() };
    if too_deep(&doc) {
        return Vec::new();
    }
    blocks_from_nodes(doc.descendants())
}

fn blocks_from_nodes<'a, 'input: 'a>(
    nodes: impl Iterator<Item = roxmltree::Node<'a, 'input>>,
) -> Vec<String> {
    let mut out = Vec::new();
    for n in nodes.filter(|n| n.is_element() && BLOCKS.contains(&n.tag_name().name())) {
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
            if out.len() >= MAX_PARAGRAPHS {
                break;
            }
        }
    }
    out
}

pub fn epub_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let bad = || "این کتاب EPUB باز نشد. شاید خراب باشد.".to_string();
    let too_big = || "این کتاب EPUB خیلی بزرگ است.".to_string();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|_| bad())?;
    let container = part(&mut z, "META-INF/container.xml").ok_or_else(bad)?;
    let c = roxmltree::Document::parse_with_options(&container, xml_options(false)).map_err(|_| bad())?;
    if too_deep(&c) {
        return Err(bad());
    }
    let opf_path = c.descendants().find(|n| n.tag_name().name() == "rootfile").and_then(|n| n.attribute("full-path")).ok_or_else(bad)?.to_string();
    let base = opf_path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
    let opf = part(&mut z, &opf_path).ok_or_else(bad)?;
    let o = roxmltree::Document::parse_with_options(&opf, xml_options(true)).map_err(|_| bad())?;
    if too_deep(&o) {
        return Err(bad());
    }
    let items: Vec<(String, String)> = o
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|n| Some((n.attribute("id")?.to_string(), n.attribute("href")?.to_string())))
        .collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let by_id: std::collections::HashMap<&str, &str> = items.iter().rev().map(|i| (i.0.as_str(), i.1.as_str())).collect();
    let mut used: u64 = 0;
    let mut out = Vec::new();
    for r in o.descendants().filter(|n| n.tag_name().name() == "itemref") {
        let Some(href) = r.attribute("idref").and_then(|id| by_id.get(id)).map(|h| h.to_string()) else { continue };
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
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(x) = part(&mut z, &name) {
            used = used.saturating_add(x.len() as u64);
            out.extend(blocks(&x));
            if used > TOTAL_LIMIT || out.len() >= MAX_PARAGRAPHS {
                return Err(too_big());
            }
        }
    }
    if out.is_empty() {
        return Err("در این کتاب EPUB متنی پیدا نشد.".into());
    }
    Ok(out)
}

pub fn fb2_paragraphs(data: &[u8]) -> Result<Vec<String>, String> {
    let text = String::from_utf8_lossy(data);
    let safe = xml_safe(&text);
    let doc = roxmltree::Document::parse_with_options(&safe, xml_options(true))
        .map_err(|_| "این کتاب FB2 باز نشد. شاید خراب باشد.".to_string())?;
    if too_deep(&doc) {
        return Err("این کتاب FB2 باز نشد. شاید خراب باشد.".into());
    }
    let body_nodes = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "body")
        .flat_map(|body| body.descendants());
    let out = blocks_from_nodes(body_nodes);
    if out.is_empty() {
        return Err("در این کتاب FB2 متنی پیدا نشد.".into());
    }
    if out.len() >= MAX_PARAGRAPHS {
        return Err("این کتاب FB2 خیلی بزرگ است.".into());
    }
    Ok(out)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
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

#[cfg(test)]
mod reader_tests {
    use super::{epub_paragraphs, fb2_paragraphs};
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn epub_preserves_spine_order_and_paragraphs() {
        let container = r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/book.opf"/></rootfiles></container>"#;
        let opf = r#"<package xmlns="http://www.idpf.org/2007/opf"><manifest><item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="two.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="two"/><itemref idref="one"/></spine></package>"#;
        let first = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><h1>Second chapter</h1><p>Last text.</p></body></html>"#;
        let second = r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>First in spine.</p></body></html>"#;
        let data = archive(&[("META-INF/container.xml", container), ("OEBPS/book.opf", opf), ("OEBPS/one.xhtml", first), ("OEBPS/two.xhtml", second)]);
        assert_eq!(epub_paragraphs(&data).unwrap(), ["First in spine.", "Second chapter", "Last text."]);
    }

    #[test]
    fn epub_rejects_corrupt_archives_and_deep_metadata() {
        assert!(epub_paragraphs(b"not a zip").is_err());
        let mut container = String::from(r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container">"#);
        for _ in 0..260 {
            container.push_str("<x>");
        }
        for _ in 0..260 {
            container.push_str("</x>");
        }
        container.push_str("</container>");
        let data = archive(&[("META-INF/container.xml", &container)]);
        assert!(epub_paragraphs(&data).is_err());
    }

    #[test]
    fn epub_rejects_overdeep_package_metadata() {
        let container = r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfile full-path="book.opf"/></container>"#;
        let mut opf = String::from(r#"<package xmlns="http://www.idpf.org/2007/opf">"#);
        for _ in 0..260 {
            opf.push_str("<x>");
        }
        for _ in 0..260 {
            opf.push_str("</x>");
        }
        opf.push_str("</package>");
        let data = archive(&[("META-INF/container.xml", container), ("book.opf", &opf)]);
        assert!(epub_paragraphs(&data).is_err());
    }

    #[test]
    fn fb2_reads_paragraphs_and_rejects_corrupt_xml() {
        let fb2 = b"<FictionBook><body><section><p>First.</p><p>Second.</p></section></body></FictionBook>";
        assert_eq!(fb2_paragraphs(fb2).unwrap(), ["First.", "Second."]);
        assert!(fb2_paragraphs(b"<FictionBook><body><p>truncated").is_err());
        assert!(fb2_paragraphs(b"not an FB2 document").is_err());
        let mut deep = String::from("<FictionBook><body>");
        for _ in 0..260 {
            deep.push_str("<x>");
        }
        deep.push_str("text");
        for _ in 0..260 {
            deep.push_str("</x>");
        }
        deep.push_str("</body></FictionBook>");
        assert!(fb2_paragraphs(deep.as_bytes()).is_err());
    }
}



