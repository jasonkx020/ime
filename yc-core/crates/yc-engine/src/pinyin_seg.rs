//! Pinyin syllable segmentation and prefix validation for table-mode schemes.

pub use yc_lexicon::{is_valid_prefix, key_matches_composing, split_syllables};

/// Normalize composing text for lexicon lookup (lowercase, strip separators).
pub fn normalize_query(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<String> {
        vec![
            "ni".into(),
            "hao".into(),
            "zhong".into(),
            "guo".into(),
            "shu".into(),
            "ru".into(),
            "fa".into(),
        ]
    }

    #[test]
    fn split_nihao() {
        let syls = table();
        assert_eq!(split_syllables("nihao", &syls), vec!["ni", "hao"]);
    }

    #[test]
    fn split_zhongguo() {
        let syls = table();
        assert_eq!(split_syllables("zhongguo", &syls), vec!["zhong", "guo"]);
    }

    #[test]
    fn split_shurufa() {
        let syls = table();
        assert_eq!(
            split_syllables("shurufa", &syls),
            vec!["shu", "ru", "fa"]
        );
    }

    #[test]
    fn prefix_incremental() {
        let syls = table();
        assert!(is_valid_prefix("n", &syls));
        assert!(is_valid_prefix("ni", &syls));
        assert!(is_valid_prefix("nihao", &syls));
        assert!(!is_valid_prefix("nihaox", &syls));
    }

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
