//! Correction tables (LangPack/layout data) + edit1 / fuzzy / typo variant gen.
//! Algorithms live here; tables load from pack or [`CorrectionTables::builtin_fallback`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;

/// Process-wide correction tables (updated on enable / switch_layout).
#[derive(Debug, Clone)]
pub struct SharedCorrectionTables {
    inner: Arc<Mutex<Arc<CorrectionTables>>>,
}

impl Default for SharedCorrectionTables {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedCorrectionTables {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Arc::new(CorrectionTables::builtin_fallback()))),
        }
    }

    pub fn publish(&self, tables: CorrectionTables) {
        *self.inner.lock() = Arc::new(tables);
    }

    pub fn set_neighbors(&self, neighbors: HashMap<char, Vec<char>>) {
        let mut g = self.inner.lock();
        let mut t = (**g).clone();
        t.set_neighbors(neighbors);
        *g = Arc::new(t);
    }

    pub fn get(&self) -> Arc<CorrectionTables> {
        self.inner.lock().clone()
    }
}

/// In-memory correction data for the hot path (no YAML).
#[derive(Debug, Clone)]
pub struct CorrectionTables {
    pub initial_pairs: Vec<(String, String)>,
    pub final_pairs: Vec<(String, String)>,
    /// After these initials, u↔v is allowed (lü/nv).
    pub uv_after: Vec<char>,
    /// Layout-scoped keyboard neighbors.
    pub neighbors: HashMap<char, Vec<char>>,
}

impl CorrectionTables {
    /// QWERTY-26 + default Mandarin fuzzy (matches product KEYBOARD_NEIGHBORS).
    pub fn builtin_fallback() -> Self {
        let mut neighbors = HashMap::new();
        for (k, vs) in BUILTIN_NEIGHBORS {
            neighbors.insert(
                *k,
                vs.chars().collect(),
            );
        }
        Self {
            initial_pairs: vec![
                ("z".into(), "zh".into()),
                ("c".into(), "ch".into()),
                ("s".into(), "sh".into()),
                ("n".into(), "l".into()),
                ("f".into(), "h".into()),
            ],
            final_pairs: vec![
                ("an".into(), "ang".into()),
                ("en".into(), "eng".into()),
                ("in".into(), "ing".into()),
            ],
            uv_after: vec!['l', 'n'],
            neighbors,
        }
    }

    /// Parse a minimal fuzzy YAML subset (no full YAML crate required).
    pub fn from_fuzzy_yaml(text: &str) -> Self {
        let mut t = Self::builtin_fallback();
        // Keep neighbors from builtin; only override pairs if present.
        let mut initial = Vec::new();
        let mut finals = Vec::new();
        let mut uv = Vec::new();
        let mut section = "";
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("initial_pairs:") {
                section = "initial";
                continue;
            }
            if line.starts_with("final_pairs:") {
                section = "final";
                continue;
            }
            if line.starts_with("uv_after:") {
                section = "uv";
                if let Some(rest) = line.strip_prefix("uv_after:") {
                    parse_char_list(rest, &mut uv);
                }
                continue;
            }
            if line.starts_with('-') {
                let rest = line.trim_start_matches('-').trim();
                match section {
                    "initial" | "final" => {
                        if let Some((a, b)) = parse_pair(rest) {
                            if section == "initial" {
                                initial.push((a, b));
                            } else {
                                finals.push((a, b));
                            }
                        }
                    }
                    "uv" => parse_char_list(rest, &mut uv),
                    _ => {}
                }
            }
        }
        if !initial.is_empty() {
            t.initial_pairs = initial;
        }
        if !finals.is_empty() {
            t.final_pairs = finals;
        }
        if !uv.is_empty() {
            t.uv_after = uv;
        }
        t
    }

    /// Parse `keyboard_neighbors:` block from a layout YAML excerpt.
    pub fn with_neighbors_yaml(mut self, text: &str) -> Self {
        let mut map = HashMap::new();
        let mut in_block = false;
        for line in text.lines() {
            let raw = line;
            let line = line.trim();
            if line.starts_with("keyboard_neighbors:") {
                in_block = true;
                continue;
            }
            if !in_block {
                continue;
            }
            // Leave block on next top-level key (no indent).
            if !raw.is_empty()
                && !raw.starts_with(' ')
                && !raw.starts_with('\t')
                && line.contains(':')
                && !line.starts_with('#')
            {
                break;
            }
            if let Some((k, rest)) = line.split_once(':') {
                let k = k.trim();
                if k.len() == 1 {
                    let ch = k.chars().next().unwrap();
                    let mut vals = Vec::new();
                    parse_char_list(rest, &mut vals);
                    if !vals.is_empty() {
                        map.insert(ch, vals);
                    }
                }
            }
        }
        if !map.is_empty() {
            self.neighbors = map;
        }
        self
    }

    pub fn set_neighbors(&mut self, neighbors: HashMap<char, Vec<char>>) {
        if !neighbors.is_empty() {
            self.neighbors = neighbors;
        }
    }
}

fn parse_pair(s: &str) -> Option<(String, String)> {
    // [z, zh] or ["z", "zh"]
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    let parts: Vec<&str> = s.split(',').map(|x| x.trim().trim_matches('"').trim_matches('\'')).collect();
    if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

fn parse_char_list(s: &str, out: &mut Vec<char>) {
    for tok in s.split(|c: char| c == ',' || c == '[' || c == ']' || c.is_whitespace()) {
        let tok = tok.trim().trim_matches('"').trim_matches('\'');
        if tok.len() == 1 {
            if let Some(ch) = tok.chars().next() {
                if ch.is_ascii_lowercase() {
                    out.push(ch);
                }
            }
        }
    }
}

/// Product KEYBOARD_NEIGHBORS for layout_pinyin26 / builtin fallback.
const BUILTIN_NEIGHBORS: &[(char, &str)] = &[
    ('q', "was"),
    ('w', "qeasd"),
    ('e', "wrsdf"),
    ('r', "etdfg"),
    ('t', "ryfgh"),
    ('y', "tughj"),
    ('u', "yihjk"),
    ('i', "uojkl"),
    ('o', "ipkl"),
    ('p', "ol"),
    ('a', "qwszx"),
    ('s', "qweadzxc"),
    ('d', "wersfxcv"),
    ('f', "ertdgcvb"),
    ('g', "rtyfhvbn"),
    ('h', "tyugjbnm"),
    ('j', "yuihknm"),
    ('k', "uiojlm"),
    ('l', "iopk"),
    ('z', "asx"),
    ('x', "asdzc"),
    ('c', "sdfxv"),
    ('v', "dfgcb"),
    ('b', "fghvn"),
    ('n', "ghjbm"),
    ('m', "hjkn"),
];

/// Cap for correction score so they sort below normal L3 band.
pub const CORRECTION_SCORE_CAP: f32 = 0.74;

/// Fuzzy pronunciation variants using tables.
pub fn fuzzy_variants_with(composing: &str, tables: &CorrectionTables, cap: usize) -> Vec<String> {
    if cap == 0 {
        return Vec::new();
    }
    let s = composing.trim().to_ascii_lowercase();
    if s.len() < 4 || !s.bytes().all(|b| b.is_ascii_lowercase()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(s.clone());
    let mut push = |v: Option<String>| -> bool {
        let Some(v) = v else {
            return false;
        };
        if v.is_empty() || !seen.insert(v.clone()) {
            return false;
        }
        out.push(v);
        out.len() >= cap
    };

    // Expand / contract initials from pairs.
    for (a, b) in &tables.initial_pairs {
        if a.len() < b.len() {
            if push(replace_first(&s, a, b)) {
                return out;
            }
            if push(replace_first(&s, b, a)) {
                return out;
            }
        } else {
            if push(replace_first(&s, b, a)) {
                return out;
            }
            if push(replace_first(&s, a, b)) {
                return out;
            }
        }
    }
    for (a, b) in &tables.final_pairs {
        if push(swap_final_safe(&s, a, b)) {
            return out;
        }
        if push(swap_final_safe(&s, b, a)) {
            return out;
        }
    }
    // u↔v after l/n
    if push(uv_swap(&s, &tables.uv_after)) {
        return out;
    }
    out
}

fn replace_first(s: &str, from: &str, to: &str) -> Option<String> {
    if from.is_empty() || from == to {
        return None;
    }
    let i = s.find(from)?;
    // Avoid expanding z that is already zh
    if from.len() == 1 && matches!(from.as_bytes()[0], b'z' | b'c' | b's') {
        if s.as_bytes().get(i + 1) == Some(&b'h') {
            return None;
        }
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push_str(&s[..i]);
    out.push_str(to);
    out.push_str(&s[i + from.len()..]);
    Some(out)
}

fn swap_final_safe(s: &str, from: &str, to: &str) -> Option<String> {
    let mut start = 0;
    while let Some(rel) = s[start..].find(from) {
        let abs = start + rel;
        let after = abs + from.len();
        if matches!(from, "an" | "en" | "in") && s.as_bytes().get(after) == Some(&b'g') {
            start = abs + 1;
            continue;
        }
        let mut out = String::with_capacity(s.len() + 1);
        out.push_str(&s[..abs]);
        out.push_str(to);
        out.push_str(&s[after..]);
        return Some(out);
    }
    None
}

fn uv_swap(s: &str, after: &[char]) -> Option<String> {
    let b = s.as_bytes();
    for i in 0..b.len() {
        let ch = b[i] as char;
        if ch != 'u' && ch != 'v' {
            continue;
        }
        let prev_ok = i > 0 && after.contains(&(b[i - 1] as char));
        if !prev_ok {
            continue;
        }
        let mut out = b.to_vec();
        out[i] = if ch == 'u' { b'v' } else { b'u' };
        return String::from_utf8(out).ok();
    }
    None
}

/// Adjacent-key typo variants using layout neighbor table.
pub fn adjacent_typo_variants_with(
    composing: &str,
    tables: &CorrectionTables,
    cap: usize,
) -> Vec<String> {
    let s = composing.trim().to_ascii_lowercase();
    if s.len() < 2 || !s.bytes().all(|b| b.is_ascii_lowercase()) || cap == 0 {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(s.clone());
    for i in 0..bytes.len() {
        let ch = bytes[i] as char;
        let Some(alts) = tables.neighbors.get(&ch) else {
            continue;
        };
        for &alt in alts {
            if alt == ch {
                continue;
            }
            let mut v = bytes.to_vec();
            v[i] = alt as u8;
            let candidate = match String::from_utf8(v) {
                Ok(x) => x,
                Err(_) => continue,
            };
            if !seen.insert(candidate.clone()) {
                continue;
            }
            out.push(candidate);
            if out.len() >= cap {
                return out;
            }
        }
    }
    out
}

/// 1-edit insert / delete / transpose (漏字/多字/交换). Tail edits preferred.
/// Minimum length 3 to avoid `ta`→`tai` style noise on complete short syllables.
pub fn edit1_variants(composing: &str, cap: usize) -> Vec<String> {
    let s = composing.trim().to_ascii_lowercase();
    if s.len() < 3 || !s.bytes().all(|b| b.is_ascii_lowercase()) || cap == 0 {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(s.clone());

    let mut push = |v: String| -> bool {
        if v.is_empty() || v == s || !seen.insert(v.clone()) {
            return false;
        }
        out.push(v);
        out.len() >= cap
    };

    // Delete one char (prefer from end).
    for i in (0..n).rev() {
        let mut v = Vec::with_capacity(n - 1);
        v.extend_from_slice(&bytes[..i]);
        v.extend_from_slice(&bytes[i + 1..]);
        if let Ok(cand) = String::from_utf8(v) {
            if push(cand) {
                return out;
            }
        }
    }

    // Insert syllable-forming letters at every position first (漏字: nhao→nihao).
    let early_insert = b"aeioungh";
    for i in 0..=n {
        for &ch in early_insert {
            let mut v = Vec::with_capacity(n + 1);
            v.extend_from_slice(&bytes[..i]);
            v.push(ch);
            v.extend_from_slice(&bytes[i..]);
            if let Ok(cand) = String::from_utf8(v) {
                if push(cand) {
                    return out;
                }
            }
        }
    }

    // Insert remaining letters at end / before last.
    for ch in b'a'..=b'z' {
        if early_insert.contains(&ch) {
            continue;
        }
        let mut v = bytes.to_vec();
        v.push(ch);
        if let Ok(cand) = String::from_utf8(v) {
            if push(cand) {
                return out;
            }
        }
    }

    // Adjacent transpose
    for i in (0..n.saturating_sub(1)).rev() {
        let mut v = bytes.to_vec();
        v.swap(i, i + 1);
        if let Ok(cand) = String::from_utf8(v) {
            if push(cand) {
                return out;
            }
        }
    }
    out
}

/// All correction variant keys for a composing string (fuzzy + typo + edit1).
pub fn correction_variants(
    composing: &str,
    tables: &CorrectionTables,
    fuzzy_cap: usize,
    typo_cap: usize,
    edit_cap: usize,
) -> Vec<(String, u8)> {
    // class: 1 fuzzy, 2 typo, 3 edit1
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(composing.to_ascii_lowercase());
    for v in fuzzy_variants_with(composing, tables, fuzzy_cap) {
        if seen.insert(v.clone()) {
            out.push((v, 1));
        }
    }
    for v in adjacent_typo_variants_with(composing, tables, typo_cap) {
        if seen.insert(v.clone()) {
            out.push((v, 2));
        }
    }
    for v in edit1_variants(composing, edit_cap) {
        if seen.insert(v.clone()) {
            out.push((v, 3));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nuhao_typo_includes_nihao() {
        let t = CorrectionTables::builtin_fallback();
        let vs = adjacent_typo_variants_with("nuhao", &t, 48);
        assert!(vs.contains(&"nihao".into()), "got {vs:?}");
    }

    #[test]
    fn nhao_edit1_includes_nihao() {
        let vs = edit1_variants("nhao", 64);
        assert!(vs.contains(&"nihao".into()), "got {vs:?}");
    }

    #[test]
    fn zongguo_fuzzy_includes_zhongguo() {
        let t = CorrectionTables::builtin_fallback();
        let vs = fuzzy_variants_with("zongguo", &t, 4);
        assert!(vs.contains(&"zhongguo".into()), "got {vs:?}");
    }

    #[test]
    fn neighbors_from_yaml_block() {
        let yaml = "\
keyboard_neighbors:
  n: [g, h, j, b, m]
  m: [h, j, k, n]
rows:
  - []
";
        let t = CorrectionTables::builtin_fallback().with_neighbors_yaml(yaml);
        assert!(t.neighbors.get(&'n').unwrap().contains(&'m'));
    }
}
