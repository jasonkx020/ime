//! One-edit fuzzy pinyin: zh/z, ch/c, sh/s and nasal finals.
//! Variants are capped by the caller so the first screen does not scan the whole table.

use std::collections::HashSet;

/// Fuzzy rewrites of `composing`, most useful first. Does not include the original.
/// Empty when `composing` is shorter than 4 letters or `cap` is 0.
pub fn fuzzy_variants(composing: &str, cap: usize) -> Vec<String> {
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
    if push(expand_initial(&s)) {
        return out;
    }
    if push(contract_initial(&s)) {
        return out;
    }
    for (from, to) in [
        ("ang", "an"),
        ("eng", "en"),
        ("ing", "in"),
        ("an", "ang"),
        ("en", "eng"),
        ("in", "ing"),
    ] {
        if push(swap_final(&s, from, to)) {
            return out;
        }
    }
    out
}

/// First bare z/c/s (not already zh/ch/sh) becomes zh/ch/sh. `zongguo` → `zhongguo`.
fn expand_initial(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if matches!(c, b'z' | b'c' | b's') {
            if i + 1 < b.len() && b[i + 1] == b'h' {
                i += 2;
                continue;
            }
            let mut out = String::with_capacity(s.len() + 1);
            out.push_str(&s[..i]);
            out.push(c as char);
            out.push('h');
            out.push_str(&s[i + 1..]);
            return Some(out);
        }
        i += 1;
    }
    None
}

fn contract_initial(s: &str) -> Option<String> {
    for (from, to) in [("zh", "z"), ("ch", "c"), ("sh", "s")] {
        if let Some(i) = s.find(from) {
            let mut out = String::with_capacity(s.len() - 1);
            out.push_str(&s[..i]);
            out.push_str(to);
            out.push_str(&s[i + from.len()..]);
            return Some(out);
        }
    }
    None
}

fn swap_final(s: &str, from: &str, to: &str) -> Option<String> {
    let mut start = 0;
    while let Some(rel) = s[start..].find(from) {
        let abs = start + rel;
        let after = abs + from.len();
        // `an` inside `ang` is not a separate final.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zongguo_expands_to_zhongguo_first() {
        let vs = fuzzy_variants("zongguo", 1);
        assert_eq!(vs, vec!["zhongguo".to_string()]);
    }

    #[test]
    fn short_input_has_no_fuzzy() {
        assert!(fuzzy_variants("zho", 4).is_empty());
        assert!(fuzzy_variants("ni", 4).is_empty());
    }
}
