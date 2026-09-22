//! One-edit fuzzy pinyin — delegates to [`crate::correction`] tables.

use crate::correction::{fuzzy_variants_with, CorrectionTables};

/// Fuzzy rewrites of `composing`, most useful first. Does not include the original.
pub fn fuzzy_variants(composing: &str, cap: usize) -> Vec<String> {
    fuzzy_variants_with(composing, &CorrectionTables::builtin_fallback(), cap)
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
