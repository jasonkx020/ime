use std::collections::HashMap;

use yc_lexicon::InMemoryLexicon;
use yc_types::{
    Candidate, ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode,
    KeyboardLayout, UiCommand,
};

use crate::{invalid_session, key_code_to_char, session_invalid, InputEngine};

#[derive(Debug)]
struct SessionState {
    composing: ComposingText,
    candidates: Vec<Candidate>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            composing: ComposingText::empty(),
            candidates: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct PinyinEngine {
    lexicon: InMemoryLexicon,
    sessions: HashMap<u64, SessionState>,
    active_editor: EditorId,
}

impl PinyinEngine {
    pub fn new() -> Self {
        Self {
            lexicon: InMemoryLexicon::new(),
            sessions: HashMap::new(),
            active_editor: EditorId::NONE,
        }
    }

    pub fn set_active(&mut self, editor_id: EditorId) {
        self.active_editor = editor_id;
        if editor_id != EditorId::NONE {
            self.sessions
                .entry(editor_id.raw())
                .or_insert_with(SessionState::new);
        }
    }

    pub fn remove_session(&mut self, editor_id: EditorId) {
        self.sessions.remove(&editor_id.raw());
        if self.active_editor == editor_id {
            self.active_editor = EditorId::NONE;
        }
    }

    fn state_mut(&mut self, editor_id: EditorId) -> HotResult<&mut SessionState> {
        if invalid_session(editor_id, self.active_editor) {
            return session_invalid();
        }
        self.sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)
    }

    fn to_step(state: &SessionState, commands: Vec<UiCommand>) -> EngineStep {
        EngineStep {
            composing: state.composing.clone(),
            candidates: state.candidates.clone(),
            commands,
        }
    }

    fn feed_ascii(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
    ) -> HotResult<EngineStep> {
        let ch = key_code_to_char(key_code).ok_or(EngineError::Unsupported)?;
        let commands = vec![UiCommand::Commit {
            text: ch.to_string(),
        }];
        let state = self.state_mut(editor_id)?;
        Ok(Self::to_step(state, commands))
    }

    fn feed_numeric(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
    ) -> HotResult<EngineStep> {
        if !(b'0'..=b'9').contains(&(key_code as u8)) {
            return Err(EngineError::Unsupported);
        }
        let ch = key_code as u8 as char;
        let commands = vec![UiCommand::Commit {
            text: ch.to_string(),
        }];
        let state = self.state_mut(editor_id)?;
        Ok(Self::to_step(state, commands))
    }
}

impl Default for PinyinEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InputEngine for PinyinEngine {
    fn reset(&mut self, editor_id: EditorId) {
        if let Some(state) = self.sessions.get_mut(&editor_id.raw()) {
            state.composing = ComposingText::empty();
            state.candidates.clear();
        }
    }

    fn feed(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
        input_mode: &InputMode,
    ) -> HotResult<EngineStep> {
        if input_mode.ascii_mode {
            return self.feed_ascii(editor_id, key_code);
        }
        if input_mode.layout == KeyboardLayout::Numeric {
            return self.feed_numeric(editor_id, key_code);
        }

        let ch = key_code_to_char(key_code).ok_or(EngineError::Unsupported)?;
        let prefix = {
            let state = self.state_mut(editor_id)?;
            state.composing.text.push(ch);
            state.composing.cursor = state.composing.text.len() as u32;
            state.composing.text.clone()
        };
        let candidates = self.lexicon.lookup(&prefix);
        let state = self.state_mut(editor_id)?;
        state.candidates = candidates;
        Ok(Self::to_step(state, Vec::new()))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        let state = self.state_mut(editor_id)?;
        let text = state
            .candidates
            .iter()
            .find(|c| c.id == candidate_id)
            .map(|c| c.text.clone())
            .ok_or(EngineError::Unsupported)?;
        let commands = vec![UiCommand::Commit { text }];
        state.composing = ComposingText::empty();
        state.candidates.clear();
        Ok(Self::to_step(state, commands))
    }

    fn backspace(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        let prefix = {
            let state = self.state_mut(editor_id)?;
            state.composing.text.pop();
            state.composing.cursor = state.composing.text.len() as u32;
            state.composing.text.clone()
        };
        let candidates = self.lexicon.lookup(&prefix);
        let state = self.state_mut(editor_id)?;
        state.candidates = candidates;
        Ok(Self::to_step(state, Vec::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::{ComposingText, EditorId};

    fn press(engine: &mut PinyinEngine, id: EditorId, ch: char) {
        engine.set_active(id);
        engine
            .feed(id, ch as u32, &InputMode::default())
            .unwrap();
    }

    #[test]
    fn nihao_candidates() {
        let mut engine = PinyinEngine::new();
        let id = EditorId(1);
        engine.set_active(id);
        let mut last = EngineStep {
            composing: ComposingText::empty(),
            candidates: Vec::new(),
            commands: Vec::new(),
        };
        for ch in "nihao".chars() {
            last = engine.feed(id, ch as u32, &InputMode::default()).unwrap();
        }
        assert!(last.candidates.iter().any(|c| c.text == "你好"));
    }

    #[test]
    fn select_commits() {
        let mut engine = PinyinEngine::new();
        let id = EditorId(1);
        for ch in "nihao".chars() {
            press(&mut engine, id, ch);
        }
        let step = engine.select(id, 0).unwrap();
        assert!(matches!(
            step.commands.first(),
            Some(UiCommand::Commit { text }) if text == "你好"
        ));
        assert!(step.composing.text.is_empty());
    }

    #[test]
    fn ascii_mode_commits_letter() {
        let mut engine = PinyinEngine::new();
        let id = EditorId(1);
        engine.set_active(id);
        let mode = InputMode {
            ascii_mode: true,
            ..InputMode::default()
        };
        let step = engine.feed(id, b'a' as u32, &mode).unwrap();
        assert!(matches!(
            step.commands.first(),
            Some(UiCommand::Commit { text }) if text == "a"
        ));
    }

    #[test]
    fn numeric_layout_commits_digit() {
        let mut engine = PinyinEngine::new();
        let id = EditorId(1);
        engine.set_active(id);
        let mode = InputMode {
            layout: KeyboardLayout::Numeric,
            ..InputMode::default()
        };
        let step = engine.feed(id, b'5' as u32, &mode).unwrap();
        assert!(matches!(
            step.commands.first(),
            Some(UiCommand::Commit { text }) if text == "5"
        ));
    }
}
