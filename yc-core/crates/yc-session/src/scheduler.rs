use yc_engine::{EngineFactory, InputEngine, PinyinEngine};
use yc_handwriting::HandwritingService;
use yc_types::{
    ComposingText, EditorId, EngineError, HotOutcome, ImmSnapshot, InputScheme, KeyboardLayout,
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
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
        action: UserAction,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }

        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();

        match action {
            UserAction::OpenHandwriting => {
                return self.open_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::DismissHandwriting => {
                return self.dismiss_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::PushStrokeBatch { batch } => {
                return self.push_stroke_batch(sessions, handwriting, editor_id, batch);
            }
            UserAction::RecognizeHandwriting => {
                return self.recognize_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::ClearHandwriting => {
                return self.clear_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::UndoHandwriting => {
                return self.undo_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::SelectCandidate { candidate_id }
                if input_mode.scheme == InputScheme::Handwriting =>
            {
                return self.select_hw_candidate(sessions, handwriting, editor_id, candidate_id);
            }
            UserAction::SwitchLayout { layout } => {
                return self.switch_layout(sessions, handwriting, editor_id, layout);
            }
            UserAction::SwitchScheme { scheme } => {
                return self.switch_scheme(sessions, handwriting, editor_id, scheme);
            }
            UserAction::ToggleAscii => return self.toggle_ascii(sessions, handwriting, editor_id),
            other => {
                self.factory.engine_mut().set_active(editor_id);
                let step = match other {
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
                    UserAction::SelectCandidate { candidate_id } => self
                        .factory
                        .engine_mut()
                        .select(editor_id, candidate_id)?,
                    _ => return Err(EngineError::Unsupported),
                };
                self.finish_step(sessions, editor_id, step)
            }
        }
    }

    fn open_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        let privacy = sessions
            .privacy_of(editor_id)
            .unwrap_or(yc_types::PrivacyLevel::Normal);
        if !handwriting.is_allowed(privacy) {
            return Err(EngineError::Unsupported);
        }
        let mut mode = sessions.input_mode(editor_id).unwrap_or_default();
        if mode.forced_by_editor {
            return Err(EngineError::Unsupported);
        }
        mode.scheme = InputScheme::Handwriting;
        mode.layout = KeyboardLayout::HandwritingPad;
        mode.lang = Language::Zh;
        sessions.set_input_mode(editor_id, mode);
        self.factory.engine_mut().reset(editor_id);
        sessions.update_composing(editor_id, ComposingText::empty());
        handwriting.begin(editor_id);
        self.hw_outcome(sessions, handwriting, editor_id, vec![UiCommand::ReloadKeyboard {
            layout: KeyboardLayout::HandwritingPad,
        }])
    }

    fn dismiss_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        handwriting.clear(editor_id)?;
        let mut mode = sessions.input_mode(editor_id).unwrap_or_default();
        mode.scheme = InputScheme::PinyinFull;
        mode.layout = KeyboardLayout::Pinyin26;
        mode.lang = Language::Zh;
        sessions.set_input_mode(editor_id, mode);
        self.apply_mode_change(sessions, handwriting, editor_id)
    }

    fn push_stroke_batch(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
        batch: yc_types::StrokeBatch,
    ) -> Result<HotOutcome, EngineError> {
        handwriting.push_batch(batch)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn recognize_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        let _result = handwriting.recognize(editor_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn clear_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        handwriting.clear(editor_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn undo_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        handwriting.undo(editor_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn select_hw_candidate(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
        candidate_id: u32,
    ) -> Result<HotOutcome, EngineError> {
        let commands = handwriting.select_candidate(editor_id, candidate_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, commands)
    }

    fn hw_outcome(
        &self,
        sessions: &mut SessionManager,
        handwriting: &HandwritingService,
        editor_id: EditorId,
        commands: Vec<UiCommand>,
    ) -> Result<HotOutcome, EngineError> {
        let seq = sessions.bump_seq(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();
        let snapshot = ImmSnapshot {
            editor_id,
            seq,
            input_mode,
            composing: ComposingText::empty(),
            candidates: handwriting.candidates(editor_id),
            status_flags: 0,
        };
        Ok(HotOutcome { snapshot, commands })
    }

    pub fn switch_layout(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
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
        } else if layout == KeyboardLayout::HandwritingPad {
            mode.scheme = InputScheme::Handwriting;
            mode.lang = Language::Zh;
            handwriting.begin(editor_id);
        }
        sessions.set_input_mode(editor_id, mode);
        self.apply_mode_change(sessions, handwriting, editor_id)
    }

    pub fn switch_scheme(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
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
            InputScheme::Handwriting => {
                let privacy = sessions
            .privacy_of(editor_id)
            .unwrap_or(yc_types::PrivacyLevel::Normal);
                if !handwriting.is_allowed(privacy) {
                    return Err(EngineError::Unsupported);
                }
                mode.layout = KeyboardLayout::HandwritingPad;
                mode.lang = Language::Zh;
                handwriting.begin(editor_id);
            }
        }
        sessions.set_input_mode(editor_id, mode);
        self.apply_mode_change(sessions, handwriting, editor_id)
    }

    pub fn toggle_ascii(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        sessions.update_input_mode(editor_id, |mode| {
            mode.ascii_mode = !mode.ascii_mode;
        });
        self.apply_mode_change(sessions, handwriting, editor_id)
    }

    pub fn restore_user_preference(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        if !sessions.validate(editor_id) {
            return Err(EngineError::SessionInvalid);
        }
        sessions.restore_user_preference(editor_id);
        self.apply_mode_change(sessions, handwriting, editor_id)
    }

    fn apply_mode_change(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        self.factory.engine_mut().set_active(editor_id);
        self.factory.engine_mut().reset(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();
        let layout = input_mode.layout;
        if input_mode.scheme != InputScheme::Handwriting {
            handwriting.remove_session(editor_id);
        }
        sessions.update_composing(editor_id, ComposingText::empty());
        let seq = sessions.bump_seq(editor_id);
        let snapshot = ImmSnapshot {
            editor_id,
            seq,
            input_mode,
            composing: ComposingText::empty(),
            candidates: if layout == KeyboardLayout::HandwritingPad {
                handwriting.candidates(editor_id)
            } else {
                Vec::new()
            },
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

    pub fn on_session_stopped(&mut self, editor_id: EditorId, handwriting: &mut HandwritingService) {
        self.factory.engine_mut().remove_session(editor_id);
        handwriting.remove_session(editor_id);
    }
}
