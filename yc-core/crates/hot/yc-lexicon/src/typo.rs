//! QWERTY adjacent-key typo variants — delegates to layout neighbor tables.

use crate::correction::{adjacent_typo_variants_with, CorrectionTables};

/// Generate single-edit adjacent-key substitutions (builtin QWERTY neighbors).
pub fn adjacent_typo_variants(composing: &str) -> Vec<String> {
    adjacent_typo_variants_with(composing, &CorrectionTables::builtin_fallback(), 24)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn n_neighbors_include_m() {
        let t = CorrectionTables::builtin_fallback();
        assert!(t.neighbors.get(&'n').unwrap().contains(&'m'));
        assert!(t.neighbors.get(&'m').unwrap().contains(&'n'));
    }
}
