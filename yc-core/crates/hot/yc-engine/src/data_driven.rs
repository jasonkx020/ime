//! Data-driven engine: scheme transform + lexicon lookup.

use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{LexiconManager, UserWordStore};
use yc_scheme::SchemeDesc;
use yc_scheme::TransformKind;
use yc_types::{
    Candidate, ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
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
    last_candidates: Vec<Candidate>,
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
            last_candidates: Vec::new(),
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

    pub fn set_last_candidates(&mut self, cands: Vec<Candidate>) {
        self.last_candidates = cands;
    }

    pub fn last_query_key(&self) -> &str {
        &self.last_query_key
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

    fn step(&mut self, composing: String, candidates: Vec<Candidate>) -> EngineStep {
        self.last_query_key = self.transformed(&composing);
        self.last_candidates = candidates.clone();
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            candidates,
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
}

impl InputEngine for DataDrivenEngine {
    fn reset(&mut self, editor_id: EditorId) {
        self.active = editor_id;
        self.composing.clear();
        self.last_candidates.clear();
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
            let text = if let Some(first) = self.last_candidates.first() {
                first.text.clone()
            } else {
                self.lookup()
                    .first()
                    .map(|c| c.text.clone())
                    .unwrap_or_else(|| self.composing.clone())
            };
            self.composing.clear();
            self.last_candidates.clear();
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
        let cands = self.lookup();
        Ok(self.step(self.composing.clone(), cands))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let text = self
            .last_candidates
            .iter()
            .find(|c| c.id == candidate_id)
            .map(|c| c.text.clone())
            .or_else(|| {
                self.lookup()
                    .into_iter()
                    .find(|c| c.id == candidate_id)
                    .map(|c| c.text)
            })
            .ok_or(EngineError::Unsupported)?;
        self.composing.clear();
        self.last_candidates.clear();
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
        let cands = self.lookup();
        Ok(self.step(self.composing.clone(), cands))
    }
}
