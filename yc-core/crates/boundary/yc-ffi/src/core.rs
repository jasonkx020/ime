use std::path::PathBuf;

#[cfg(feature = "data")]
use yc_data::ColdPathRuntime;
use yc_session::CoreServices;
use yc_types::{EditorFingerprint, EditorId, SessionStopReason, UserAction};

use crate::arena::HotArena;

pub struct CoreState {
    #[allow(dead_code)] // retained for cold-path / diagnostics; read when feature = "data"
    pub data_dir: PathBuf,
    pub services: CoreServices,
    pub arena: HotArena,
    #[cfg(feature = "data")]
    pub cold: ColdPathRuntime,
}

impl CoreState {
    pub fn new(data_dir: PathBuf) -> Self {
        // 持久化用户选词习惯：`{data_dir}/user_words.tsv`
        // 选「陶」学 tao→陶 后，下次输入 tao 由 merge_user_boosts 置顶
        let user_words =
            yc_lexicon::UserWordStore::open_or_create(data_dir.join("user_words.tsv"));
        #[cfg(feature = "data")]
        let cold = {
            let cold = ColdPathRuntime::new(data_dir.clone());
            crate::handlers::install_optional_handlers(&cold);
            cold
        };
        Self {
            data_dir: data_dir.clone(),
            services: CoreServices::with_user_words(user_words),
            arena: HotArena::new(),
            #[cfg(feature = "data")]
            cold,
        }
    }

    pub fn begin_session(&mut self, field_id: u64, input_type: u32) -> EditorId {
        let fp = EditorFingerprint {
            package_name: String::new(),
            field_id,
            input_type,
            ime_options: 0,
            hint_hash: 0,
        };
        let id = self.services.sessions.create(fp);
        self.services.sessions.activate(id);
        self.services.scheduler.on_session_created(id);
        self.services.handwriting.begin(id);
        let _ = self.services.scheduler.handle(
            &mut self.services.sessions,
            &mut self.services.handwriting,
            id,
            UserAction::Init,
        );
        id
    }

    pub fn stop_session(&mut self, editor_id: EditorId, reason: SessionStopReason) {
        self.services
            .scheduler
            .on_session_stopped(editor_id, &mut self.services.handwriting);
        self.services.sessions.stop(editor_id, reason);
    }

    pub fn submit_action(&mut self, editor_id: EditorId, action: UserAction) -> i32 {
        use yc_types::{YC_ERR_BUSY, YC_ERR_SESSION, YC_OK};

        match self.services.scheduler.handle(
            &mut self.services.sessions,
            &mut self.services.handwriting,
            editor_id,
            action,
        ) {
            Ok(outcome) => {
                self.arena
                    .write_snapshot(&outcome.snapshot, &outcome.commands);
                YC_OK
            }
            Err(yc_types::EngineError::SessionInvalid) => YC_ERR_SESSION,
            Err(_) => YC_ERR_BUSY,
        }
    }

    pub fn push_hw_stroke(
        &mut self,
        editor_id: EditorId,
        stroke: yc_types::Stroke,
        canvas_width: u32,
        canvas_height: u32,
        writing_mode: yc_types::WritingMode,
        session_stroke_id: u64,
    ) -> i32 {
        use yc_types::{YC_ERR_BUSY, YC_ERR_SESSION, YC_OK, UserAction};

        let batch = yc_types::StrokeBatch {
            editor_id,
            session_stroke_id,
            strokes: vec![stroke],
            canvas_width,
            canvas_height,
            writing_mode,
        };
        match self.submit_action(editor_id, UserAction::PushStrokeBatch { batch }) {
            YC_OK => YC_OK,
            YC_ERR_SESSION => YC_ERR_SESSION,
            _ => YC_ERR_BUSY,
        }
    }

    /// Apply shell-side Handwritten recognition into the hot arena.
    pub fn apply_hw_result(
        &mut self,
        editor_id: EditorId,
        texts: &[String],
        scores: &[f32],
        recognized_text: Option<String>,
        needs_cloud_confirm: bool,
    ) -> i32 {
        use yc_types::{YC_ERR_BUSY, YC_ERR_SESSION, YC_OK};

        match self.services.scheduler.apply_handwriting_result(
            &mut self.services.sessions,
            &mut self.services.handwriting,
            editor_id,
            texts,
            scores,
            recognized_text,
            needs_cloud_confirm,
        ) {
            Ok(outcome) => {
                self.arena
                    .write_snapshot(&outcome.snapshot, &outcome.commands);
                YC_OK
            }
            Err(yc_types::EngineError::SessionInvalid) => YC_ERR_SESSION,
            Err(_) => YC_ERR_BUSY,
        }
    }

    #[cfg(feature = "data")]
    pub fn sync_lang_packs(&mut self) -> i32 {
        use yc_session::EnabledLangPack;
        use yc_types::{LangPackEngineSpec, YC_OK};

        {
            let mut host = self.cold.plugin();
            host.reload_from_disk();
        }

        let host = self.cold.plugin();
        let enabled_slots: Vec<_> = host.list_enabled_slots().into_iter().cloned().collect();
        drop(host);

        let mut enabled = Vec::new();
        for slot in &enabled_slots {
            let spec = LangPackEngineSpec {
                pack_id: slot.pack_id.clone(),
                lexicon_path: slot.lexicon_path().to_string_lossy().into_owned(),
                install_path: slot.install_path.to_string_lossy().into_owned(),
                engine_kind: slot.engine_kind.clone(),
                default_scheme_id: slot.default_scheme_id.clone(),
            };
            let _ = self.services.scheduler.factory_mut().register(&spec);
            enabled.push(EnabledLangPack {
                pack_id: slot.pack_id.clone(),
                lang_tag: slot.lang_tag.clone(),
                default_scheme_id: slot.default_scheme_id.clone(),
                default_layout_id: slot.default_layout_id.clone(),
            });
        }
        self.services.scheduler.set_enabled_packs(enabled);
        YC_OK
    }

    /// Sync install + enable an `.imepack`, then register into the hot scheduler.
    #[cfg(feature = "data")]
    pub fn install_and_enable_langpack(&mut self, pack_path: &str) -> i32 {
        use yc_types::{YC_ERR_INTERNAL, YC_OK};

        {
            let mut host = self.cold.plugin();
            let manifest = match host.install_lang_pack(pack_path) {
                Ok(m) => m,
                Err(_) => return YC_ERR_INTERNAL,
            };
            if host.enable(&manifest.id).is_err() {
                return YC_ERR_INTERNAL;
            }
        }
        let _ = self.sync_lang_packs();
        YC_OK
    }

    /// Hook for cold LangPackDisable → scheduler; wire when cold completion notifies core.
    #[cfg(feature = "data")]
    #[allow(dead_code)]
    pub fn on_lang_pack_disabled(&mut self, pack_id: &str) {
        self.services.scheduler.on_pack_disabled(pack_id);
    }

    /// Hook for cold Skin → arena ApplyTheme; wire when cold completion notifies core.
    #[cfg(feature = "data")]
    #[allow(dead_code)]
    pub fn apply_theme_from_cold(&mut self, editor_id: EditorId, skin_id: &str) {
        let snapshot = yc_types::ImmSnapshot {
            editor_id,
            seq: self.services.sessions.bump_seq(editor_id),
            input_mode: self
                .services
                .sessions
                .input_mode(editor_id)
                .unwrap_or_default(),
            composing: yc_types::ComposingText::empty(),
            candidates: Vec::new(),
            status_flags: 0,
            cand_page: 0,
            cand_total: 0,
        };
        let commands = vec![yc_types::UiCommand::ApplyTheme {
            skin_id: skin_id.to_string(),
        }];
        self.arena.write_snapshot(&snapshot, &commands);
    }
}
