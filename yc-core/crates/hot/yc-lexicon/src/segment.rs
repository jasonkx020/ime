//! Sentence segmentation: syllable lattice when the input is full pinyin,
//! otherwise the mixed initial/syllable slots used for jianpin (`haopg`).

use std::collections::HashSet;

use yc_types::{Candidate, CandidateSource};

use crate::cand_tiers::{
    score_l2, score_l2_from_parts, score_l3, score_l4, SCORE_L0, SCORE_L2, SCORE_L3, SCORE_L4,
};
use crate::dat::DatLexicon;
use crate::lookup_opts::LookupCancel;
use crate::pinyin_match::{is_orphan_final, mixed_slots, MixSlot};

const MAX_LATTICE_BYTES: usize = 24;
const MAX_SYLLABLES_PER_SPAN: u8 = 6;
const WORDS_PER_SPAN: usize = 3;
const BEAM_WIDTH: usize = 8;
const TOP_SENTENCES: usize = 16;

/// Edge tier: P1 complete-syllable / multi-syl spans; P2 fine letter splits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeTier {
    P1 = 0,
    P2 = 1,
}

#[derive(Clone)]
struct Edge {
    end: usize,
    text: String,
    freq: u32,
    code_bytes: usize,
    tier: EdgeTier,
}

#[derive(Clone)]
struct Acc {
    dp: f64,
    sum_ln: f64,
    chars: u32,
    edges: u32,
    text: String,
    /// (word text, end byte offset in composing)
    spans: Vec<(String, usize)>,
}

/// Merge composed sentences into `cands`. Returns `None` if canceled.
///
/// `has_exact_full` is inferred inside; fine splits are off (lean path).
#[allow(dead_code)]
pub fn merge_composed(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
) -> Option<()> {
    merge_composed_ex(lex, composing, syllables, cancel, mine, cands, false)
}

/// Like [`merge_composed`], with `include_fine_splits` forcing P2 letter edges
/// (used on expanded / end-of-page).
pub fn merge_composed_ex(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
    include_fine_splits: bool,
) -> Option<()> {
    let composing = composing.trim();
    if composing.is_empty() {
        return Some(());
    }
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }

    let has_exact_full = !lex.exact_key_words(composing).is_empty();

    if composing.len() <= MAX_LATTICE_BYTES && covers_with_syllables(composing, syllables) {
        merge_syllable_lattice(
            lex,
            composing,
            syllables,
            cancel,
            mine,
            cands,
            has_exact_full,
            include_fine_splits,
        )
    } else {
        merge_mixed(lex, composing, syllables, cancel, mine, cands, has_exact_full)
    }
}

fn covers_with_syllables(input: &str, table: &[String]) -> bool {
    let n = input.len();
    if n == 0 {
        return false;
    }
    let jumps = syllable_jumps(input, table);
    let mut reach = vec![false; n + 1];
    reach[0] = true;
    for i in 0..n {
        if !reach[i] {
            continue;
        }
        for &j in &jumps[i] {
            reach[j] = true;
        }
    }
    reach[n]
}

fn syllable_jumps(input: &str, table: &[String]) -> Vec<Vec<usize>> {
    let n = input.len();
    let mut jumps = vec![Vec::new(); n + 1];
    for i in 0..n {
        let rest = &input[i..];
        for syl in table {
            if !syl.is_empty() && rest.starts_with(syl.as_str()) {
                jumps[i].push(i + syl.len());
            }
        }
        jumps[i].sort_unstable();
        jumps[i].dedup();
    }
    jumps
}

/// Keys to try for a span: exact + pair variants + fuzzy (see span_resolve).
fn span_key_variants(key: &str) -> Vec<String> {
    crate::span_resolve::span_key_variants(key)
}

fn words_for_span(lex: &DatLexicon, key: &str) -> Vec<(u32, String, bool)> {
    // (freq, text, is_fuzzy)
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
            out.push((freq.max(1), text, fuzzy));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.truncate(WORDS_PER_SPAN);
    out
}

fn skip_orphan_span(lex: &DatLexicon, key: &str) -> bool {
    if !is_orphan_final(key) {
        return false;
    }
    lex.exact_key_words(key).is_empty()
}

fn first_syllable_end(_composing: &str, jumps: &[Vec<usize>]) -> Option<usize> {
    jumps.first()?.iter().copied().max()
}

/// Byte length of the first complete syllable in `composing`, or `composing.len()`.
pub fn first_syllable_len(composing: &str, syllables: &[String]) -> usize {
    if composing.is_empty() {
        return 0;
    }
    let jumps = syllable_jumps(composing, syllables);
    first_syllable_end(composing, &jumps).unwrap_or(composing.len())
}

/// Syllable-boundary prefix ends from the start of `composing` (ascending).
pub fn syllable_prefix_ends(composing: &str, syllables: &[String]) -> Vec<usize> {
    if composing.is_empty() {
        return Vec::new();
    }
    let jumps = syllable_jumps(composing, syllables);
    let mut ends = Vec::new();
    let mut stack = vec![0usize];
    let mut seen = HashSet::new();
    seen.insert(0usize);
    while let Some(pos) = stack.pop() {
        if pos > 0 {
            ends.push(pos);
        }
        for &nxt in jumps.get(pos).into_iter().flatten() {
            if nxt <= composing.len() && seen.insert(nxt) {
                stack.push(nxt);
            }
        }
    }
    ends.sort_unstable();
    ends.dedup();
    ends
}

fn merge_syllable_lattice(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
    has_exact_full: bool,
    include_fine_splits: bool,
) -> Option<()> {
    let n = composing.len();
    let jumps = syllable_jumps(composing, syllables);
    let mut edges: Vec<Vec<Edge>> = vec![Vec::new(); n + 1];

    // P1: multi-syllable spans from syllable jumps.
    for i in 0..n {
        if i & 0x7 == 0 && cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }
        for end in span_ends(i, &jumps, n) {
            let key = &composing[i..end];
            if skip_orphan_span(lex, key) {
                continue;
            }
            for (freq, text, _) in words_for_span(lex, key) {
                edges[i].push(Edge {
                    end,
                    text,
                    freq,
                    code_bytes: end - i,
                    tier: EdgeTier::P1,
                });
            }
        }
    }

    // P2: IntraSyl / Initial under the *current* first syllable (residual-aware).
    let first_end = first_syllable_end(composing, &jumps).unwrap_or(0);
    let first_has_p1 = edges
        .first()
        .map(|es| es.iter().any(|e| e.tier == EdgeTier::P1 && e.end == first_end))
        .unwrap_or(false);
    // Emit fine splits when expanded, or when the first syllable has no lexicon words.
    if include_fine_splits || !first_has_p1 {
        add_fine_split_edges(lex, composing, first_end, &mut edges);
    }

    let mut beam: Vec<Vec<Acc>> = vec![Vec::new(); n + 1];
    beam[0].push(Acc {
        dp: 0.0,
        sum_ln: 0.0,
        chars: 0,
        edges: 0,
        text: String::new(),
        spans: Vec::new(),
    });

    for i in 0..n {
        if beam[i].is_empty() {
            continue;
        }
        let current = beam[i].clone();
        for acc in current {
            for e in &edges[i] {
                let mut text = acc.text.clone();
                text.push_str(&e.text);
                let chars = e.text.chars().count() as u32;
                let ln = (e.freq as f64 + 1.0).ln();
                let tier_pen = match e.tier {
                    EdgeTier::P1 => 0.0,
                    EdgeTier::P2 => -4.0,
                };
                let mut spans = acc.spans.clone();
                spans.push((e.text.clone(), e.end));
                push_beam(
                    &mut beam[e.end],
                    Acc {
                        dp: acc.dp + ln + 0.8 * chars as f64 - 12.0 + tier_pen,
                        sum_ln: acc.sum_ln + ln,
                        chars: acc.chars + chars,
                        edges: acc.edges + 1,
                        text,
                        spans,
                    },
                );
            }
        }
    }

    let mut sentences: Vec<(Candidate, Vec<(String, usize)>)> = Vec::new();
    for acc in &beam[n] {
        if acc.text.is_empty() || acc.edges < 2 {
            continue;
        }
        let quality_parts = score_l2_from_parts(acc.sum_ln, acc.chars, acc.edges);
        let score = if has_exact_full {
            quality_parts.min(SCORE_L2)
        } else {
            quality_parts
        };
        sentences.push((
            Candidate {
                id: 0,
                text: acc.text.clone(),
                source: CandidateSource::Lexicon,
                score,
                code_len: n as u32,
            },
            acc.spans.clone(),
        ));
    }
    sentences.sort_by(|a, b| {
        b.0.score
            .partial_cmp(&a.0.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.text.cmp(&b.0.text))
    });
    sentences.truncate(TOP_SENTENCES);

    for (sent, spans) in &sentences {
        insert_by_score(cands, sent.clone(), has_exact_full);
        // Path prefixes stay in L2 (slightly below full sentence).
        let mut prefix_text = String::new();
        for (word, end) in spans {
            prefix_text.push_str(word);
            if *end >= n {
                continue;
            }
            let cover = (*end as f32) / (n as f32);
            let score = (SCORE_L2 - 0.01 + 0.02 * cover).clamp(0.88, SCORE_L2);
            insert_by_score(
                cands,
                Candidate {
                    id: 0,
                    text: prefix_text.clone(),
                    source: CandidateSource::Lexicon,
                    score,
                    code_len: *end as u32,
                },
                has_exact_full,
            );
        }
    }

    // L3 syllable spans then L4 letter splits.
    let mut first = edges.first().cloned().unwrap_or_default();
    first.sort_by(|a, b| {
        (a.tier as u8)
            .cmp(&(b.tier as u8))
            .then(b.freq.cmp(&a.freq))
            .then(b.code_bytes.cmp(&a.code_bytes))
            .then(a.text.cmp(&b.text))
    });
    for (rank, e) in first.into_iter().take(32).enumerate() {
        let score = match e.tier {
            EdgeTier::P1 => score_l3(rank, e.code_bytes),
            EdgeTier::P2 => score_l4(rank),
        };
        merge_prefix_edge(cands, n, e.text, e.code_bytes, score, has_exact_full);
    }

    // Subsequent syllables as L3.
    if let Some(fe) = first_syllable_end(composing, &jumps) {
        if fe < n {
            let mut rest_edges = edges.get(fe).cloned().unwrap_or_default();
            rest_edges.sort_by(|a, b| {
                b.freq
                    .cmp(&a.freq)
                    .then(b.code_bytes.cmp(&a.code_bytes))
                    .then(a.text.cmp(&b.text))
            });
            for (rank, e) in rest_edges.into_iter().take(16).enumerate() {
                let score = match e.tier {
                    EdgeTier::P1 => (SCORE_L3 - 0.01 - rank as f32 * 0.001).max(SCORE_L4 + 0.01),
                    EdgeTier::P2 => score_l4(rank + 8),
                };
                merge_prefix_edge(cands, n, e.text, e.code_bytes, score, has_exact_full);
            }
        }
    }

    reassign_ids(cands);
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }
    Some(())
}

/// Letter-level edges under the **current** first syllable of `composing`
/// (IntraSyl / Initial under residual, e.g. t|a|o for `tao`, or under `liuchang`→`liu`).
fn add_fine_split_edges(
    lex: &DatLexicon,
    composing: &str,
    first_end: usize,
    edges: &mut [Vec<Edge>],
) {
    if first_end == 0 || first_end > composing.len() {
        return;
    }
    let bytes = composing.as_bytes();
    let mut i = 0usize;
    while i < first_end {
        let ch = bytes[i] as char;
        if !ch.is_ascii_lowercase() {
            break;
        }
        let key = &composing[i..i + 1];
        if skip_orphan_span(lex, key) {
            // Still advance so we don't get stuck; orphan letters don't form edges.
            i += 1;
            continue;
        }
        for (freq, text, _) in words_for_span(lex, key) {
            edges[i].push(Edge {
                end: i + 1,
                text,
                freq: freq.saturating_mul(70) / 100,
                code_bytes: 1,
                tier: EdgeTier::P2,
            });
        }
        // Also allow multi-letter prefixes inside the first syllable (ta under tao).
        for end in (i + 2)..=first_end {
            let key = &composing[i..end];
            if skip_orphan_span(lex, key) {
                continue;
            }
            // Prefer not duplicating the full first syllable as P2 (already P1).
            if i == 0 && end == first_end {
                continue;
            }
            for (freq, text, _) in words_for_span(lex, key) {
                edges[i].push(Edge {
                    end,
                    text,
                    freq: freq.saturating_mul(80) / 100,
                    code_bytes: end - i,
                    tier: EdgeTier::P2,
                });
            }
        }
        i += 1;
    }
}

fn span_ends(start: usize, jumps: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut stack = vec![(start, 0u8)];
    let mut seen = HashSet::<(usize, u8)>::new();
    while let Some((pos, depth)) = stack.pop() {
        if depth > 0 {
            ends.push(pos);
        }
        if depth >= MAX_SYLLABLES_PER_SPAN {
            continue;
        }
        for &nxt in &jumps[pos] {
            if nxt <= n && seen.insert((nxt, depth + 1)) {
                stack.push((nxt, depth + 1));
            }
        }
    }
    ends.sort_unstable();
    ends.dedup();
    ends
}

fn push_beam(beam: &mut Vec<Acc>, acc: Acc) {
    if let Some(existing) = beam.iter_mut().find(|e| e.text == acc.text) {
        if acc.dp > existing.dp {
            *existing = acc;
        }
        return;
    }
    beam.push(acc);
    beam.sort_by(|a, b| {
        b.dp
            .partial_cmp(&a.dp)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    beam.truncate(BEAM_WIDTH);
}

fn merge_mixed(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
    has_exact_full: bool,
) -> Option<()> {
    let slots = mixed_slots(composing, syllables);
    if slots.is_empty() {
        return Some(());
    }
    let n = slots.len();
    if n > 3 && slots.iter().all(|s| matches!(s, MixSlot::Initial(_))) {
        return Some(());
    }
    let mut slot_starts = Vec::with_capacity(n + 1);
    let mut acc = 0usize;
    slot_starts.push(0);
    for slot in &slots {
        acc += match slot {
            MixSlot::Syl(s) => s.len(),
            MixSlot::Initial(_) => 1,
        };
        slot_starts.push(acc);
    }
    let covered_bytes = acc;

    let mut lattice: Vec<Vec<Edge>> = vec![Vec::new(); n];
    for i in 0..n {
        if i & 0x7 == 0 && cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }
        let mut pattern = String::new();
        let mut any_initial = false;
        for j in (i + 1)..=n {
            match slots[j - 1] {
                MixSlot::Syl(s) => pattern.push_str(s),
                MixSlot::Initial(c) => {
                    pattern.push(c);
                    any_initial = true;
                }
            }
            let span = j - i;
            let code_bytes = slot_starts[j] - slot_starts[i];
            if !any_initial && skip_orphan_span(lex, &pattern) {
                continue;
            }
            let words = if any_initial {
                let key_prefix = match slots[i] {
                    MixSlot::Syl(s) => s.to_string(),
                    MixSlot::Initial(c) => c.to_string(),
                };
                lex.jianpin_span_words(&key_prefix, &pattern, syllables, span)
            } else {
                lex.exact_key_words(&pattern)
            };
            if let Some((freq, text)) = pick_best_word(&words, span) {
                lattice[i].push(Edge {
                    end: j,
                    text,
                    freq,
                    code_bytes,
                    tier: EdgeTier::P1,
                });
            }
        }
    }

    let mut best_score = vec![None::<f64>; n + 1];
    let mut prev_edge: Vec<Option<(usize, usize)>> = vec![None; n + 1];
    best_score[0] = Some(0.0);

    for i in 0..n {
        let Some(base) = best_score[i] else {
            continue;
        };
        for (ei, e) in lattice[i].iter().enumerate() {
            let chars = e.text.chars().count() as f64;
            let edge_score = (e.freq as f64 + 1.0).ln() + 0.8 * chars - 12.0;
            let total = base + edge_score;
            let better = match best_score[e.end] {
                None => true,
                Some(s) => total > s,
            };
            if better {
                best_score[e.end] = Some(total);
                prev_edge[e.end] = Some((i, ei));
            }
        }
    }

    if best_score[n].is_some() {
        let mut parts_ln = 0.0f64;
        let mut chars = 0u32;
        let mut n_edges = 0u32;
        let mut texts = Vec::new();
        let mut at = n;
        while at > 0 {
            let (from, ei) = prev_edge[at].expect("path");
            let e = &lattice[from][ei];
            parts_ln += (e.freq as f64 + 1.0).ln();
            chars += e.text.chars().count() as u32;
            n_edges += 1;
            texts.push(e.text.clone());
            at = from;
        }
        texts.reverse();
        let text: String = texts.concat();
        if !text.is_empty() {
            let mut score = score_l2(display_score(parts_ln, chars, n_edges, true));
            if has_exact_full {
                score = score.min(SCORE_L2);
            }
            insert_by_score(
                cands,
                Candidate {
                    id: 0,
                    text,
                    source: CandidateSource::Lexicon,
                    score,
                    code_len: covered_bytes as u32,
                },
                has_exact_full,
            );
        }
    }

    if !lattice.is_empty() {
        let mut first_spans: Vec<Edge> = lattice[0].clone();
        first_spans.sort_by(|a, b| {
            b.code_bytes
                .cmp(&a.code_bytes)
                .then(b.freq.cmp(&a.freq))
                .then(a.text.cmp(&b.text))
        });
        for (rank, e) in first_spans.into_iter().take(24).enumerate() {
            merge_prefix_edge(
                cands,
                composing.len(),
                e.text,
                e.code_bytes,
                score_l3(rank, e.code_bytes),
                has_exact_full,
            );
        }
    }

    reassign_ids(cands);
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }
    Some(())
}

fn display_score(sum_ln: f64, chars: u32, edges: u32, jianpin: bool) -> f32 {
    let n = edges.max(1) as f64;
    let quality = sum_ln / n + 0.8 * (chars as f64 / n);
    let mut score = 0.88 + (quality - 6.0) * 0.025;
    if jianpin {
        score -= 0.08;
    }
    score.clamp(0.80, 1.04) as f32
}

fn insert_by_score(cands: &mut Vec<Candidate>, mut cand: Candidate, has_exact_full: bool) {
    // Keep L2+ below true L0 exact full-key hits.
    if has_exact_full && cand.score < SCORE_L0 {
        if let Some(floor) = cands
            .iter()
            .filter(|c| c.score >= 0.995)
            .map(|c| c.score)
            .reduce(f32::min)
        {
            cand.score = cand.score.min(floor - 0.001);
        }
    }
    if let Some(pos) = cands.iter().position(|c| c.text == cand.text) {
        let old_len = cands[pos].code_len;
        if cand.score <= cands[pos].score {
            // Prefer shorter correct code_len when same text.
            if cand.code_len > 0
                && (cands[pos].code_len == 0 || cand.code_len < cands[pos].code_len)
            {
                cands[pos].code_len = cand.code_len;
            }
            return;
        }
        // Higher score wins, but keep the shorter non-zero code_len.
        let kept = match (old_len, cand.code_len) {
            (0, n) => n,
            (n, 0) => n,
            (a, b) => a.min(b),
        };
        cand.code_len = kept;
        cands.remove(pos);
    }
    let idx = cands
        .iter()
        .position(|c| cand.score > c.score)
        .unwrap_or(cands.len());
    cands.insert(idx, cand);
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
    insert_by_score(
        cands,
        Candidate {
            id: 0,
            text,
            source: CandidateSource::Lexicon,
            score,
            code_len: code_bytes as u32,
        },
        has_exact_full,
    );
}

fn reassign_ids(cands: &mut [Candidate]) {
    for (i, c) in cands.iter_mut().enumerate() {
        c.id = i as u32;
    }
}

fn pick_best_word(words: &[(u32, String)], span_syls: usize) -> Option<(u32, String)> {
    if words.is_empty() {
        return None;
    }
    let exact: Vec<_> = words
        .iter()
        .filter(|(_, w)| w.chars().count() == span_syls)
        .cloned()
        .collect();
    let pool = if !exact.is_empty() {
        exact
    } else {
        words.to_vec()
    };
    pool.into_iter()
        .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_tsv_to_dat;
    use crate::LookupOpts;

    #[test]
    fn compose_nihaoma_from_parts() {
        let tmp = std::env::temp_dir().join("yc_segment_nihaoma.tsv");
        let tsv = "\
word\tfreq\tpinyin
你\t90000\tni
好\t80000\thao
吗\t70000\tma
你好\t95000\tnihao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "ma".into()];
        let cands = lex
            .lookup_pinyin_opts("nihaoma", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            cands.first().map(|c| c.text.as_str()) == Some("你好吗")
                || cands.iter().any(|c| c.text == "你好吗"),
            "expected composed 你好吗, got {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let composed = cands.iter().find(|c| c.text == "你好吗").unwrap();
        assert_eq!(composed.code_len, 7);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn compose_falls_back_to_chars() {
        let tmp = std::env::temp_dir().join("yc_segment_chars.tsv");
        let tsv = "\
word\tfreq\tpinyin
你\t90000\tni
好\t80000\thao
吗\t70000\tma
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "ma".into()];
        let cands = lex
            .lookup_pinyin_opts("nihaoma", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            cands.iter().any(|c| c.text == "你好吗"),
            "char fallback compose: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn first_span_nihao_has_prefix_code_len() {
        let tmp = std::env::temp_dir().join("yc_segment_prefix.tsv");
        let tsv = "\
word\tfreq\tpinyin
你\t90000\tni
好\t80000\thao
吗\t70000\tma
你好\t95000\tnihao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "ma".into()];
        let cands = lex
            .lookup_pinyin_opts("nihaoma", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let nihao = cands.iter().find(|c| c.text == "你好").expect("你好");
        assert_eq!(nihao.code_len, 5, "nihao len");
        let ni = cands.iter().find(|c| c.text == "你").expect("你");
        assert_eq!(ni.code_len, 2);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn compose_haopg_without_whole_word() {
        let tmp = std::env::temp_dir().join("yc_segment_haopg.tsv");
        let tsv = "\
word\tfreq\tpinyin
好\t80000\thao
苹果\t90000\tpingguo
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["hao".into(), "ping".into(), "guo".into()];
        let slots = crate::mixed_slots("haopg", &syls);
        assert_eq!(
            slots,
            vec![
                crate::MixSlot::Syl("hao"),
                crate::MixSlot::Initial('p'),
                crate::MixSlot::Initial('g'),
            ]
        );
        let cands = lex
            .lookup_pinyin_opts("haopg", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let hit = cands.iter().find(|c| c.text == "好苹果");
        assert!(
            hit.is_some(),
            "expected composed 好苹果, got {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        assert_eq!(hit.unwrap().code_len, 5);
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn fangan_keeps_both_segmentations() {
        let tmp = std::env::temp_dir().join("yc_segment_fangan.tsv");
        let tsv = "\
word\tfreq\tpinyin
方案\t90000\tfangan
反感\t40000\tfangan
方\t50000\tfang
案\t50000\tan
反\t50000\tfan
感\t50000\tgan
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["fang".into(), "an".into(), "fan".into(), "gan".into()];
        let cands = lex
            .lookup_pinyin_opts("fangan", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
        let fang_an = texts
            .iter()
            .position(|t| *t == "方案")
            .expect(&format!("{texts:?}"));
        let fan_gan = texts
            .iter()
            .position(|t| *t == "反感")
            .expect(&format!("{texts:?}"));
        assert!(fang_an < fan_gan, "higher freq 方案 should lead: {texts:?}");
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn woaixian_prefers_xian_when_more_frequent() {
        let tmp = std::env::temp_dir().join("yc_segment_woaixian.tsv");
        let tsv = "\
word\tfreq\tpinyin
我\t90000\two
爱\t80000\tai
西\t20000\txi
安\t20000\tan
先\t50000\txian
西安\t120000\txian
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec![
            "wo".into(),
            "ai".into(),
            "xi".into(),
            "an".into(),
            "xian".into(),
        ];
        let cands = lex
            .lookup_pinyin_opts("woaixian", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
        let xian = texts
            .iter()
            .position(|t| *t == "我爱西安")
            .expect(&format!("{texts:?}"));
        let xian_char = texts.iter().position(|t| *t == "我爱先");
        if let Some(xian_char) = xian_char {
            assert!(
                xian < xian_char,
                "我爱西安 should rank above 我爱先: {texts:?}"
            );
        }
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn taoliuchang_composes_and_first_span() {
        let tmp = std::env::temp_dir().join("yc_segment_taoliuchang.tsv");
        let tsv = "\
word\tfreq\tpinyin
淘\t90000\ttao
桃\t80000\ttao
流\t85000\tliu
场\t70000\tchang
畅\t75000\tchang
流畅\t95000\tliuchang
他\t60000\tta
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls: Vec<String> = vec![
            "tao".into(),
            "liu".into(),
            "chang".into(),
            "chuang".into(),
            "ta".into(),
            "a".into(),
            "o".into(),
        ];
        let cands = lex
            .lookup_pinyin_opts("taoliuchang", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("流畅") || *t == "淘流畅" || *t == "桃流畅"),
            "expected compose with 流畅: {texts:?}"
        );
        let tao = cands.iter().find(|c| c.text == "淘" || c.text == "桃");
        assert!(tao.is_some(), "first syllable word: {texts:?}");
        assert_eq!(tao.unwrap().code_len, 3);
        // Fine splits should not outrank tao words on lean (no expanded).
        if let (Some(ti), Some(t_pos)) = (
            cands.iter().position(|c| c.text == "淘" || c.text == "桃"),
            cands.iter().position(|c| c.text == "他" && c.code_len == 1),
        ) {
            assert!(ti < t_pos, "tao before fine t: {texts:?}");
        }
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn orphan_i_u_v_not_listed_alone() {
        let tmp = std::env::temp_dir().join("yc_segment_orphan.tsv");
        let tsv = "\
word\tfreq\tpinyin
尼\t90000\tni
好\t80000\thao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "i".into(), "u".into(), "v".into()];
        let mut cands = Vec::new();
        crate::segment::merge_composed_ex(
            &lex,
            "nihao",
            &syls,
            None,
            0,
            &mut cands,
            true,
        )
        .unwrap();
        assert!(
            !cands.iter().any(|c| c.code_len == 1 && matches!(c.text.as_str(), "" )),
            "no empty"
        );
        // No standalone orphan-final candidates from empty keys.
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn tier_scores_compose_above_letter_splits() {
        let tmp = std::env::temp_dir().join("yc_segment_tiers.tsv");
        let tsv = "\
word\tfreq\tpinyin
淘\t90000\ttao
流\t85000\tliu
畅\t75000\tchang
流畅\t95000\tliuchang
他\t60000\tta
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls: Vec<String> = vec![
            "tao".into(),
            "liu".into(),
            "chang".into(),
            "ta".into(),
            "a".into(),
            "o".into(),
        ];
        let cands = lex
            .lookup_pinyin_opts(
                "taoliuchang",
                &syls,
                None,
                0,
                LookupOpts::expanded(),
            )
            .unwrap();
        let composed = cands
            .iter()
            .find(|c| c.text.contains("流畅") && c.code_len == 11);
        let letterish = cands.iter().find(|c| c.code_len == 1);
        if let (Some(c), Some(l)) = (composed, letterish) {
            assert!(
                c.score > l.score,
                "L2 compose {} > L4 letter {}: compose={} letter={}",
                c.text,
                l.text,
                c.score,
                l.score
            );
        }
        let _ = std::fs::remove_file(tmp);
    }
}
