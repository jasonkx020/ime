use yc_engine::EngineFactory;
use yc_handwriting::HandwritingService;
use yc_intel::{LightIntel, UserBoostIntel};
use yc_types::{
    ComposingText, EditorId, EngineError, HotOutcome, ImmSnapshot, InputScheme, KeyboardLayout,
    Language, PrivacyLevel, UiCommand, UserAction,
};

use crate::langpack::EnabledLangPack;
use crate::manager::SessionManager;

pub struct Scheduler {
    factory: EngineFactory,
    enabled_packs: Vec<EnabledLangPack>,
    intel: Box<dyn LightIntel>,
}

impl Scheduler {
    pub fn new(factory: EngineFactory) -> Self {
        let store = factory.user_words();
        Self {
            factory,
            enabled_packs: Vec::new(),
            intel: Box::new(UserBoostIntel::new(store)),
        }
    }

    pub fn with_intel(factory: EngineFactory, intel: Box<dyn LightIntel>) -> Self {
        Self {
            factory,
            enabled_packs: Vec::new(),
            intel,
        }
    }

    pub fn factory_mut(&mut self) -> &mut EngineFactory {
        &mut self.factory
    }

    pub fn set_enabled_packs(&mut self, packs: Vec<EnabledLangPack>) {
        self.enabled_packs = packs;
    }

    pub fn enabled_packs(&self) -> &[EnabledLangPack] {
        &self.enabled_packs
    }

    pub fn on_pack_disabled(&mut self, pack_id: &str) {
        self.factory.unregister(pack_id);
        self.enabled_packs.retain(|p| p.pack_id != pack_id);
        if self.factory.active_pack_id() == Some(pack_id) {
            self.factory.set_active_pack(None);
        }
    }

    fn find_zh_pack(&self) -> Option<&EnabledLangPack> {
        self.enabled_packs
            .iter()
            .find(|p| p.lang_tag == "zh" || p.pack_id.contains("zh"))
    }

    fn activate_zh_pinyin(&mut self, mode: &mut yc_types::InputMode, layout_id: &str) -> Result<(), EngineError> {
        let pack = self.find_zh_pack().ok_or(EngineError::Unsupported)?.clone();
        self.factory
            .create(&pack.pack_id, "pinyin_full")?;
        mode.lang_tag = pack.lang_tag.clone();
        mode.active_pack_id = pack.pack_id.clone();
        mode.scheme_id = "pinyin_full".into();
        mode.layout_id = layout_id.into();
        mode.scheme = InputScheme::PinyinFull;
        mode.lang = Language::Zh;
        Ok(())
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
            UserAction::ConfirmCloudHandwriting => {
                return self.confirm_cloud_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::DismissCloudHandwriting => {
                return self.dismiss_cloud_handwriting(sessions, handwriting, editor_id);
            }
            UserAction::SwitchLang { pack_id_hash } => {
                return self.switch_lang(sessions, handwriting, editor_id, pack_id_hash);
            }
            other => {
                self.factory.set_active_editor(editor_id);
                let learn_key = match &other {
                    UserAction::SelectCandidate { .. } => self.factory.active_query_key(),
                    UserAction::KeyPress { key_code } if *key_code == b' ' as u32 => {
                        self.factory.active_query_key()
                    }
                    _ => String::new(),
                };
                let step = match other {
                    UserAction::Init => {
                        self.factory.reset_active(editor_id);
                        sessions.update_composing(editor_id, ComposingText::empty());
                        yc_types::EngineStep {
                            composing: ComposingText::empty(),
                            candidates: Vec::new(),
                            commands: Vec::new(),
                        }
                    }
                    UserAction::KeyPress { key_code } => {
                        self.factory.feed_active(editor_id, key_code, &input_mode)?
                    }
                    UserAction::Backspace => self.factory.backspace_active(editor_id)?,
                    UserAction::SelectCandidate { candidate_id } => {
                        self.factory.select_active(editor_id, candidate_id)?
                    }
                    _ => return Err(EngineError::Unsupported),
                };
                self.finish_step(sessions, editor_id, step, learn_key)
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
        self.factory.reset_active(editor_id);
        sessions.update_composing(editor_id, ComposingText::empty());
        handwriting.begin(editor_id);
        self.hw_outcome(sessions, handwriting, editor_id, vec![UiCommand::ReloadKeyboard {
            layout: KeyboardLayout::HandwritingPad,
            layout_id: "layout_handwriting".into(),
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
        let _ = self.activate_zh_pinyin(&mut mode, "layout_pinyin26");
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
        let privacy = sessions
            .privacy_of(editor_id)
            .unwrap_or(yc_types::PrivacyLevel::Normal);
        let _result = handwriting.recognize(editor_id, privacy)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn confirm_cloud_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        let privacy = sessions
            .privacy_of(editor_id)
            .unwrap_or(yc_types::PrivacyLevel::Normal);
        if privacy == yc_types::PrivacyLevel::ForbiddenCloud {
            return Err(EngineError::Unsupported);
        }
        handwriting.confirm_cloud(editor_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn dismiss_cloud_handwriting(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
    ) -> Result<HotOutcome, EngineError> {
        handwriting.dismiss_cloud(editor_id)?;
        self.hw_outcome(sessions, handwriting, editor_id, Vec::new())
    }

    fn switch_lang(
        &mut self,
        sessions: &mut SessionManager,
        handwriting: &mut HandwritingService,
        editor_id: EditorId,
        pack_id_hash: u32,
    ) -> Result<HotOutcome, EngineError> {
        let pack = self
            .enabled_packs
            .iter()
            .find(|p| p.pack_id_hash() == pack_id_hash)
            .cloned()
            .ok_or(EngineError::Unsupported)?;
        self.factory.set_active_pack(Some(pack.pack_id.clone()));
        self.factory.reset_active(editor_id);
        let mut mode = sessions.input_mode(editor_id).unwrap_or_default();
        mode.lang_tag = pack.lang_tag.clone();
        mode.active_pack_id = pack.pack_id.clone();
        mode.scheme_id = pack.default_scheme_id.clone();
        mode.layout_id = pack.default_layout_id.clone();
        mode.scheme = InputScheme::Qwerty;
        mode.layout = KeyboardLayout::Qwerty;
        mode.lang = Language::En;
        sessions.set_input_mode(editor_id, mode);
        sessions.update_composing(editor_id, ComposingText::empty());
        handwriting.remove_session(editor_id);
        self.apply_mode_change(sessions, handwriting, editor_id)
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
        let status_flags = if handwriting.pending_cloud(editor_id) {
            1
        } else {
            0
        };
        let snapshot = ImmSnapshot {
            editor_id,
            seq,
            input_mode,
            composing: ComposingText::empty(),
            candidates: handwriting.candidates(editor_id),
            status_flags,
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
        match layout {
            KeyboardLayout::Qwerty => {
                mode.scheme = InputScheme::Qwerty;
                mode.lang = Language::En;
            }
            KeyboardLayout::HandwritingPad => {
                mode.scheme = InputScheme::Handwriting;
                mode.lang = Language::Zh;
                handwriting.begin(editor_id);
            }
            KeyboardLayout::Pinyin26 | KeyboardLayout::Numeric | KeyboardLayout::Symbol => {
                mode.layout = layout;
                let layout_id = match layout {
                    KeyboardLayout::Numeric => "layout_numeric",
                    KeyboardLayout::Symbol => "layout_symbol",
                    _ => "layout_pinyin26",
                };
                self.activate_zh_pinyin(&mut mode, layout_id)?;
            }
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
        if !mode.active_pack_id.is_empty() {
            let scheme_id = match scheme {
                InputScheme::Qwerty => {
                    mode.layout = KeyboardLayout::Qwerty;
                    mode.lang = Language::En;
                    "latin"
                }
                InputScheme::PinyinFull => "pinyin_full",
                InputScheme::Handwriting => {
                    return self.open_handwriting(sessions, handwriting, editor_id)
                }
            };
            self.factory
                .create(&mode.active_pack_id, scheme_id)
                .ok();
            mode.scheme_id = scheme_id.to_string();
            if scheme == InputScheme::PinyinFull {
                mode.layout = KeyboardLayout::Pinyin26;
                mode.lang = Language::Zh;
            }
        } else {
            match scheme {
                InputScheme::PinyinFull => {
                    mode.layout = KeyboardLayout::Pinyin26;
                    self.activate_zh_pinyin(&mut mode, "layout_pinyin26")?;
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
        self.factory.set_active_editor(editor_id);
        self.factory.reset_active(editor_id);
        let input_mode = sessions.input_mode(editor_id).unwrap_or_default();
        let layout = input_mode.layout;
        let layout_id = input_mode.layout_id.clone();
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
            commands: vec![UiCommand::ReloadKeyboard { layout, layout_id }],
        })
    }

    fn finish_step(
        &mut self,
        sessions: &mut SessionManager,
        editor_id: EditorId,
        mut step: yc_types::EngineStep,
        learn_key: String,
    ) -> Result<HotOutcome, EngineError> {
        let privacy = sessions
            .privacy_of(editor_id)
            .unwrap_or(PrivacyLevel::Normal);

        if !step.candidates.is_empty() {
            let prefix = step.composing.text.clone();
            let cands = std::mem::take(&mut step.candidates);
            if let Ok(ranked) = self.intel.rerank(&prefix, cands) {
                step.candidates = ranked;
                self.factory
                    .update_active_candidates(step.candidates.clone());
            }
        }

        if privacy == PrivacyLevel::Normal {
            for cmd in &step.commands {
                if let UiCommand::Commit { text } = cmd {
                    if !learn_key.is_empty() && !text.is_empty() {
                        self.factory.touch_user_word(&learn_key, text);
                    }
                }
            }
        }

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
        self.factory.set_active_editor(editor_id);
    }

    pub fn on_session_stopped(&mut self, editor_id: EditorId, handwriting: &mut HandwritingService) {
        self.factory.remove_active_session(editor_id);
        handwriting.remove_session(editor_id);
    }
}
