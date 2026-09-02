//! Light-weight correction / intel (M0 stub).

use yc_types::{Candidate, HotResult};

pub trait LightIntel {
    fn rerank(&self, _prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>>;
}

#[derive(Debug, Default)]
pub struct NoOpIntel;

impl LightIntel for NoOpIntel {
    fn rerank(&self, _prefix: &str, candidates: Vec<Candidate>) -> HotResult<Vec<Candidate>> {
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_passthrough() {
        let intel = NoOpIntel;
        let out = intel.rerank("ni", vec![]).unwrap();
        assert!(out.is_empty());
    }
}
