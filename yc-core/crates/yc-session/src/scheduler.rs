use yc_engine::{EngineFactory, InputEngine, PinyinEngine};
use yc_types::{
    EditorId, EngineError, HotOutcome, ImmSnapshot, InputScheme, KeyboardLayout,
    Language, UiCommand, UserAction,
};

use crate::manager::SessionManager;

#[derive(Debug)]
pub struct Scheduler {
    factory: EngineFactory,
}

impl Scheduler {
    pub fn new(factory: EngineFactory) -> Self {
        Self { factory }
    }

    pub fn engine_mut(&mut self) -> &mut PinyinEngine {
        self.factory.engine_mut()
    }

    pub fn handle(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
        action: UserAction,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }

        self.factory.engine_mut().set_active(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();

        let step = match action {
            UserAction::Init => {
                self.factory.engine_mut().reset(editor_id);
                let composing = sessions.composing(editor_id);
                yc_types::EngineStep {
                    composing,
                    candidates: Vec::new(),
                    commands: Vec::new(),
                }
            }
            UserAction::KeyPress { key_code } => self
                .factory
                .engine_mut()
                .feed(editor_id, key_code, &input_mode)?,
            UserAction::Backspace => self.factory.engine_mut().backspace(editor_id)?,
            UserAction::SelectCandidate { candidate_id } => {
                self.factory.engine_mut().select(editor_id, candidate_id)?
            }
            UserAction::SwitchLayout { layout } => {
                return self.switch_layout(sessions, editor_id, layout);
            }
            UserAction::SwitchScheme { scheme } => {
                return self.switch_scheme(sessions, editor_id, scheme);
            }
            UserAction::ToggleAscii => return self.toggle_ascii(sessions, editor_id),
        };

        self.finish_step(sessions, editor_id, step)
    }

    pub fn switch_layout(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
        layout: KeyboardLayout,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        let mut mode = sessions.input_mode(editor_id).unwrap_or_default();
        if mode.forced_by_editor {
            return Err(EngineError::Unsupported);
        }
        mode.layout = layout;
        if layout == KeyboardLayout::Qwerty {
            mode.scheme = InputScheme::Qwerty;
            mode.lang = Language::En;
        }
        sessions.set_input_mode(editor_id, mode);
        self.apply_mode_change(sessions, editor_id)
    }

    pub fn switch_scheme(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
        scheme: InputScheme,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        let mut mode = sessions.input_mode(editor_id).unwrap_or_default();
        if mode.forced_by_editor {
            return Err(EngineError::Unsupported);
        }
        mode.scheme = scheme;
        match scheme {
            InputScheme::PinyinFull => {
                mode.layout = KeyboardLayout::Pinyin26;
                mode.lang = Language::Zh;
            }
            InputScheme::Qwerty => {
                mode.layout = KeyboardLayout::Qwerty;
                mode.lang = Language::En;
            }
        }
        sessions.set_input_mode(editor_id, mode);
        self.apply_mode_change(sessions, editor_id)
    }

    pub fn toggle_ascii(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        sessions.update_input_mode(editor_id, |mode| {
            mode.ascii_mode = !mode.ascii_mode;
        });
        self.apply_mode_change(sessions, editor_id)
    }

    pub fn restore_user_preference(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        sessions.restore_user_preference(editor_id);
        self.apply_mode_change(sessions, editor_id)
    }

    fn apply_mode_change(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        self.factory.engine_mut().set_active(editor_id);
        self.factory.engine_mut().reset(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();
        let layout = input_mode.layout;
        sessions.update_composing(editor_id, yc_types::ComposingText::empty());
        let seq = sessions.bump_seq(editor_id);
        let snapshot = ImmSnapshot {
            editor_id,
            seq,
            input_mode,
            composing: yc_types::ComposingText::empty(),
            candidates: Vec::new(),
            status_flags: 0,
        };
        Ok(HotOutcome {
            snapshot,
            commands: vec![UiCommand::ReloadKeyboard { layout }],
        })
    }

    fn finish_step(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
        step: yc_types::EngineStep,
    ) -> Result<HotOutcome, EngineError> {
        sessions.update_composing(editor_id, step.composing.clone());
        let seq = sessions.bump_seq(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();
        let snapshot = ImmSnapshot {
            editor_id,
            seq,
            input_mode,
            composing: step.composing,
            candidates: step.candidates,
            status_flags: 0,
        };
        Ok(HotOutcome {
            snapshot,
            commands: step.commands,
        })
    }

    pub fn on_session_created(&mut self, editor_id: EditorId) {
        self.factory.engine_mut().set_active(editor_id);
    }

    pub fn on_session_stopped(&mut self, editor_id: EditorId) {
        self.factory.engine_mut().remove_session(editor_id);
    }
}
