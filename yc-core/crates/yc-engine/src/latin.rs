//! Latin predict engine for OTA langpacks (vi/id/ms).

use yc_lexicon::LexiconManager;
use yc_types::{
    ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
};

use crate::{invalid_session, key_code_to_char, session_invalid, InputEngine};

#[derive(Debug)]
pub struct LatinPredictEngine {
    active: EditorId,
    composing: String,
    lexicon: LexiconManager,
    pack_id: String,
}

impl LatinPredictEngine {
    pub fn new(pack_id: String) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
        }
    }

    pub fn load_lexicon(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        self.lexicon.open_lang(pack_id, path)?;
        self.lexicon.set_active(pack_id);
        self.pack_id = pack_id.to_string();
        Ok(())
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
}

impl InputEngine for LatinPredictEngine {
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
            let text = self.composing.clone();
            self.composing.clear();
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::Commit { text }],
            });
        }
        let ch = key_code_to_char(key_code).ok_or(EngineError::Unsupported)?;
        self.composing.push(ch);
        let cands = self.lexicon.lookup(&self.composing);
        Ok(self.step(self.composing.clone(), cands))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let cands = self.lexicon.lookup(&self.composing);
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
        let cands = self.lexicon.lookup(&self.composing);
        Ok(self.step(self.composing.clone(), cands))
    }
}
