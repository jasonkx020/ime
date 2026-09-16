//! Light-weight correction / intel.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use yc_lexicon::{merge_user_boosts, SharedCharNgram, UserWordStore};
use yc_types::{Candidate, HotResult};

pub trait LightIntel: Send + Sync {
    fn rerank(&self, prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>>;
}

#[derive(Debug, Default)]
pub struct NoOpIntel;

impl LightIntel for NoOpIntel {
    fn rerank(&self, _prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>> {
        Ok(candidates)
    }
}

/// Rerank using user-word frequencies; skips work if budget exceeded.
#[derive(Debug, Clone)]
pub struct UserBoostIntel {
    store: Arc<Mutex<UserWordStore>>,
    budget: Duration,
}

impl UserBoostIntel {
    pub fn new(store: Arc<Mutex<UserWordStore>>) -> Self {
        Self {
            store,
            budget: Duration::from_millis(4),
        }
    }

    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }
}

impl LightIntel for UserBoostIntel {
    fn rerank(&self, prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>> {
        let start = Instant::now();
        let store = self.store.lock();
        if start.elapsed() > self.budget {
            return Ok(candidates);
        }
        Ok(merge_user_boosts(prefix, candidates, &store))
    }
}

fn is_cjk_context(prefix: &str) -> bool {
    prefix.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// Association / Hot-candidate rerank via shared char bigram; falls back to user boost for pinyin.
#[derive(Debug, Clone)]
pub struct NgramAssocIntel {
    user: UserBoostIntel,
    ngram: SharedCharNgram,
    budget: Duration,
}

impl NgramAssocIntel {
    pub fn new(store: Arc<Mutex<UserWordStore>>, ngram: SharedCharNgram) -> Self {
        Self {
            user: UserBoostIntel::new(store),
            ngram,
            budget: Duration::from_millis(4),
        }
    }

    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self.user = self.user.clone().with_budget(budget);
        self
    }

    pub fn shared_ngram(&self) -> SharedCharNgram {
        self.ngram.clone()
    }

    fn rerank_ngram(&self, prefix: &str, mut candidates: Vec<Candidate>) -> Vec<Candidate> {
        if !self.ngram.has_model() || candidates.is_empty() {
            return candidates;
        }
        candidates.sort_by(|a, b| {
            // Prefer single-character next-token, then bigram score.
            let la = a.text.chars().count();
            let lb = b.text.chars().count();
            let sa = self.ngram.continuation_score(prefix, &a.text) + a.score * 0.01
                - (la.saturating_sub(1) as f32) * 2.0;
            let sb = self.ngram.continuation_score(prefix, &b.text) + b.score * 0.01
                - (lb.saturating_sub(1) as f32) * 2.0;
            sb.partial_cmp(&sa)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| la.cmp(&lb))
                .then_with(|| a.text.cmp(&b.text))
        });
        for (i, c) in candidates.iter_mut().enumerate() {
            c.id = i as u32;
            c.score = 1.0 - (i as f32 * 0.001);
        }
        candidates
    }
}

impl LightIntel for NgramAssocIntel {
    fn rerank(&self, prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>> {
        let start = Instant::now();
        if start.elapsed() > self.budget {
            return Ok(candidates);
        }
        if is_cjk_context(prefix) {
            return Ok(self.rerank_ngram(prefix, candidates));
        }
        self.user.rerank(prefix, candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_lexicon::{compile_tsv_to_dat, DatLexicon};
    use yc_types::CandidateSource;

    #[test]
    fn noop_passthrough() {
        let intel = NoOpIntel;
        let out = intel.rerank("ni", vec![]).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn user_boost_rerank() {
        let store = UserWordStore::shared();
        store.lock().touch("ta", "他");
        store.lock().touch("ta", "他");
        let intel = UserBoostIntel::new(store);
        let cands = vec![
            Candidate {
                id: 0,
                text: "他们".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
            },
            Candidate {
                id: 1,
                text: "他".into(),
                source: CandidateSource::Lexicon,
                score: 0.85,
            },
        ];
        let out = intel.rerank("ta", cands).unwrap();
        assert_eq!(out[0].text, "他");
    }

    #[test]
    fn ngram_assoc_ranks_hao_first() {
        let sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv");
        if !sample.exists() {
            return;
        }
        let dat = compile_tsv_to_dat(&sample).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let shared = SharedCharNgram::new();
        shared.publish(lex.ngram());
        let intel = NgramAssocIntel::new(UserWordStore::shared(), shared);
        let cands = vec![
            Candidate {
                id: 0,
                text: "们".into(),
                source: CandidateSource::Hot,
                score: 1.0,
            },
            Candidate {
                id: 1,
                text: "的".into(),
                source: CandidateSource::Hot,
                score: 0.9,
            },
            Candidate {
                id: 2,
                text: "好".into(),
                source: CandidateSource::Hot,
                score: 0.5,
            },
        ];
        let out = intel.rerank("你", cands).unwrap();
        assert_eq!(out[0].text, "好", "got {:?}", out);
    }
}
