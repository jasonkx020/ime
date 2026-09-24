//! Gen stage: turn Seg hypotheses into lexicon candidates (prefix edges).
//! Full lattice sentences remain in `segment::merge_composed_ex`.

use std::collections::HashSet;

use yc_types::{Candidate, CandidateSource};

use crate::cand_tiers::{score_l3, score_l4, SCORE_L3};
use crate::dat::DatLexicon;
use crate::pinyin_match::is_orphan_final;
use crate::rank::rank_candidates;
use crate::seg_hypotheses::{HypKind, SegHypotheses, SegSeverity};
use crate::span_resolve::span_key_variants;

const WORDS_PER_SPAN: usize = 8;
const SINGLE_CHAR_FALLBACK_CAP: usize = 8;

/// Fill `cands` from structural hypotheses (Mono / Multi / Initial / Intra).
/// Only emits edges that start at byte 0 (prefix residual chain).
pub fn gen_from_hyps(
    lex: &DatLexicon,
    composing: &str,
    hyps: &SegHypotheses,
    cands: &mut Vec<Candidate>,
) {
    let composing = composing.trim();
    if composing.is_empty() || hyps.spans.is_empty() {
        return;
    }
    let n = composing.len();
    let has_exact_full = !lex.exact_key_words(composing).is_empty();

    for hyp in &hyps.spans {
        if hyp.start != 0 {
            continue;
        }
        let key = hyp.key.as_str();
        if key.is_empty() {
            continue;
        }
        if is_orphan_final(key) && lex.exact_key_words(key).is_empty() {
            continue;
        }
        let code_bytes = hyp.code_len() as usize;
        for (rank, (_freq, text)) in words_for_span(lex, key).into_iter().enumerate() {
            let score = match hyp.kind {
                HypKind::MonoSyl | HypKind::MultiSyl => score_l3(rank, code_bytes),
                HypKind::Initial | HypKind::IntraSyl => {
                    if text.chars().count() <= 1 {
                        score_l4(rank)
                    } else {
                        (SCORE_L3 - 0.01 - rank as f32 * 0.001).max(0.80)
                    }
                }
            };
            merge_prefix_edge(cands, n, text, code_bytes, score, has_exact_full);
        }
    }
    rank_candidates(cands);
}

/// L4 single-char兜底：池空或 Seg L3 且无 `code_len==1` 单字时，按首字母查 DAT。
/// `code_len = 1`，分数落在 L4，不抢已有 L0–L3 正常带首位。
pub fn ensure_single_char_fallback(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    hyps: &SegHypotheses,
    cands: &mut Vec<Candidate>,
) {
    let composing = composing.trim();
    if composing.is_empty() {
        return;
    }
    let has_single = cands
        .iter()
        .any(|c| c.text.chars().count() == 1 && c.code_len == 1);
    let need = cands.is_empty()
        || (!has_single && matches!(hyps.severity, Some(SegSeverity::L3)));
    if !need {
        return;
    }

    let letters: Vec<char> = composing
        .chars()
        .filter(|c| c.is_ascii_lowercase())
        .take(2)
        .collect();
    if letters.is_empty() {
        return;
    }

    for (li, ch) in letters.iter().enumerate() {
        // Second letter only when pool still empty after first.
        if li > 0 && !cands.is_empty() {
            break;
        }
        let key = ch.to_string();
        let mut words = lex.exact_key_words(&key);
        if words.iter().all(|(_, t)| t.chars().count() != 1) {
            // Packs usually key 不 as `bu`, not `b` — scan one-syllable keys under initial.
            words = lex.jianpin_span_words(&key, &key, syllables, 1);
        }
        words.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut rank = 0usize;
        for (_freq, text) in words {
            if text.chars().count() != 1 {
                continue;
            }
            if cands.iter().any(|c| c.text == text) {
                continue;
            }
            cands.push(Candidate {
                id: 0,
                text,
                source: CandidateSource::Lexicon,
                score: score_l4(rank),
                code_len: 1,
            });
            rank += 1;
            if rank >= SINGLE_CHAR_FALLBACK_CAP {
                break;
            }
        }
        if rank > 0 {
            break;
        }
    }
}

fn words_for_span(lex: &DatLexicon, key: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (vi, k) in span_key_variants(key).into_iter().enumerate() {
        let fuzzy = vi > 0;
        for (freq, text) in lex.exact_key_words(&k) {
            if text.is_empty() || !seen.insert(text.clone()) {
                continue;
            }
            let freq = if fuzzy {
                freq.saturating_mul(85) / 100
            } else {
                freq
            };
            out.push((freq.max(1), text));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.truncate(WORDS_PER_SPAN);
    out
}

fn merge_prefix_edge(
    cands: &mut Vec<Candidate>,
    composing_len: usize,
    text: String,
    code_bytes: usize,
    score: f32,
    has_exact_full: bool,
) {
    if text.is_empty() || code_bytes == 0 {
        return;
    }
    if let Some(c) = cands.iter_mut().find(|c| c.text == text) {
        if (code_bytes as u32) < composing_len as u32
            && (c.code_len == 0 || (code_bytes as u32) < c.code_len)
        {
            c.code_len = code_bytes as u32;
        }
        return;
    }
    let cand = Candidate {
        id: 0,
        text,
        source: CandidateSource::Lexicon,
        score,
        code_len: code_bytes as u32,
    };
    if has_exact_full && score < 0.92 {
        // Keep L0 whole-key hits ahead of structural fillers.
        cands.push(cand);
        return;
    }
    let idx = cands
        .iter()
        .position(|c| cand.score > c.score)
        .unwrap_or(cands.len());
    cands.insert(idx, cand);
}
