//! In-memory lexicon (M1 placeholder; MMAP in later milestone).

mod memory;

pub use memory::InMemoryLexicon;

use yc_types::{Candidate, EngineError, HotResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LangLexiconHandle(pub u64);

pub trait Lexicon {
    fn lookup(&self, prefix: &str) -> Vec<Candidate>;

    fn open_lang(&mut self, _pack_id: &str, _path: &str) -> HotResult<LangLexiconHandle> {
        Err(EngineError::Unsupported)
    }

    fn close_lang(&mut self, _pack_id: &str) -> HotResult<()> {
        Err(EngineError::Unsupported)
    }
}

impl Lexicon for InMemoryLexicon {
    fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        memory::lookup_prefix(self, prefix)
    }
}
