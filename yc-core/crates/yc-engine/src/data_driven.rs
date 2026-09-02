//! Data-driven engine: scheme transform + lexicon lookup.

use yc_lexicon::LexiconManager;
use yc_scheme::TransformKind;
use yc_scheme::SchemeDesc;
use yc_types::{
    ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
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
}

impl DataDrivenEngine {
    pub fn new(pack_id: String, scheme: SchemeDesc) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
            scheme,
        }
    }

    pub fn load_lexicon(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        self.lexicon.open_lang(pack_id, path)?;
        self.lexicon.set_active(pack_id);
        self.pack_id = pack_id.to_string();
        Ok(())
    }

    fn transformed(&self, raw: &str) -> String {
        match self.scheme.transform {
            TransformKind::RuleChain => self.scheme.apply_rule_chain(raw),
            TransformKind::Table | TransformKind::LatinPredict => normalize_query(raw),
        }
    }

    fn step(&self, composing: String, candidates: Vec<yc_types::Candidate>) -> EngineStep {
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            candidates,
            commands: Vec::new(),
        }
    }

    fn lookup(&self) -> Vec<yc_types::Candidate> {
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
            let cands = self.lookup();
            let text = if let Some(first) = cands.first() {
                first.text.clone()
            } else {
                self.composing.clone()
            };
            self.composing.clear();
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
        let cands = self.lookup();
        let text = cands
            .iter()
            .find(|c| c.id == candidate_id)
            .map(|c| c.text.clone())
            .ok_or(EngineError::Unsupported)?;
        self.composing.clear();
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
