//! Latin predict engine for OTA langpacks (vi/id/ms).

use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{LexiconManager, UserWordStore};
use yc_types::{
    Candidate, ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
    MAX_CANDIDATES,
};

use crate::data_driven::{page_count, page_slice};
use crate::{invalid_session, key_code_to_char, session_invalid, InputEngine};

#[derive(Debug)]
pub struct LatinPredictEngine {
    active: EditorId,
    composing: String,
    lexicon: LexiconManager,
    pack_id: String,
    cand_pool: Vec<Candidate>,
    cand_page: u32,
    last_query_key: String,
}

impl LatinPredictEngine {
    pub fn new(pack_id: String) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
            cand_pool: Vec::new(),
            cand_page: 0,
            last_query_key: String::new(),
        }
    }

    pub fn load_lexicon(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        self.lexicon.open_lang(pack_id, path)?;
        self.lexicon.set_active(pack_id);
        self.pack_id = pack_id.to_string();
        Ok(())
    }

    pub fn set_user_words(&mut self, store: Arc<Mutex<UserWordStore>>) {
        self.lexicon.set_user_words(store);
    }

    pub fn set_cand_pool(&mut self, cands: Vec<Candidate>) {
        self.cand_pool = cands;
        self.cand_page = 0;
    }

    pub fn replace_cand_pool_keep_page(&mut self, cands: Vec<Candidate>) {
        let max_page = page_count(&cands).saturating_sub(1);
        self.cand_pool = cands;
        if self.cand_page > max_page {
            self.cand_page = max_page;
        }
    }

    pub fn set_last_candidates(&mut self, cands: Vec<Candidate>) {
        self.set_cand_pool(cands);
    }

    pub fn last_query_key(&self) -> &str {
        &self.last_query_key
    }

    pub fn cand_page(&self) -> u32 {
        self.cand_page
    }

    pub fn cand_total(&self) -> u32 {
        self.cand_pool.len() as u32
    }

    pub fn touch_user_word(&self, pinyin: &str, word: &str) {
        self.lexicon.touch_user_word(pinyin, word);
    }

    fn step_from_pool(&mut self, composing: String) -> EngineStep {
        self.last_query_key = composing.clone();
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            candidates: self.cand_pool.clone(),
            commands: Vec::new(),
        }
    }

    pub fn page_next(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let pages = page_count(&self.cand_pool);
        if pages == 0 {
            return Err(EngineError::Unsupported);
        }
        if self.cand_page + 1 < pages {
            self.cand_page += 1;
        }
        Ok(self.step_from_pool(self.composing.clone()))
    }

    pub fn page_prev(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        if self.cand_pool.is_empty() {
            return Err(EngineError::Unsupported);
        }
        if self.cand_page > 0 {
            self.cand_page -= 1;
        }
        Ok(self.step_from_pool(self.composing.clone()))
    }

    pub fn current_paged_step(&self) -> EngineStep {
        EngineStep {
            composing: ComposingText {
                text: self.composing.clone(),
                cursor: self.composing.len() as u32,
            },
            candidates: page_slice(&self.cand_pool, self.cand_page),
            commands: Vec::new(),
        }
    }
}

impl InputEngine for LatinPredictEngine {
    fn reset(&mut self, editor_id: EditorId) {
        self.active = editor_id;
        self.composing.clear();
        self.cand_pool.clear();
        self.cand_page = 0;
        self.last_query_key.clear();
    }

    fn feed(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
        _input_mode: &InputMode,
    ) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        if key_code == b' ' as u32 {
            let text = self.composing.clone();
            self.composing.clear();
            self.cand_pool.clear();
            self.cand_page = 0;
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::Commit { text }],
            });
        }
        let ch = key_code_to_char(key_code).ok_or(EngineError::Unsupported)?;
        self.composing.push(ch);
        self.cand_pool = self.lexicon.lookup(&self.composing);
        self.cand_page = 0;
        Ok(self.step_from_pool(self.composing.clone()))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let global = self.cand_page * MAX_CANDIDATES as u32 + candidate_id;
        let text = self
            .cand_pool
            .get(global as usize)
            .map(|c| c.text.clone())
            .or_else(|| {
                page_slice(&self.cand_pool, self.cand_page)
                    .into_iter()
                    .find(|c| c.id == candidate_id)
                    .map(|c| c.text)
            })
            .ok_or(EngineError::Unsupported)?;
        self.composing.clear();
        self.cand_pool.clear();
        self.cand_page = 0;
        Ok(EngineStep {
            composing: ComposingText::empty(),
            candidates: Vec::new(),
            commands: vec![UiCommand::Commit { text }],
        })
    }

    fn backspace(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        self.composing.pop();
        self.cand_pool = self.lexicon.lookup(&self.composing);
        self.cand_page = 0;
        Ok(self.step_from_pool(self.composing.clone()))
    }
}
