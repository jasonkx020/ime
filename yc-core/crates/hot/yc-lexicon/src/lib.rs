//! Lexicon DAT binary format (YCLX v2) + mmap + compile from TSV.

mod dat;
mod fuzzy;
mod lookup_opts;
mod pinyin_match;
mod segment;
mod typo;
mod user_words;

pub use dat::{
    clears_assoc_context, compile_merged_tsv, compile_tsv_to_dat, normalize_lookup_key,
    normalize_romanized, romanize_latin,
    CharNgramModel, DatLexicon, LexiconManager, SharedCharNgram, ASSOC_MAX_SUFFIX_CHARS,
    LEXICON_MAGIC, LEXICON_VERSION,
};
pub use lookup_opts::{LookupCancel, LookupOpts};
pub use pinyin_match::{
    is_complete_syllable, is_valid_pinyin_input, is_valid_prefix, key_matches_composing,
    key_matches_jianpin, mixed_slots, split_syllables, MixSlot,
};
pub use typo::adjacent_typo_variants;
pub use user_words::{merge_user_boosts, merge_user_boosts_lang, UserWordStore};

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
