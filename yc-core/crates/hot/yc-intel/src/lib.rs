//! Light-weight correction / intel.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use yc_lexicon::{merge_user_boosts, UserWordStore};
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
