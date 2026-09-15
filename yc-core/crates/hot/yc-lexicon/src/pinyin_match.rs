//! Pinyin syllable alignment for lexicon key matching (full + jianpin).

/// Greedy longest-match syllable split over `input`.
pub fn split_syllables<'a>(input: &'a str, syllable_table: &[String]) -> Vec<&'a str> {
    let input = input.trim();
    if input.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let mut best: Option<usize> = None;
        for syl in syllable_table {
            if rest.starts_with(syl.as_str()) {
                let len = syl.len();
                if best.map_or(true, |b| len > b) {
                    best = Some(len);
                }
            }
        }
        let Some(len) = best else {
            break;
        };
        result.push(&input[i..i + len]);
        i += len;
    }
    result
}

/// True when `input` is empty, all complete syllables, or ends on a syllable prefix (`n`/`ni`/…).
pub fn is_valid_prefix(input: &str, syllable_table: &[String]) -> bool {
    let input = input.trim();
    if input.is_empty() {
        return true;
    }
    if !input.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let mut best: Option<usize> = None;
        for syl in syllable_table {
            if rest.starts_with(syl.as_str()) {
                let len = syl.len();
                if best.map_or(true, |b| len > b) {
                    best = Some(len);
                }
            }
        }
        if let Some(len) = best {
            i += len;
        } else {
            return syllable_table.iter().any(|s| s.starts_with(rest));
        }
    }
    true
}

/// Accept full-pinyin prefixes **or** jianpin sequences (`nh`, `bj`, `nih`, …).
pub fn is_valid_pinyin_input(input: &str, syllable_table: &[String]) -> bool {
    if is_valid_prefix(input, syllable_table) {
        return true;
    }
    is_valid_jianpin_prefix(input, syllable_table)
}

/// Whether `input` can be read as jianpin / mixed (initials + syllable pieces).
pub fn is_valid_jianpin_prefix(input: &str, syllable_table: &[String]) -> bool {
    let input = input.trim();
    if input.is_empty() {
        return true;
    }
    if !input.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    jianpin_consume_ok(input, syllable_table)
}

fn jianpin_consume_ok(rest: &str, table: &[String]) -> bool {
    if rest.is_empty() {
        return true;
    }
    // 1) Full syllable
    let mut tried = false;
    for syl in table {
        if rest.starts_with(syl.as_str()) {
            tried = true;
            if jianpin_consume_ok(&rest[syl.len()..], table) {
                return true;
            }
        }
    }
    // 2) Ends on / continues after a proper syllable prefix (not full syl)
    for syl in table {
        let max = syl.len().min(rest.len());
        for len in (1..max).rev() {
            if syl.as_str().starts_with(&rest[..len]) {
                tried = true;
                let next = &rest[len..];
                if next.is_empty() {
                    return true;
                }
                // Mid-string partial syllable only if remainder can start a new segment
                if jianpin_consume_ok(next, table) {
                    return true;
                }
            }
        }
    }
    // 3) Single initial of some syllable
    let ch = rest.as_bytes()[0];
    if table.iter().any(|s| s.as_bytes().first() == Some(&ch)) {
        tried = true;
        if jianpin_consume_ok(&rest[1..], table) {
            return true;
        }
    }
    let _ = tried;
    false
}

/// Whether `syl` is an exact entry in the syllable table (e.g. `tao`, not `t` / `nihao`).
pub fn is_complete_syllable(syl: &str, syllable_table: &[String]) -> bool {
    syllable_table.iter().any(|s| s == syl)
}

/// Full-pinyin alignment (existing boundary rules) OR jianpin (`nh` ↔ `nihao`).
pub fn key_matches_composing(key: &str, composing: &str, syllable_table: &[String]) -> bool {
    if composing.is_empty() {
        return false;
    }
    if key_matches_full_pinyin(key, composing, syllable_table) {
        return true;
    }
    key_matches_jianpin(key, composing, syllable_table)
}

fn key_matches_full_pinyin(key: &str, composing: &str, syllable_table: &[String]) -> bool {
    if key == composing {
        return true;
    }
    if !key.starts_with(composing) {
        return false;
    }
    let key_syls = split_syllables(key, syllable_table);
    if !key_syls.is_empty() {
        let mut acc = String::new();
        for syl in &key_syls {
            acc.push_str(syl);
            if acc == composing {
                return true;
            }
            if acc.len() > composing.len() {
                break;
            }
        }
    }
    if is_complete_syllable(composing, syllable_table) {
        return false;
    }
    is_valid_prefix(composing, syllable_table)
}

/// Match composing against key syllables via full / prefix / initial per syllable.
pub fn key_matches_jianpin(key: &str, composing: &str, syllable_table: &[String]) -> bool {
    let composing = composing.trim();
    if composing.is_empty() || key.is_empty() {
        return false;
    }
    // Fast reject before syllable split.
    if key.as_bytes()[0] != composing.as_bytes()[0] {
        return false;
    }
    if key.len() < composing.len() {
        return false;
    }
    let key_syls = split_syllables(key, syllable_table);
    if key_syls.is_empty() {
        return false;
    }
    align_jianpin(composing, &key_syls, syllable_table)
}

fn align_jianpin(composing: &str, key_syls: &[&str], table: &[String]) -> bool {
    fn go(c: &str, syls: &[&str], table: &[String]) -> bool {
        if c.is_empty() {
            return true;
        }
        if syls.is_empty() {
            return false;
        }
        let syl = syls[0];
        let rest_syls = &syls[1..];

        // Prefer full syllable
        if c.starts_with(syl) {
            if go(&c[syl.len()..], rest_syls, table) {
                return true;
            }
        }

        // Syllable prefix (progressive typing within this syllable)
        let max = syl.len().min(c.len());
        for len in (1..=max).rev() {
            if len == syl.len() {
                continue; // already tried full
            }
            if !syl.starts_with(&c[..len]) {
                continue;
            }
            // Reject: composing is exactly a shorter complete syllable (ta ↛ tai)
            if c.len() == len && is_complete_syllable(&c[..len], table) {
                continue;
            }
            if go(&c[len..], rest_syls, table) {
                return true;
            }
        }

        // Jianpin initial
        if c.as_bytes()[0] == syl.as_bytes()[0] {
            // Single-char composing that is a complete syllable of another reading: still OK as initial
            // only when matching as initial of this syl (e.g. "n" → "ni")
            if go(&c[1..], rest_syls, table) {
                return true;
            }
        }
        false
    }
    go(composing, key_syls, table)
}

/// True when lookup should also scan non-prefix keys for jianpin hits.
pub fn needs_jianpin_scan(composing: &str, syllable_table: &[String]) -> bool {
    let composing = composing.trim();
    if composing.len() < 2 {
        return false;
    }
    // Pure full-pinyin progressive input can rely on DAT prefix scan alone.
    if is_valid_prefix(composing, syllable_table) && !looks_like_jianpin(composing, syllable_table)
    {
        return false;
    }
    true
}

/// Heuristic: after some full syllables, remaining looks like initials / short pieces.
fn looks_like_jianpin(composing: &str, table: &[String]) -> bool {
    // If full prefix parse leaves nothing, not jianpin-only.
    if is_valid_prefix(composing, table) {
        // Still may be mixed like "nih" (ni + h) — valid full prefix path ends with syllable prefix "h" of hao?
        // "nih": is_valid_prefix — ni matched, rem "h", "h" is prefix of hao/he/... → true.
        // So is_valid_prefix("nih") is true! Then needs_jianpin_scan would be false.
        // But "nih" is found via prefix scan of keys starting with "nih" — "nihao".starts_with("nih") ✓
        // "nh": is_valid_prefix false → needs jianpin scan. Good.
        return false;
    }
    is_valid_jianpin_prefix(composing, table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_ta() -> Vec<String> {
        vec![
            "ta".into(),
            "tai".into(),
            "tan".into(),
            "tang".into(),
            "men".into(),
        ]
    }

    fn table_nihao() -> Vec<String> {
        vec![
            "ni".into(),
            "hao".into(),
            "bei".into(),
            "jing".into(),
            "tao".into(),
            "tai".into(),
            "guang".into(),
            "he".into(),
            "hen".into(),
            "heng".into(),
            "hu".into(),
            "hua".into(),
            "hai".into(),
            "han".into(),
            "hang".into(),
        ]
    }

    #[test]
    fn key_match_ta_not_tai() {
        let syls = table_ta();
        assert!(key_matches_composing("ta", "ta", &syls));
        assert!(key_matches_composing("tamen", "ta", &syls));
        assert!(!key_matches_composing("tai", "ta", &syls));
        assert!(!key_matches_composing("tang", "ta", &syls));
        assert!(key_matches_composing("ta", "t", &syls));
    }

    #[test]
    fn complete_syllable_tao() {
        let syls = vec![
            "tao".into(),
            "tai".into(),
            "guang".into(),
            "ni".into(),
            "hao".into(),
        ];
        assert!(is_complete_syllable("tao", &syls));
        assert!(is_complete_syllable("tai", &syls));
        assert!(!is_complete_syllable("t", &syls));
        assert!(!is_complete_syllable("nihao", &syls));
    }

    #[test]
    fn jianpin_nh_matches_nihao() {
        let syls = table_nihao();
        assert!(is_valid_jianpin_prefix("nh", &syls));
        assert!(is_valid_pinyin_input("nh", &syls));
        assert!(!is_valid_prefix("nh", &syls));
        assert!(key_matches_jianpin("nihao", "nh", &syls));
        assert!(key_matches_composing("nihao", "nh", &syls));
        assert!(key_matches_composing("nihao", "nih", &syls));
        assert!(key_matches_composing("nihao", "nihao", &syls));
    }

    #[test]
    fn jianpin_bj_matches_beijing() {
        let syls = table_nihao();
        assert!(key_matches_jianpin("beijing", "bj", &syls));
        assert!(key_matches_jianpin("beijing", "bjing", &syls));
        assert!(is_valid_pinyin_input("bj", &syls));
    }

    #[test]
    fn jianpin_does_not_break_ta_tai() {
        let syls = table_ta();
        assert!(!key_matches_jianpin("tai", "ta", &syls));
        assert!(!key_matches_composing("tai", "ta", &syls));
    }
}
