//! Pinyin syllable alignment for lexicon key matching.

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

fn is_complete_syllable(syl: &str, syllable_table: &[String]) -> bool {
    syllable_table.iter().any(|s| s == syl)
}

/// Whether lexicon key aligns with user composing at syllable boundaries.
pub fn key_matches_composing(key: &str, composing: &str, syllable_table: &[String]) -> bool {
    if composing.is_empty() {
        return false;
    }
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
    if key.starts_with(composing) {
        return is_valid_prefix(composing, syllable_table);
    }
    false
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

    #[test]
    fn key_match_ta_not_tai() {
        let syls = table_ta();
        assert!(key_matches_composing("ta", "ta", &syls));
        assert!(key_matches_composing("tamen", "ta", &syls));
        assert!(!key_matches_composing("tai", "ta", &syls));
        assert!(!key_matches_composing("tang", "ta", &syls));
        assert!(key_matches_composing("ta", "t", &syls));
    }
}
