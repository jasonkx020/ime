//! Preprocess stage: normalize composing before Seg/Gen.
//! See docs/PINYIN_SEGMENTATION.md — classic pipeline.

use crate::pinyin_match::{is_valid_pinyin_input, mixed_slots, MixSlot};

/// Input shape after normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputShape {
    QuanPin,
    JianPin,
    Mixed,
}

/// Output of the preprocess stage.
#[derive(Debug, Clone)]
pub struct PreprocessOut {
    /// Lowercased key with apostrophes stripped (DAT lookup key).
    pub key: String,
    pub shape: InputShape,
    /// Byte offsets in `key` that must be syllable boundaries (from `'`, `’`, `-`).
    pub hard_breaks: Vec<usize>,
}

/// Normalize pinyin composing for traditional lookup.
///
/// - trim, lowercase, `ü`/`Ü` → `v`
/// - strip internal whitespace
/// - strip trailing non-letters (e.g. `nihao123` → `nihao`)
/// - apostrophe / dash → hard syllable breaks (removed from key)
pub fn preprocess_pinyin(raw: &str, syllables: &[String]) -> PreprocessOut {
    let mut hard_breaks = Vec::new();
    let mut key = String::with_capacity(raw.len());
    let mut trailing = String::new();
    let mut in_trailing = false;

    for c in raw.chars() {
        let c = match c {
            'ü' | 'Ü' => 'v',
            other => other,
        };
        if c.is_whitespace() {
            continue;
        }
        if matches!(c, '\'' | '\u{2019}' | '-') {
            if !key.is_empty() {
                let br = key.len();
                if hard_breaks.last() != Some(&br) {
                    hard_breaks.push(br);
                }
            }
            in_trailing = false;
            trailing.clear();
            continue;
        }
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_lowercase() {
            if in_trailing {
                // Digit/symbol then letter again: keep letter path only if we
                // haven't committed trailing; treat mid-string digits as strip.
                key.push_str(&trailing.chars().filter(|x| x.is_ascii_lowercase()).collect::<String>());
                trailing.clear();
                in_trailing = false;
            }
            key.push(lower);
        } else if key.is_empty() {
            continue;
        } else {
            in_trailing = true;
            // Drop from key: trailing non-letters are stripped for lookup.
        }
    }
    let _ = trailing;

    hard_breaks.retain(|&b| b > 0 && b < key.len());
    hard_breaks.sort_unstable();
    hard_breaks.dedup();

    let shape = classify_shape(&key, syllables);
    PreprocessOut {
        key,
        shape,
        hard_breaks,
    }
}

fn classify_shape(key: &str, syllables: &[String]) -> InputShape {
    if key.is_empty() {
        return InputShape::QuanPin;
    }
    if syllables.is_empty() {
        return InputShape::Mixed;
    }
    // Full syllable cover → QuanPin
    if is_valid_pinyin_input(key, syllables) && fully_syllable_covered(key, syllables) {
        return InputShape::QuanPin;
    }
    let slots = mixed_slots(key, syllables);
    if slots.is_empty() {
        return InputShape::Mixed;
    }
    let all_initial = slots.iter().all(|s| matches!(s, MixSlot::Initial(_)));
    if all_initial {
        return InputShape::JianPin;
    }
    let has_syl = slots.iter().any(|s| matches!(s, MixSlot::Syl(_)));
    let has_init = slots.iter().any(|s| matches!(s, MixSlot::Initial(_)));
    if has_syl && has_init {
        InputShape::Mixed
    } else if has_syl {
        InputShape::QuanPin
    } else {
        InputShape::JianPin
    }
}

fn fully_syllable_covered(input: &str, table: &[String]) -> bool {
    let n = input.len();
    if n == 0 {
        return false;
    }
    let mut jumps = vec![Vec::new(); n + 1];
    for i in 0..n {
        let rest = &input[i..];
        for syl in table {
            if !syl.is_empty() && rest.starts_with(syl.as_str()) {
                jumps[i].push(i + syl.len());
            }
        }
    }
    let mut reach = vec![false; n + 1];
    reach[0] = true;
    for i in 0..n {
        if !reach[i] {
            continue;
        }
        for &j in &jumps[i] {
            reach[j] = true;
        }
    }
    reach[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_and_ue_to_v() {
        let syls = vec!["nv".into(), "hao".into()];
        let out = preprocess_pinyin("NüHao", &syls);
        assert_eq!(out.key, "nvhao");
    }

    #[test]
    fn apostrophe_hard_break() {
        let syls = vec!["xi".into(), "an".into()];
        let out = preprocess_pinyin("xi'an", &syls);
        assert_eq!(out.key, "xian");
        assert_eq!(out.hard_breaks, vec![2]);
    }

    #[test]
    fn strip_trailing_digits() {
        let syls = vec!["ni".into(), "hao".into()];
        let out = preprocess_pinyin("nihao123", &syls);
        assert_eq!(out.key, "nihao");
    }

    #[test]
    fn jianpin_shape() {
        let syls = vec!["ni".into(), "hao".into()];
        let out = preprocess_pinyin("nh", &syls);
        assert_eq!(out.shape, InputShape::JianPin);
    }
}
