//! Sentence segmentation: syllable lattice when the input is full pinyin,
//! otherwise the mixed initial/syllable slots used for jianpin (`haopg`).

use std::collections::HashSet;

use yc_types::{Candidate, CandidateSource};

use crate::dat::DatLexicon;
use crate::lookup_opts::LookupCancel;
use crate::pinyin_match::{mixed_slots, MixSlot};

const MAX_LATTICE_BYTES: usize = 24;
const MAX_SYLLABLES_PER_SPAN: u8 = 6;
const WORDS_PER_SPAN: usize = 3;

#[derive(Clone)]
struct Edge {
    end: usize,
    text: String,
    freq: u32,
    code_bytes: usize,
}

#[derive(Clone)]
struct Acc {
    dp: f64,
    sum_ln: f64,
    chars: u32,
    edges: u32,
    text: String,
}

/// Merge composed sentences into `cands`. Returns `None` if canceled.
pub fn merge_composed(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
) -> Option<()> {
    let composing = composing.trim();
    if composing.is_empty() {
        return Some(());
    }
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }

    if composing.len() <= MAX_LATTICE_BYTES && covers_with_syllables(composing, syllables) {
        merge_syllable_lattice(lex, composing, syllables, cancel, mine, cands)
    } else {
        merge_mixed(lex, composing, syllables, cancel, mine, cands)
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

fn merge_syllable_lattice(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
) -> Option<()> {
    let n = composing.len();
    let jumps = syllable_jumps(composing, syllables);
    let mut edges: Vec<Vec<Edge>> = vec![Vec::new(); n + 1];

    for i in 0..n {
        if i & 0x7 == 0 && cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }
        for end in span_ends(i, &jumps, n) {
            let key = &composing[i..end];
            let mut words = lex.exact_key_words(key);
            if words.is_empty() {
                continue;
            }
            words.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            words.truncate(WORDS_PER_SPAN);
            for (freq, text) in words {
                if text.is_empty() {
                    continue;
                }
                edges[i].push(Edge {
                    end,
                    text,
                    freq,
                    code_bytes: end - i,
                });
            }
        }
    }

    let mut beam: Vec<Vec<Acc>> = vec![Vec::new(); n + 1];
    beam[0].push(Acc {
        dp: 0.0,
        sum_ln: 0.0,
        chars: 0,
        edges: 0,
        text: String::new(),
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
                push_beam(
                    &mut beam[e.end],
                    Acc {
                        dp: acc.dp + ln + 0.8 * chars as f64 - 12.0,
                        sum_ln: acc.sum_ln + ln,
                        chars: acc.chars + chars,
                        edges: acc.edges + 1,
                        text,
                    },
                );
            }
        }
    }

    let mut sentences: Vec<Candidate> = Vec::new();
    for acc in &beam[n] {
        // A single lexicon word is already ranked by lookup. Only stitch 2+ words.
        if acc.text.is_empty() || acc.edges < 2 {
            continue;
        }
        sentences.push(Candidate {
            id: 0,
            text: acc.text.clone(),
            source: CandidateSource::Lexicon,
            score: display_score(acc.sum_ln, acc.chars, acc.edges, false),
            code_len: n as u32,
        });
    }
    sentences.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.text.cmp(&b.text))
    });
    for sent in sentences {
        insert_by_score(cands, sent);
    }

    let mut first = edges.first().cloned().unwrap_or_default();
    first.sort_by(|a, b| {
        b.code_bytes
            .cmp(&a.code_bytes)
            .then(b.freq.cmp(&a.freq))
            .then(a.text.cmp(&b.text))
    });
    for e in first.into_iter().take(24) {
        merge_prefix_edge(cands, n, e.text, e.code_bytes);
    }
    reassign_ids(cands);
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }
    Some(())
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
    beam.truncate(2);
}

fn merge_mixed(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    cands: &mut Vec<Candidate>,
) -> Option<()> {
    let slots = mixed_slots(composing, syllables);
    if slots.is_empty() {
        return Some(());
    }
    // All-initial strings like hhhh/hhhhh would O(n²) scan the h* bucket via
    // jianpin_span_words; skip long pure-initial lattices (haopg still has Syl).
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
            insert_by_score(
                cands,
                Candidate {
                    id: 0,
                    text,
                    source: CandidateSource::Lexicon,
                    score: display_score(parts_ln, chars, n_edges, true),
                    code_len: covered_bytes as u32,
                },
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
        for e in first_spans.into_iter().take(24) {
            merge_prefix_edge(cands, composing.len(), e.text, e.code_bytes);
        }
    }

    reassign_ids(cands);
    if cancel.is_some_and(|c| c.is_canceled(mine)) {
        return None;
    }
    Some(())
}

/// Average log-freq plus a mild length bonus. Jianpin sentences sit below full-pinyin hits.
fn display_score(sum_ln: f64, chars: u32, edges: u32, jianpin: bool) -> f32 {
    let n = edges.max(1) as f64;
    let quality = sum_ln / n + 0.8 * (chars as f64 / n);
    let mut score = 0.88 + (quality - 6.0) * 0.025;
    if jianpin {
        score -= 0.08;
    }
    score.clamp(0.80, 1.04) as f32
}

fn insert_by_score(cands: &mut Vec<Candidate>, mut cand: Candidate) {
    // Exact full-pinyin hits sit at ~1.0. A stitched sentence must not jump them
    // (nihao → 你好 stays ahead of 你+哈+哦).
    if let Some(floor) = cands
        .iter()
        .map(|c| c.score)
        .filter(|s| *s >= 0.97)
        .reduce(f32::min)
    {
        cand.score = cand.score.min(floor - 0.001);
    }
    if let Some(pos) = cands.iter().position(|c| c.text == cand.text) {
        if cand.score <= cands[pos].score {
            return;
        }
        cands.remove(pos);
    }
    let idx = cands
        .iter()
        .position(|c| cand.score > c.score)
        .unwrap_or(cands.len());
    cands.insert(idx, cand);
}

fn merge_prefix_edge(cands: &mut Vec<Candidate>, composing_len: usize, text: String, code_bytes: usize) {
    if text.is_empty() || code_bytes == 0 {
        return;
    }
    if let Some(c) = cands.iter_mut().find(|c| c.text == text) {
        if (code_bytes as u32) < composing_len as u32 && (c.code_len == 0 || (code_bytes as u32) < c.code_len)
        {
            c.code_len = code_bytes as u32;
        }
        return;
    }
    cands.push(Candidate {
        id: 0,
        text,
        source: CandidateSource::Lexicon,
        score: 0.95 - (code_bytes as f32 * 0.001),
        code_len: code_bytes as u32,
    });
}

fn reassign_ids(cands: &mut [Candidate]) {
    for (i, c) in cands.iter_mut().enumerate() {
        c.id = i as u32;
    }
}

/// Pick best word for a syllable span: prefer matching char-length ≈ span, else highest freq.
fn pick_best_word(words: &[(u32, String)], span_syls: usize) -> Option<(u32, String)> {
    if words.is_empty() {
        return None;
    }
    let exact: Vec<_> = words
        .iter()
        .filter(|(_, w)| w.chars().count() == span_syls)
        .cloned()
        .collect();
    let pool = if !exact.is_empty() { exact } else { words.to_vec() };
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
        let fang_an = texts.iter().position(|t| *t == "方案").expect(&format!("{texts:?}"));
        let fan_gan = texts.iter().position(|t| *t == "反感").expect(&format!("{texts:?}"));
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
        let xian = texts.iter().position(|t| *t == "我爱西安").expect(&format!("{texts:?}"));
        let xian_char = texts.iter().position(|t| *t == "我爱先");
        if let Some(xian_char) = xian_char {
            assert!(xian < xian_char, "我爱西安 should rank above 我爱先: {texts:?}");
        }
        let _ = std::fs::remove_file(tmp);
    }
}
