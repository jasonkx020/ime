//! Lexicon DAT binary format (YCLX v2) + mmap + compile from TSV.

mod cand_gen;
mod cand_tiers;
mod correction;
mod dat;
mod fuzzy;
mod lookup_opts;
mod pinyin_match;
mod preprocess;
mod rank;
mod seg_hypotheses;
mod segment;
mod span_resolve;
mod typo;
mod user_words;

pub use cand_tiers::{
    score_l0, score_l1, score_l2, score_l2_from_parts, score_l3, score_l4, AI_WAVE_CAP, SCORE_L0,
    SCORE_L1, SCORE_L2, SCORE_L3, SCORE_L4,
};

pub use correction::{
    adjacent_typo_variants_with, correction_variants, edit1_variants, fuzzy_variants_with,
    CorrectionTables, SharedCorrectionTables, CORRECTION_SCORE_CAP,
};
pub use dat::{
    clears_assoc_context, compile_merged_tsv, compile_tsv_to_dat, normalize_lookup_key,
    normalize_romanized, romanize_latin, CharNgramModel, DatLexicon, LexiconManager,
    SharedCharNgram, ASSOC_MAX_SUFFIX_CHARS, LEXICON_MAGIC, LEXICON_VERSION,
};
pub use lookup_opts::{LookupCancel, LookupOpts};
pub use pinyin_match::{
    is_complete_syllable, is_orphan_final, is_valid_pinyin_input, is_valid_prefix,
    key_matches_composing, key_matches_jianpin, mixed_slots, split_syllables,
    syllable_start_anchors, MixSlot,
};
pub use preprocess::{preprocess_pinyin, InputShape, PreprocessOut};
pub use rank::{
    apply_ngram_nudge, as_correction_score, is_correction_cand, rank_candidates, rank_fuse,
    user_boost_delta, RankCtx, USER_EXACT_BASE, USER_FREQ_WEIGHT,
};
pub use seg_hypotheses::{
    build_seg_hypotheses, build_seg_hypotheses_ex, HypKind, SegHypotheses, SegPath, SegSeverity,
    SegmentMode, SpanHyp,
};
pub use span_resolve::{
    attach_spans, infer_code_len, map_variant_span_to_original, resolve_code_len,
    resolve_code_len_via_variant, span_key_variants, tighten_code_lens, SYL_PAIR_VARIANTS,
};
pub use typo::adjacent_typo_variants;
pub use user_words::{
    decayed_freq, merge_user_boosts, merge_user_boosts_lang, merge_user_boosts_lang_syls,
    UserCorrectionStore, UserWordStore, USER_FREQ_HALF_LIFE_DAYS,
};

use yc_types::{Candidate, EngineError, HotResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LangLexiconHandle(pub u64);

pub trait Lexicon {
    fn lookup(&self, prefix: &str) -> Vec<Candidate>;

    fn open_lang(&mut self, _pack_id: &str, _path: &str) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }

    fn close_lang(&mut self, _pack_id: &str) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }
}

impl Lexicon for LexiconManager {
    fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        LexiconManager::lookup(self, prefix)
    }

    fn open_lang(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        LexiconManager::open_lang(self, pack_id, path)
    }

    fn close_lang(&mut self, pack_id: &str) -> HotResult<()> {
        LexiconManager::close_lang(self, pack_id)
    }
}
