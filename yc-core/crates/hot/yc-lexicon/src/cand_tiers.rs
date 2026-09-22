//! Candidate score **tier bands** (hints only). Final order is `rank::rank_candidates`.
//!
//! L0 whole-query hot words → L1 AI phrases → L2 segment compose →
//! L3 syllable hot words → L4 letter characters (freq-first).

/// L0: exact / user whole-key hits.
pub const SCORE_L0: f32 = 1.0;
/// L1: AI whole-query phrases (at most 5).
pub const SCORE_L1: f32 = 0.94;
/// L2: stitched sentences / path prefixes.
pub const SCORE_L2: f32 = 0.90;
/// L3: syllable-span lexicon words.
pub const SCORE_L3: f32 = 0.86;
/// L4: letter-split single characters.
pub const SCORE_L4: f32 = 0.78;

/// Max AI phrases injected for L1 (and each subsequent AI wave).
pub const AI_WAVE_CAP: usize = 5;

pub fn score_l0(rank: usize) -> f32 {
    SCORE_L0 - (rank as f32 * 0.001)
}

pub fn score_l1(rank: usize) -> f32 {
    SCORE_L1 - (rank as f32 * 0.005)
}

pub fn score_l2(quality: f32) -> f32 {
    (SCORE_L2 + (quality - 0.88) * 0.3).clamp(0.88, 0.919)
}

/// L2 score from lattice sums so relative rank among sentences is preserved.
pub fn score_l2_from_parts(sum_ln: f64, chars: u32, edges: u32) -> f32 {
    let n = edges.max(1) as f64;
    let q = sum_ln / n + 0.8 * (chars as f64 / n);
    // Typical q ~10–13; map into L2 band without collapsing ties.
    (SCORE_L2 + ((q - 10.0) as f32) * 0.006).clamp(0.88, 0.919)
}

pub fn score_l3(rank: usize, code_bytes: usize) -> f32 {
    SCORE_L3 - (rank as f32 * 0.001) - (code_bytes as f32 * 0.0003)
}

pub fn score_l4(rank: usize) -> f32 {
    SCORE_L4 - (rank as f32 * 0.002)
}
