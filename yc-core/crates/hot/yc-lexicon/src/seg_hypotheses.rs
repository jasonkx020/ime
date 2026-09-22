//! Seg stage: structural hypotheses (letter spans) without lexicon lookup.

use crate::pinyin_match::is_orphan_final;

/// Kind of structural span (R2).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypKind {
    MonoSyl = 0,
    MultiSyl = 1,
    Initial = 2,
    IntraSyl = 3,
}

/// How badly Seg is damaged (纠错门控).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegSeverity {
    /// Mild — attach correction as extras.
    L1,
    /// Segmentation hurt but repairable.
    L2,
    /// Cannot cover with syllables — force correction.
    L3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanHyp {
    pub start: usize,
    pub end: usize,
    pub kind: HypKind,
    pub key: String,
}

impl SpanHyp {
    pub fn code_len(&self) -> u32 {
        (self.end.saturating_sub(self.start)) as u32
    }
}

/// One full syllable path (byte cuts).
#[derive(Debug, Clone, PartialEq)]
pub struct SegPath {
    pub cuts: Vec<usize>,
    pub score: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SegHypotheses {
    pub jumps: Vec<Vec<usize>>,
    pub spans: Vec<SpanHyp>,
    pub paths: Vec<SegPath>,
    pub severity: Option<SegSeverity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentMode {
    Off,
    FirstSyllable,
    Full,
    FullFine,
}

const MAX_SYL_PER_MULTI: u8 = 6;

/// Build pure structural hypotheses for `composing`.
pub fn build_seg_hypotheses(
    composing: &str,
    syllables: &[String],
    mode: SegmentMode,
) -> SegHypotheses {
    build_seg_hypotheses_ex(composing, syllables, mode, &[])
}

/// Like [`build_seg_hypotheses`], with preprocess hard syllable breaks.
pub fn build_seg_hypotheses_ex(
    composing: &str,
    syllables: &[String],
    mode: SegmentMode,
    hard_breaks: &[usize],
) -> SegHypotheses {
    let composing = composing.trim();
    let mut out = SegHypotheses::default();
    if composing.is_empty() || mode == SegmentMode::Off {
        out.severity = Some(SegSeverity::L3);
        return out;
    }
    let n = composing.len();
    out.jumps = syllable_jumps_respecting_breaks(composing, syllables, hard_breaks);

    let covered = is_fully_reachable(&out.jumps, n);
    out.severity = Some(if !covered {
        SegSeverity::L3
    } else if hard_breaks.is_empty() {
        SegSeverity::L1
    } else {
        SegSeverity::L2
    });

    let first_end = out
        .jumps
        .first()
        .and_then(|j| j.iter().copied().max())
        .unwrap_or(0);

    let path_cap = match mode {
        SegmentMode::Off => 0,
        SegmentMode::FirstSyllable => 1,
        SegmentMode::Full => 8,
        SegmentMode::FullFine => 16,
    };
    out.paths = enumerate_paths(&out.jumps, n, path_cap, hard_breaks);

    match mode {
        SegmentMode::Off => {}
        SegmentMode::FirstSyllable => {
            push_mono_at(&mut out.spans, composing, 0, first_end);
        }
        SegmentMode::Full | SegmentMode::FullFine => {
            // Prefer spans from pruned paths; fall back to full span enum.
            if !out.paths.is_empty() {
                for path in &out.paths {
                    let mut prev = 0usize;
                    for &cut in &path.cuts {
                        if cut > prev {
                            let key = composing[prev..cut].to_string();
                            if !is_orphan_final(&key) {
                                let depth = count_syl_depth(prev, cut, &out.jumps);
                                out.spans.push(SpanHyp {
                                    start: prev,
                                    end: cut,
                                    kind: if depth <= 1 {
                                        HypKind::MonoSyl
                                    } else {
                                        HypKind::MultiSyl
                                    },
                                    key,
                                });
                            }
                        }
                        prev = cut;
                    }
                    // Multi-syl prefixes along path
                    for i in 0..path.cuts.len() {
                        let start = if i == 0 { 0 } else { path.cuts[i - 1] };
                        for j in i..path.cuts.len() {
                            let end = path.cuts[j];
                            if end <= start {
                                continue;
                            }
                            let key = composing[start..end].to_string();
                            if is_orphan_final(&key) {
                                continue;
                            }
                            let depth = (j - i + 1) as u8;
                            out.spans.push(SpanHyp {
                                start,
                                end,
                                kind: if depth <= 1 {
                                    HypKind::MonoSyl
                                } else {
                                    HypKind::MultiSyl
                                },
                                key,
                            });
                        }
                    }
                }
            } else {
                for i in 0..n {
                    for end in span_ends(i, &out.jumps, n) {
                        if crosses_break(i, end, hard_breaks) {
                            continue;
                        }
                        let depth = count_syl_depth(i, end, &out.jumps);
                        let key = composing[i..end].to_string();
                        if is_orphan_final(&key) {
                            continue;
                        }
                        out.spans.push(SpanHyp {
                            start: i,
                            end,
                            kind: if depth <= 1 {
                                HypKind::MonoSyl
                            } else {
                                HypKind::MultiSyl
                            },
                            key,
                        });
                    }
                }
            }
            if mode == SegmentMode::FullFine {
                push_intra_under(&mut out.spans, composing, 0, first_end);
            }
        }
    }

    out.spans.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(a.end.cmp(&b.end))
            .then((a.kind as u8).cmp(&(b.kind as u8)))
    });
    out.spans
        .dedup_by(|a, b| a.start == b.start && a.end == b.end && a.kind == b.kind);
    out
}

fn crosses_break(start: usize, end: usize, breaks: &[usize]) -> bool {
    breaks.iter().any(|&b| start < b && b < end)
}

fn syllable_jumps_respecting_breaks(
    input: &str,
    table: &[String],
    hard_breaks: &[usize],
) -> Vec<Vec<usize>> {
    let n = input.len();
    let mut jumps = vec![Vec::new(); n + 1];
    for i in 0..n {
        let rest = &input[i..];
        for syl in table {
            if syl.is_empty() || !rest.starts_with(syl.as_str()) {
                continue;
            }
            let end = i + syl.len();
            if crosses_break(i, end, hard_breaks) {
                continue;
            }
            jumps[i].push(end);
        }
        jumps[i].sort_unstable();
        jumps[i].dedup();
    }
    jumps
}

fn is_fully_reachable(jumps: &[Vec<usize>], n: usize) -> bool {
    if n == 0 {
        return false;
    }
    let mut reach = vec![false; n + 1];
    reach[0] = true;
    for i in 0..n {
        if !reach[i] {
            continue;
        }
        for &j in &jumps[i] {
            if j <= n {
                reach[j] = true;
            }
        }
    }
    reach[n]
}

fn score_path(cuts: &[usize], n: usize) -> f32 {
    if cuts.is_empty() || *cuts.last().unwrap_or(&0) != n {
        return -1e9;
    }
    let mut prev = 0usize;
    let mut score = 0.0f32;
    let mut lens = Vec::new();
    for &c in cuts {
        let len = c - prev;
        lens.push(len as f32);
        // Prefer syllable lengths 2–4.
        score += match len {
            2..=4 => 2.0,
            1 => 0.5,
            5..=6 => 1.0,
            _ => -1.0,
        };
        prev = c;
    }
    // Uniformity bonus
    if lens.len() > 1 {
        let mean = lens.iter().sum::<f32>() / lens.len() as f32;
        let var: f32 = lens.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / lens.len() as f32;
        score -= var * 0.1;
    }
    score - cuts.len() as f32 * 0.01
}

fn enumerate_paths(
    jumps: &[Vec<usize>],
    n: usize,
    cap: usize,
    hard_breaks: &[usize],
) -> Vec<SegPath> {
    if cap == 0 || n == 0 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    let mut stack: Vec<(usize, Vec<usize>)> = vec![(0, Vec::new())];
    while let Some((pos, cuts)) = stack.pop() {
        if paths.len() >= cap.saturating_mul(4) {
            break;
        }
        if pos == n {
            let score = score_path(&cuts, n);
            // Must hit every hard break as a cut.
            if hard_breaks.iter().all(|b| cuts.contains(b)) {
                paths.push(SegPath { cuts, score });
            }
            continue;
        }
        if cuts.len() > MAX_SYL_PER_MULTI as usize {
            continue;
        }
        for &nxt in jumps.get(pos).into_iter().flatten() {
            if nxt <= pos || nxt > n {
                continue;
            }
            let mut next_cuts = cuts.clone();
            next_cuts.push(nxt);
            stack.push((nxt, next_cuts));
        }
    }
    paths.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    paths.truncate(cap);
    paths
}

fn push_mono_at(spans: &mut Vec<SpanHyp>, composing: &str, start: usize, end: usize) {
    if end <= start || end > composing.len() {
        return;
    }
    let key = composing[start..end].to_string();
    if is_orphan_final(&key) {
        return;
    }
    spans.push(SpanHyp {
        start,
        end,
        kind: HypKind::MonoSyl,
        key,
    });
}

fn push_intra_under(spans: &mut Vec<SpanHyp>, composing: &str, start: usize, first_end: usize) {
    if first_end <= start + 1 {
        return;
    }
    let bytes = composing.as_bytes();
    let mut i = start;
    while i < first_end {
        if !(bytes[i] as char).is_ascii_lowercase() {
            break;
        }
        let key = composing[i..i + 1].to_string();
        if !is_orphan_final(&key) {
            spans.push(SpanHyp {
                start: i,
                end: i + 1,
                kind: HypKind::Initial,
                key,
            });
        }
        for end in (i + 2)..=first_end {
            if i == start && end == first_end {
                continue;
            }
            let key = composing[i..end].to_string();
            if is_orphan_final(&key) {
                continue;
            }
            spans.push(SpanHyp {
                start: i,
                end,
                kind: HypKind::IntraSyl,
                key,
            });
        }
        i += 1;
    }
}

fn span_ends(start: usize, jumps: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut stack = vec![(start, 0u8)];
    let mut seen = std::collections::HashSet::<(usize, u8)>::new();
    while let Some((pos, depth)) = stack.pop() {
        if depth > 0 {
            ends.push(pos);
        }
        if depth >= MAX_SYL_PER_MULTI {
            continue;
        }
        for &nxt in jumps.get(pos).into_iter().flatten() {
            if nxt <= n && seen.insert((nxt, depth + 1)) {
                stack.push((nxt, depth + 1));
            }
        }
    }
    ends.sort_unstable();
    ends.dedup();
    ends
}

fn count_syl_depth(start: usize, end: usize, jumps: &[Vec<usize>]) -> u8 {
    let mut dist = vec![None; end + 1];
    dist[start] = Some(0u8);
    let mut q = std::collections::VecDeque::new();
    q.push_back(start);
    while let Some(pos) = q.pop_front() {
        let d = dist[pos].unwrap_or(0);
        if pos == end {
            return d;
        }
        for &nxt in jumps.get(pos).into_iter().flatten() {
            if nxt > end {
                continue;
            }
            if dist[nxt].is_none() {
                dist[nxt] = Some(d.saturating_add(1));
                q.push_back(nxt);
            }
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liuchang_first_syl_mono_is_liu() {
        let syls = vec!["liu".into(), "chang".into()];
        let hyps = build_seg_hypotheses("liuchang", &syls, SegmentMode::FirstSyllable);
        assert!(
            hyps.spans.iter().any(|s| {
                s.kind == HypKind::MonoSyl && s.key == "liu" && s.code_len() == 3
            }),
            "{:?}",
            hyps.spans
        );
    }

    #[test]
    fn beijing_full_has_bei_and_beijing() {
        let syls = vec!["bei".into(), "jing".into()];
        let hyps = build_seg_hypotheses("beijing", &syls, SegmentMode::Full);
        assert!(hyps.spans.iter().any(|s| s.key == "bei" && s.end == 3));
        assert!(hyps.spans.iter().any(|s| s.key == "beijing" && s.end == 7));
    }

    #[test]
    fn xian_apostrophe_forces_xi_an() {
        let syls = vec!["xi".into(), "an".into(), "xian".into()];
        let hyps = build_seg_hypotheses_ex("xian", &syls, SegmentMode::Full, &[2]);
        assert!(
            hyps.paths.iter().any(|p| p.cuts == vec![2, 4]),
            "paths={:?}",
            hyps.paths
        );
    }
}
