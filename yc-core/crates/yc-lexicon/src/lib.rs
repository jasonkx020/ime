//! Lexicon DAT binary format (YCLX v2) + mmap + compile from TSV.

mod dat;
mod pinyin_match;

pub use dat::{
    compile_merged_tsv, compile_tsv_to_dat, normalize_romanized, DatLexicon, LexiconManager,
    LEXICON_MAGIC, LEXICON_VERSION,
};
pub use pinyin_match::{is_valid_prefix, key_matches_composing, split_syllables};

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
