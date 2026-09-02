use yc_types::{Candidate, CandidateSource};

/// Built-in demo lexicon: prefix → Chinese candidates.
const ENTRIES: &[(&str, &[&str])] = &[
    ("ni", &["你", "尼", "泥"]),
    ("nihao", &["你好", "你好吗", "你好啊"]),
    ("hao", &["好", "号", "豪"]),
    ("wo", &["我", "握", "窝"]),
    ("women", &["我们", "我门"]),
];

#[derive(Debug, Default)]
pub struct InMemoryLexicon;

impl InMemoryLexicon {
    pub fn new() -> Self {
        Self
    }

    pub fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        lookup_prefix(self, prefix)
    }
}

pub(crate) fn lookup_prefix(_lex: &InMemoryLexicon, prefix: &str) -> Vec<Candidate> {
        let prefix = prefix.trim().to_ascii_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut best: Option<&[&str]> = None;
        let mut best_len = 0usize;

        for (key, words) in ENTRIES {
            if prefix.starts_with(key) && key.len() >= best_len {
                best = Some(words);
                best_len = key.len();
            }
        }

        let words = match best {
            Some(w) => w,
            None => return Vec::new(),
        };

        words
            .iter()
            .enumerate()
            .map(|(i, text)| Candidate {
                id: i as u32,
                text: (*text).to_string(),
                source: CandidateSource::Lexicon,
                score: 1.0 - (i as f32 * 0.05),
            })
            .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_nihao() {
        let lex = InMemoryLexicon::new();
        let cands = lex.lookup("nihao");
        assert!(!cands.is_empty());
        assert_eq!(cands[0].text, "你好");
    }

    #[test]
    fn lookup_prefix_ni() {
        let lex = InMemoryLexicon::new();
        let cands = lex.lookup("ni");
        assert!(cands.iter().any(|c| c.text == "你"));
    }
}
