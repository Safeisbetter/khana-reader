
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub struct SequenceMatcher<'a, T: Eq + Hash> {
    a: &'a [T],
    b: &'a [T],
    b2j: HashMap<&'a T, Vec<usize>>,
    bjunk: HashSet<&'a T>,
    bpopular: HashSet<&'a T>,
    matching: Option<Vec<(usize, usize, usize)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Equal,
    Replace,
    Delete,
    Insert,
}

impl<'a, T: Eq + Hash> SequenceMatcher<'a, T> {
    pub fn new(a: &'a [T], b: &'a [T], autojunk: bool) -> Self {
        let mut b2j: HashMap<&T, Vec<usize>> = HashMap::new();
        for (i, elt) in b.iter().enumerate() {
            b2j.entry(elt).or_default().push(i);
        }
        let bjunk = HashSet::new();
        let mut bpopular = HashSet::new();
        let n = b.len();
        if autojunk && n >= 200 {
            let ntest = n / 100 + 1;
            for (elt, idxs) in b2j.iter() {
                if idxs.len() > ntest {
                    bpopular.insert(*elt);
                }
            }
            for elt in bpopular.iter() {
                b2j.remove(elt);
            }
        }
        SequenceMatcher { a, b, b2j, bjunk, bpopular, matching: None }
    }

    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (a, b) = (self.a, self.b);
        let isbjunk = |x: &T| self.bjunk.contains(x);
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        let nothing: Vec<usize> = Vec::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            for &j in self.b2j.get(&a[i]).unwrap_or(&nothing) {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
                let k = if j > 0 { j2len.get(&(j - 1)).copied().unwrap_or(0) } else { 0 } + 1;
                newj2len.insert(j, k);
                if k > bestsize {
                    besti = i + 1 - k;
                    bestj = j + 1 - k;
                    bestsize = k;
                }
            }
            j2len = newj2len;
        }
        while besti > alo && bestj > blo && !isbjunk(&b[bestj - 1]) && a[besti - 1] == b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && !isbjunk(&b[bestj + bestsize])
            && a[besti + bestsize] == b[bestj + bestsize]
        {
            bestsize += 1;
        }
        while besti > alo && bestj > blo && isbjunk(&b[bestj - 1]) && a[besti - 1] == b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && isbjunk(&b[bestj + bestsize])
            && a[besti + bestsize] == b[bestj + bestsize]
        {
            bestsize += 1;
        }
        let _ = &self.bpopular;
        (besti, bestj, bestsize)
    }

    pub fn get_matching_blocks(&mut self) -> Vec<(usize, usize, usize)> {
        if let Some(m) = &self.matching {
            return m.clone();
        }
        let (la, lb) = (self.a.len(), self.b.len());
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                blocks.push((i, j, k));
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        blocks.sort();
        let (mut i1, mut j1, mut k1) = (0, 0, 0);
        let mut non_adjacent = Vec::new();
        for (i2, j2, k2) in blocks {
            if i1 + k1 == i2 && j1 + k1 == j2 {
                k1 += k2;
            } else {
                if k1 > 0 {
                    non_adjacent.push((i1, j1, k1));
                }
                i1 = i2;
                j1 = j2;
                k1 = k2;
            }
        }
        if k1 > 0 {
            non_adjacent.push((i1, j1, k1));
        }
        non_adjacent.push((la, lb, 0));
        self.matching = Some(non_adjacent.clone());
        non_adjacent
    }

    pub fn get_opcodes(&mut self) -> Vec<(Op, usize, usize, usize, usize)> {
        let (mut i, mut j) = (0, 0);
        let mut answer = Vec::new();
        for (ai, bj, size) in self.get_matching_blocks() {
            let tag = if i < ai && j < bj {
                Some(Op::Replace)
            } else if i < ai {
                Some(Op::Delete)
            } else if j < bj {
                Some(Op::Insert)
            } else {
                None
            };
            if let Some(t) = tag {
                answer.push((t, i, ai, j, bj));
            }
            i = ai + size;
            j = bj + size;
            if size > 0 {
                answer.push((Op::Equal, ai, i, bj, j));
            }
        }
        answer
    }

    pub fn ratio(&mut self) -> f64 {
        let matches: usize = self.get_matching_blocks().iter().map(|t| t.2).sum();
        let len = self.a.len() + self.b.len();
        if len > 0 { 2.0 * matches as f64 / len as f64 } else { 1.0 }
    }
}

pub fn ratio_str(a: &str, b: &str, autojunk: bool) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    SequenceMatcher::new(&a, &b, autojunk).ratio()
}

