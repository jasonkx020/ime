//! Unified letter→word span resolution (see docs/PINYIN_SEGMENTATION.md).
//!
//! `code_len` is always relative to the **user's original composing** string.

use yc_types::Candidate;

use crate::dat::DatLexicon;
use crate::segment::{first_syllable_len, syllable_prefix_ends};

/// Syllable-pair fuzzy variants used when looking up a span key (R2 multi-hypothesis).
/// Centralized — do not scatter `chang↔chuang` special cases elsewhere.
pub const SYL_PAIR_VARIANTS: &[(&str, &str)] = &[("chang", "chuang"), ("chuang", "chang")];

/// Extra keys to try for a pinyin span (exact + pair variants + fuzzy rewrites).
pub fn span_key_variants(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |s: String| {
        if seen.insert(s.clone()) {
            out.push(s);
        }
    };
    push(key.to_string());
    for (a, b) in SYL_PAIR_VARIANTS {
        if key == *a {
            push((*b).into());
        } else if key.ends_with(a) && !key.ends_with(b) {
            let base = &key[..key.len() - a.len()];
            push(format!("{base}{b}"));
        }
    }
    for v in crate::fuzzy::fuzzy_variants(key, 2) {
        push(v);
    }
    out
}

/// Map a consume length measured on `variant` back onto `original` (R6).
///
/// - Same length (typo / most fuzzy): keep `var_span`.
/// - Expand initial (`z`→`zh`): original usually shorter by 1 for that edit.
/// - Contract initial (`zh`→`z`): original usually longer by 1.
/// - Clamped to `original.len()`.
pub fn map_variant_span_to_original(original: &str, variant: &str, var_span: u32) -> u32 {
    let o = original.len() as u32;
    let v = variant.len() as u32;
    if o == 0 {
        return 0;
    }
    if var_span == 0 {
        return 0;
    }
    if o == v {
        return var_span.min(o);
    }
    // Length-delta edits: scale prefix by length difference at the edit site.
    // Prefer: original_span ≈ var_span + (o - v) when the edit is in the consumed prefix.
    let delta = o as i32 - v as i32;
    let mapped = (var_span as i32 + delta).clamp(1, o as i32) as u32;
    // If variant is longer (expand), consuming full variant → consume full original.
    if var_span >= v {
        return o;
    }
    mapped.min(o)
}

/// Resolve consume length for `text` under user composing (R0–R5 exact path).
pub fn resolve_code_len(
    lex: Option<&DatLexicon>,
    composing: &str,
    syllables: &[String],
    text: &str,
    default_full: bool,
) -> u32 {
    if composing.is_empty() {
        return 0;
    }
    let full = composing.len() as u32;
    let first_syl = first_syllable_len(composing, syllables) as u32;
    let first_syl = first_syl.clamp(1, full);
    let ends = syllable_prefix_ends(composing, syllables);
    let single = text.chars().count() <= 1;

    if let Some(lex) = lex {
        for &end in &ends {
            if end == 0 || end > composing.len() {
                continue;
            }
            let key = &composing[..end];
            if key_has_word(lex, key, text) {
                return end as u32;
            }
        }
        // Incomplete / IntraSyl: prefixes of the first syllable.
        let fe = first_syl as usize;
        if fe > 1 {
            for end in 1..fe {
                let key = &composing[..end];
                if key_has_word(lex, key, text) {
                    return end as u32;
                }
            }
        }
    }
    if single {
        return first_syl;
    }
    if default_full {
        full
    } else {
        first_syl
    }
}

fn key_has_word(lex: &DatLexicon, key: &str, text: &str) -> bool {
    for k in span_key_variants(key) {
        if lex.exact_key_words(&k).iter().any(|(_, w)| w == text) {
            return true;
        }
    }
    false
}

/// After a hit on `variant`, resolve span on `original` (R6).
pub fn resolve_code_len_via_variant(
    lex: Option<&DatLexicon>,
    original: &str,
    variant: &str,
    syllables: &[String],
    text: &str,
    var_span: u32,
) -> u32 {
    if original.is_empty() {
        return 0;
    }
    // Prefer exact resolve on original when possible.
    let on_orig = resolve_code_len(lex, original, syllables, text, false);
    let full = original.len() as u32;
    let single = text.chars().count() <= 1;
    if single && on_orig > 0 && on_orig < full {
        return on_orig;
    }
    if !single && on_orig > 0 && on_orig <= full {
        // Multi-char found on original structure — use it.
        if lex.is_some_and(|l| {
            syllable_prefix_ends(original, syllables)
                .iter()
                .any(|&e| e == on_orig as usize && key_has_word(l, &original[..e], text))
        }) {
            return on_orig;
        }
    }
    let mapped = map_variant_span_to_original(original, variant, var_span);
    if mapped == 0 {
        resolve_code_len(lex, original, syllables, text, true)
    } else {
        mapped
    }
}

/// Attach / tighten `code_len` for all candidates under original composing.
pub fn attach_spans(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cands: &mut [Candidate],
) {
    if composing.is_empty() || cands.is_empty() {
        return;
    }
    let full = composing.len() as u32;
    for c in cands.iter_mut() {
        let single = c.text.chars().count() <= 1;
        // Multi-char with a short structural span already set: keep (R0).
        if !single && c.code_len > 0 && c.code_len < full {
            // Still clamp if longer than a known shorter key for same text.
            let resolved = resolve_code_len(Some(lex), composing, syllables, &c.text, true);
            if resolved > 0 && resolved < c.code_len {
                c.code_len = resolved;
            }
            continue;
        }
        let target = resolve_code_len(Some(lex), composing, syllables, &c.text, true);
        if c.code_len == 0 || c.code_len > target {
            c.code_len = target;
        } else if single && c.code_len > target {
            c.code_len = target;
        }
        // Single-char: always clamp down to resolved (may equal first_syl).
        if single {
            let t = resolve_code_len(Some(lex), composing, syllables, &c.text, false);
            if c.code_len == 0 || c.code_len > t {
                c.code_len = t;
            }
        }
    }
}

/// Alias used by engine/AI inject paths.
pub fn infer_code_len(
    lex: Option<&DatLexicon>,
    composing: &str,
    syllables: &[String],
    text: &str,
    default_full: bool,
) -> u32 {
    resolve_code_len(lex, composing, syllables, text, default_full)
}

/// Alias for lookup finish / user-boost paths.
pub fn tighten_code_lens(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cands: &mut [Candidate],
) {
    attach_spans(lex, composing, syllables, cands);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_tsv_to_dat;
    use crate::lookup_opts::LookupOpts;

    #[test]
    fn map_same_len_typo() {
        assert_eq!(map_variant_span_to_original("wn", "wm", 2), 2);
    }

    #[test]
    fn map_expand_z_to_zh_full_word() {
        // Full-variant consume → full original.
        assert_eq!(
            map_variant_span_to_original("zongguo", "zhongguo", 8),
            7
        );
    }

    #[test]
    fn resolve_liu_under_liuchang() {
        let tmp = std::env::temp_dir().join("yc_span_resolve_liu.tsv");
        std::fs::write(
            &tmp,
            "word\tfreq\tpinyin\n流\t80000\tliu\n畅\t70000\tchang\n",
        )
        .unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["liu".into(), "chang".into()];
        assert_eq!(
            resolve_code_len(Some(&lex), "liuchang", &syls, "流", false),
            3
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn attach_spans_instant_style() {
        let tmp = std::env::temp_dir().join("yc_span_attach_liu.tsv");
        std::fs::write(
            &tmp,
            "word\tfreq\tpinyin\n流\t80000\tliu\n畅\t70000\tchang\n",
        )
        .unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["liu".into(), "chang".into()];
        let mut cands = lex
            .lookup_pinyin_opts("liuchang", &syls, None, 0, LookupOpts::instant())
            .unwrap();
        attach_spans(&lex, "liuchang", &syls, &mut cands);
        let liu = cands.iter().find(|c| c.text == "流").expect("流");
        assert_eq!(liu.code_len, 3);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn incomplete_bei_resolves_to_3() {
        let tmp = std::env::temp_dir().join("yc_span_bei.tsv");
        std::fs::write(&tmp, "word\tfreq\tpinyin\n北\t90000\tbei\n京\t80000\tjing\n").unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["bei".into(), "jing".into()];
        assert_eq!(
            resolve_code_len(Some(&lex), "beijing", &syls, "北", false),
            3
        );
        assert_eq!(
            resolve_code_len(Some(&lex), "bei", &syls, "北", false),
            3
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn fuzzy_zongguo_china_consumes_original_len() {
        let tmp = std::env::temp_dir().join("yc_span_zongguo.tsv");
        std::fs::write(&tmp, "word\tfreq\tpinyin\n中国\t90000\tzhongguo\n").unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec![
            "zhong".into(),
            "guo".into(),
            "zong".into(),
        ];
        let cands = lex
            .lookup_pinyin_opts("zongguo", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let hit = cands.iter().find(|c| c.text == "中国");
        assert!(hit.is_some(), "fuzzy should find 中国: {:?}", cands.iter().map(|c| &c.text).collect::<Vec<_>>());
        assert_eq!(hit.unwrap().code_len, 7, "consume original zongguo");
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn typo_wn_women_consumes_original() {
        let tmp = std::env::temp_dir().join("yc_span_wn.tsv");
        std::fs::write(&tmp, "word\tfreq\tpinyin\n我们\t90000\twomen\n").unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["wo".into(), "men".into()];
        let cands = lex
            .lookup_pinyin_opts("wn", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let hit = cands.iter().find(|c| c.text == "我们");
        assert!(hit.is_some(), "typo wn→我们: {:?}", cands.iter().map(|c| &c.text).collect::<Vec<_>>());
        assert_eq!(hit.unwrap().code_len, 2);
        let _ = std::fs::remove_file(tmp);
    }
}
