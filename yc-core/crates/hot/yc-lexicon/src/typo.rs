//! QWERTY adjacent-key typo variants for pinyin composing correction.

use std::collections::HashSet;

/// Max adjacent-substitution variants per composing string.
const VARIANT_CAP: usize = 24;

/// Minimum composing length before generating typo variants (avoid noise on single letter).
const MIN_LEN: usize = 2;

/// QWERTY letter adjacency (lowercase), aligned with pinyin26 letter rows.
fn neighbors(ch: u8) -> &'static [u8] {
    match ch {
        b'q' => b"wa",
        b'w' => b"qeas",
        b'e' => b"wsdr",
        b'r' => b"edft",
        b't' => b"rfgy",
        b'y' => b"tghu",
        b'u' => b"yhji",
        b'i' => b"ujko",
        b'o' => b"iklp",
        b'p' => b"ol",
        b'a' => b"qwsz",
        b's' => b"awedxz",
        b'd' => b"serfcx",
        b'f' => b"drtgvc",
        b'g' => b"ftyhbv",
        b'h' => b"gyujnb",
        b'j' => b"huiknm",
        b'k' => b"jiolm",
        b'l' => b"kop",
        b'z' => b"asx",
        b'x' => b"zsdc",
        b'c' => b"xdfv",
        b'v' => b"cfgb",
        b'b' => b"vghn",
        b'n' => b"bhjm",
        b'm' => b"njk",
        _ => b"",
    }
}

/// Generate single-edit adjacent-key substitutions of `composing` (lowercase a-z).
/// Does **not** include the original string. Empty if `composing.len() < 2`.
pub fn adjacent_typo_variants(composing: &str) -> Vec<String> {
    let s = composing.trim().to_ascii_lowercase();
    if s.len() < MIN_LEN || !s.bytes().all(|b| b.is_ascii_lowercase()) {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    seen.insert(s.clone());

    for i in 0..bytes.len() {
        let ch = bytes[i];
        for &alt in neighbors(ch) {
            if alt == ch {
                continue;
            }
            let mut v = bytes.to_vec();
            v[i] = alt;
            let candidate = String::from_utf8(v).unwrap_or_default();
            if candidate.is_empty() || !seen.insert(candidate.clone()) {
                continue;
            }
            out.push(candidate);
            if out.len() >= VARIANT_CAP {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n_neighbors_include_m() {
        assert!(neighbors(b'n').contains(&b'm'));
        assert!(neighbors(b'm').contains(&b'n'));
    }

    #[test]
    fn wn_variants_include_wm() {
        let vs = adjacent_typo_variants("wn");
        assert!(vs.contains(&"wm".to_string()), "got {:?}", vs);
        assert!(!vs.contains(&"wn".to_string()));
    }

    #[test]
    fn short_composing_no_variants() {
        assert!(adjacent_typo_variants("w").is_empty());
        assert!(adjacent_typo_variants("").is_empty());
    }
}
