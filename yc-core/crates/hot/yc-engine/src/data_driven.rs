//! Data-driven engine: scheme transform + lexicon lookup + candidate paging.

use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{LexiconManager, UserWordStore};
use yc_scheme::SchemeDesc;
use yc_scheme::TransformKind;
use yc_types::{
    Candidate, ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
    MAX_CANDIDATES,
};

use crate::pinyin_seg::{is_valid_prefix, normalize_query};
use crate::{invalid_session, key_code_to_char, session_invalid, InputEngine};

#[derive(Debug)]
pub struct DataDrivenEngine {
    active: EditorId,
    composing: String,
    lexicon: LexiconManager,
    pack_id: String,
    scheme: SchemeDesc,
    /// Full ranked candidate pool (before paging).
    cand_pool: Vec<Candidate>,
    cand_page: u32,
    last_query_key: String,
}

impl DataDrivenEngine {
    pub fn new(pack_id: String, scheme: SchemeDesc) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
            scheme,
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

    /// Replace pool without resetting page (used after intel rerank of same query).
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

    fn transformed(&self, raw: &str) -> String {
        match self.scheme.transform {
            TransformKind::RuleChain => self.scheme.apply_rule_chain(raw),
            TransformKind::Table | TransformKind::LatinPredict => normalize_query(raw),
        }
    }

    fn paged_candidates(&self) -> Vec<Candidate> {
        page_slice(&self.cand_pool, self.cand_page)
    }

    fn step_from_pool(&mut self, composing: String) -> EngineStep {
        self.last_query_key = self.transformed(&composing);
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            // Full pool for scheduler rerank; ImmSnapshot will page.
            candidates: self.cand_pool.clone(),
            commands: Vec::new(),
        }
    }

    fn lookup(&self) -> Vec<Candidate> {
        let query = self.transformed(&self.composing);
        if self.scheme.transform == TransformKind::Table {
            self.lexicon.lookup_pinyin(&query, &self.scheme.syllables)
        } else {
            self.lexicon.lookup(&query)
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
            candidates: self.paged_candidates(),
            commands: Vec::new(),
        }
    }
}

impl InputEngine for DataDrivenEngine {
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
            let text = page_slice(&self.cand_pool, self.cand_page)
                .first()
                .map(|c| c.text.clone())
                .or_else(|| self.cand_pool.first().map(|c| c.text.clone()))
                .or_else(|| self.lookup().first().map(|c| c.text.clone()))
                .unwrap_or_else(|| self.composing.clone());
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
        if self.scheme.transform == TransformKind::Table
            && !is_valid_prefix(&self.composing, &self.scheme.syllables)
        {
            self.composing.pop();
            return Err(EngineError::Unsupported);
        }
        self.cand_pool = self.lookup();
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
        self.cand_pool = self.lookup();
        self.cand_page = 0;
        Ok(self.step_from_pool(self.composing.clone()))
    }
}

pub(crate) fn page_count(pool: &[Candidate]) -> u32 {
    if pool.is_empty() {
        0
    } else {
        ((pool.len() + MAX_CANDIDATES - 1) / MAX_CANDIDATES) as u32
    }
}

pub(crate) fn page_slice(pool: &[Candidate], page: u32) -> Vec<Candidate> {
    let start = page as usize * MAX_CANDIDATES;
    if start >= pool.len() {
        return Vec::new();
    }
    pool[start..]
        .iter()
        .take(MAX_CANDIDATES)
        .enumerate()
        .map(|(i, c)| Candidate {
            id: i as u32,
            text: c.text.clone(),
            source: c.source,
            score: c.score,
        })
        .collect()
}
