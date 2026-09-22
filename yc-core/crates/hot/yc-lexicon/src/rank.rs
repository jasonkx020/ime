//! Rank stage: single sort entry + feature fuse.
//! Correction band never outranks normal band for #1.

use std::collections::HashMap;

use yc_types::Candidate;

use crate::correction::CORRECTION_SCORE_CAP;
use crate::user_words::UserWordStore;

pub const USER_EXACT_BASE: f32 = 2.0;
pub const USER_FREQ_WEIGHT: f32 = 0.15;

pub fn user_boost_delta(effective_freq: u32) -> f32 {
    if effective_freq == 0 {
        0.0
    } else {
        USER_EXACT_BASE + effective_freq as f32 * USER_FREQ_WEIGHT
    }
}

/// True when candidate is in the correction score band.
pub fn is_correction_cand(c: &Candidate) -> bool {
    c.score <= CORRECTION_SCORE_CAP + 1e-6
}

/// Final sort: normal band before correction band, then by score.
pub fn rank_candidates(cands: &mut Vec<Candidate>) {
    cands.sort_by(|a, b| {
        let ac = is_correction_cand(a);
        let bc = is_correction_cand(b);
        ac.cmp(&bc)
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.text.cmp(&b.text))
    });
    for (i, c) in cands.iter_mut().enumerate() {
        c.id = i as u32;
    }
}

pub struct RankCtx<'a> {
    pub query_key: &'a str,
    pub lang: &'a str,
    pub assoc_prefix: &'a str,
    pub user: Option<&'a UserWordStore>,
    pub ngram: Option<&'a dyn Fn(&str, &str) -> f32>,
    pub prefer_pairs: Option<&'a HashMap<String, HashMap<String, f32>>>,
}

/// Fuse user / ngram / prefer into scores, clamp correction band, then rank.
pub fn rank_fuse(cands: &mut Vec<Candidate>, ctx: &RankCtx<'_>) {
    if cands.is_empty() {
        return;
    }
    for c in cands.iter_mut() {
        let was_corr = is_correction_cand(c);
        if let Some(store) = ctx.user {
            let f = store.freq_lang(ctx.lang, ctx.query_key, &c.text);
            if f > 0 {
                c.score += user_boost_delta(f);
            }
        }
        if !ctx.assoc_prefix.is_empty() {
            if let Some(ng) = ctx.ngram {
                let bonus = ng(ctx.assoc_prefix, &c.text);
                if bonus != 0.0 {
                    c.score += (bonus * 0.02).clamp(-0.08, 0.08);
                }
            }
        }
        if let Some(pairs) = ctx.prefer_pairs {
            if let Some(by_next) = pairs.get(ctx.assoc_prefix) {
                if let Some(delta) = by_next.get(&c.text) {
                    c.score += *delta;
                }
            }
        }
        // Keep correction items in correction band so they cannot steal #1
        // when normal candidates exist (re-clamp after boosts).
        if was_corr {
            c.score = c.score.min(CORRECTION_SCORE_CAP);
        }
    }
    rank_candidates(cands);
}

pub fn apply_ngram_nudge(
    cands: &mut Vec<Candidate>,
    context: &str,
    bonus_fn: impl Fn(&str, &str) -> f32,
) {
    if context.is_empty() || cands.is_empty() {
        return;
    }
    let mut touched = false;
    for c in cands.iter_mut() {
        let was_corr = is_correction_cand(c);
        let bonus = bonus_fn(context, &c.text);
        if bonus == 0.0 {
            continue;
        }
        let adj = (bonus * 0.02).clamp(-0.08, 0.08);
        if adj != 0.0 {
            c.score += adj;
            if was_corr {
                c.score = c.score.min(CORRECTION_SCORE_CAP);
            }
            touched = true;
        }
    }
    if touched {
        rank_candidates(cands);
    }
}

/// Cap a score into the correction band (Gen exit for fuzzy/typo/edit hits).
pub fn as_correction_score(base: f32) -> f32 {
    base.min(CORRECTION_SCORE_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::CandidateSource;

    #[test]
    fn correction_never_before_normal() {
        let mut cands = vec![
            Candidate {
                id: 0,
                text: "纠错词".into(),
                source: CandidateSource::Lexicon,
                score: CORRECTION_SCORE_CAP,
                code_len: 5,
            },
            Candidate {
                id: 1,
                text: "正常词".into(),
                source: CandidateSource::Lexicon,
                score: 0.86,
                code_len: 5,
            },
        ];
        rank_candidates(&mut cands);
        assert_eq!(cands[0].text, "正常词");
        assert_eq!(cands[1].text, "纠错词");
    }

    #[test]
    fn rank_orders_by_score_then_text() {
        let mut cands = vec![
            Candidate {
                id: 9,
                text: "b".into(),
                source: CandidateSource::Lexicon,
                score: 0.9,
                code_len: 1,
            },
            Candidate {
                id: 8,
                text: "a".into(),
                source: CandidateSource::Lexicon,
                score: 0.9,
                code_len: 1,
            },
            Candidate {
                id: 7,
                text: "c".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
                code_len: 1,
            },
        ];
        rank_candidates(&mut cands);
        assert_eq!(cands[0].text, "c");
        assert_eq!(cands[1].text, "a");
        assert_eq!(cands[2].text, "b");
    }
}
