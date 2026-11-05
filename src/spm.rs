
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

pub struct SentencePiece {
    piece_id: HashMap<String, u32>,
    score: HashMap<String, f32>,
    pieces: Vec<String>,
    unk_id: u32,
    trie: Vec<u32>,
    normalized: Vec<u8>,
    add_dummy_prefix: bool,
    remove_extra_whitespaces: bool,
    escape_whitespaces: bool,
}

struct Pb<'a> {
    b: &'a [u8],
    p: usize,
}

impl Pb<'_> {
    fn varint(&mut self) -> u64 {
        let mut r = 0u64;
        let mut s = 0;
        loop {
            let x = self.b[self.p];
            self.p += 1;
            r |= ((x & 0x7f) as u64) << s;
            if x < 0x80 {
                return r;
            }
            s += 7;
        }
    }
    fn skip(&mut self, wire: u64) {
        match wire {
            0 => {
                self.varint();
            }
            1 => self.p += 8,
            2 => {
                let l = self.varint() as usize;
                self.p += l;
            }
            5 => self.p += 4,
            _ => panic!("bad tokenizer model"),
        }
    }
}

#[derive(PartialEq)]
struct Pair {
    score: f32,
    left: usize,
    right: usize,
    text: String,
}

impl Eq for Pair {}
impl Ord for Pair {
    fn cmp(&self, o: &Self) -> Ordering {
        self.score.partial_cmp(&o.score).unwrap_or(Ordering::Equal).then(o.left.cmp(&self.left))
    }
}
impl PartialOrd for Pair {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl SentencePiece {
    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        let b = std::fs::read(path)?;
        Ok(Self::from_bytes(&b))
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        let mut sp = SentencePiece {
            piece_id: HashMap::new(),
            score: HashMap::new(),
            pieces: Vec::new(),
            unk_id: 0,
            trie: Vec::new(),
            normalized: Vec::new(),
            add_dummy_prefix: true,
            remove_extra_whitespaces: true,
            escape_whitespaces: true,
        };
        let mut r = Pb { b, p: 0 };
        let mut n = 0u32;
        while r.p < b.len() {
            let key = r.varint();
            let field = key >> 3;
            let wire = key & 7;
            if wire != 2 || (field != 1 && field != 3) {
                r.skip(wire);
                continue;
            }
            let len = r.varint() as usize;
            let end = r.p + len;
            if field == 1 {
                let mut piece = String::new();
                let mut sc = 0f32;
                let mut ty = 1u64;
                while r.p < end {
                    let k = r.varint();
                    match k {
                        0x0a => {
                            let l = r.varint() as usize;
                            piece = String::from_utf8_lossy(&b[r.p..r.p + l]).into_owned();
                            r.p += l;
                        }
                        0x15 => {
                            sc = f32::from_le_bytes(b[r.p..r.p + 4].try_into().unwrap());
                            r.p += 4;
                        }
                        0x18 => ty = r.varint(),
                        _ => r.skip(k & 7),
                    }
                }
                if ty == 2 {
                    sp.unk_id = n;
                }
                if ty == 1 || ty == 4 {
                    sp.piece_id.insert(piece.clone(), n);
                    sp.score.insert(piece.clone(), sc);
                }
                sp.pieces.push(piece);
                n += 1;
            } else {
                while r.p < end {
                    let k = r.varint();
                    match k {
                        0x12 => {
                            let l = r.varint() as usize;
                            let cm = &b[r.p..r.p + l];
                            r.p += l;
                            if cm.len() >= 4 {
                                let tsize = u32::from_le_bytes(cm[0..4].try_into().unwrap()) as usize;
                                sp.trie = cm[4..4 + tsize]
                                    .chunks_exact(4)
                                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                                    .collect();
                                sp.normalized = cm[4 + tsize..].to_vec();
                            }
                        }
                        0x18 => sp.add_dummy_prefix = r.varint() != 0,
                        0x20 => sp.remove_extra_whitespaces = r.varint() != 0,
                        0x28 => sp.escape_whitespaces = r.varint() != 0,
                        _ => r.skip(k & 7),
                    }
                }
            }
            r.p = end;
        }
        sp
    }

    pub fn id_to_piece(&self, id: u32) -> &str {
        self.pieces.get(id as usize).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn piece_to_id(&self, p: &str) -> u32 {
        self.piece_id.get(p).copied().unwrap_or(self.unk_id)
    }

    fn normalize_prefix<'a>(&'a self, s: &'a [u8]) -> (&'a [u8], usize) {
        let mut longest = 0usize;
        let mut value = 0usize;
        if !self.trie.is_empty() {
            let a = &self.trie;
            let offset = |u: u32| -> usize { ((u >> 10) << ((u & (1 << 9)) >> 6)) as usize };
            let label = |u: u32| -> u32 { u & ((1u32 << 31) | 0xff) };
            let has_leaf = |u: u32| -> bool { (u >> 8) & 1 == 1 };
            let val = |u: u32| -> usize { (u & ((1u32 << 31) - 1)) as usize };
            let mut pos = 0usize;
            let unit = a[pos];
            pos ^= offset(unit);
            for (i, &k) in s.iter().enumerate() {
                pos ^= k as usize;
                let Some(&unit) = a.get(pos) else { break };
                if label(unit) != k as u32 {
                    break;
                }
                pos ^= offset(unit);
                if has_leaf(unit) {
                    if let Some(&leaf) = a.get(pos) {
                        longest = i + 1;
                        value = val(leaf);
                    }
                }
            }
        }
        if longest == 0 {
            let n = utf8_len(s);
            match std::str::from_utf8(&s[..n.min(s.len())]) {
                Ok(_) if n > 0 && n <= s.len() => (&s[..n], n),
                _ => ("\u{fffd}".as_bytes(), 1),
            }
        } else {
            let tail = &self.normalized[value..];
            let e = tail.iter().position(|&c| c == 0).unwrap_or(tail.len());
            (&tail[..e], longest)
        }
    }

    pub fn normalize(&self, text: &str) -> String {
        const WS: &str = "\u{2581}";
        let mut input = text.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(input.len() + 8);
        if self.remove_extra_whitespaces {
            while !input.is_empty() {
                let (p, n) = self.normalize_prefix(input);
                if p != b" " {
                    break;
                }
                input = &input[n..];
            }
        }
        if input.is_empty() {
            return String::new();
        }
        if self.add_dummy_prefix {
            if self.escape_whitespaces {
                out.extend_from_slice(WS.as_bytes());
            } else {
                out.push(b' ');
            }
        }
        let mut prev_space = self.remove_extra_whitespaces;
        while !input.is_empty() {
            let (mut p, n) = self.normalize_prefix(input);
            if prev_space && self.remove_extra_whitespaces {
                while p.first() == Some(&b' ') {
                    p = &p[1..];
                }
            }
            if !p.is_empty() {
                for &c in p {
                    if self.escape_whitespaces && c == b' ' {
                        out.extend_from_slice(WS.as_bytes());
                    } else {
                        out.push(c);
                    }
                }
                prev_space = p.ends_with(b" ");
            }
            input = &input[n..];
        }
        if self.remove_extra_whitespaces {
            let ws = WS.as_bytes();
            while out.ends_with(ws) {
                out.truncate(out.len() - 3);
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    pub fn encode_pieces(&self, text: &str) -> Vec<String> {
        let norm = self.normalize(text);
        if norm.is_empty() {
            return Vec::new();
        }
        let mut text: Vec<String> = norm.chars().map(|c| c.to_string()).collect();
        let n = text.len();
        let mut prev: Vec<isize> = (0..n as isize).map(|i| i - 1).collect();
        let mut next: Vec<isize> = (0..n as isize).map(|i| if i + 1 < n as isize { i + 1 } else { -1 }).collect();
        let mut agenda = BinaryHeap::new();
        let add = |agenda: &mut BinaryHeap<Pair>, text: &Vec<String>, l: isize, r: isize| {
            if l < 0 || r < 0 {
                return;
            }
            let s = format!("{}{}", text[l as usize], text[r as usize]);
            if let Some(&sc) = self.score.get(&s) {
                agenda.push(Pair { score: sc, left: l as usize, right: r as usize, text: s });
            }
        };
        for i in 0..n.saturating_sub(1) {
            add(&mut agenda, &text, i as isize, i as isize + 1);
        }
        while let Some(top) = agenda.pop() {
            let (l, r) = (top.left, top.right);
            if text[l].is_empty() || text[r].is_empty() || next[l] != r as isize || format!("{}{}", text[l], text[r]) != top.text {
                continue;
            }
            text[l] = top.text;
            text[r] = String::new();
            next[l] = next[r];
            if next[r] >= 0 {
                prev[next[r] as usize] = l as isize;
            }
            add(&mut agenda, &text, prev[l], l as isize);
            add(&mut agenda, &text, l as isize, next[l]);
        }
        let mut out = Vec::new();
        let mut i: isize = 0;
        while i >= 0 && (i as usize) < n {
            if !text[i as usize].is_empty() {
                out.push(std::mem::take(&mut text[i as usize]));
            }
            i = next[i as usize];
        }
        out
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        self.encode_pieces(text).iter().map(|p| self.piece_to_id(p)).collect()
    }

    pub fn decode_pieces(&self, pieces: &[String]) -> String {
        let s: String = pieces.concat().replace('\u{2581}', " ");
        s.strip_prefix(' ').map(|x| x.to_string()).unwrap_or(s)
    }
}

fn utf8_len(s: &[u8]) -> usize {
    let c = s[0];
    if c < 0x80 {
        1
    } else if c >> 5 == 0b110 {
        2
    } else if c >> 4 == 0b1110 {
        3
    } else if c >> 3 == 0b11110 {
        4
    } else {
        0
    }
}
