use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{SharedCharNgram, UserWordStore};
use yc_scheme::SchemeDesc;
use yc_scheme::TransformKind;
use yc_types::{
    Candidate, EditorId, EngineError, EngineStep, HotResult, InputMode, LangPackEngineSpec,
};

use crate::data_driven::DataDrivenEngine;
use crate::latin::LatinPredictEngine;
use crate::InputEngine;

#[derive(Debug)]
enum EngineSlotInner {
    Latin(LatinPredictEngine),
    DataDriven(DataDrivenEngine),
}

#[derive(Debug)]
struct RegisteredPack {
    /// Pack manifest default scheme (metadata; active scheme tracked separately).
    #[allow(dead_code)]
    default_scheme_id: String,
    active_scheme_id: String,
    engines: HashMap<String, EngineSlotInner>,
}

#[derive(Debug)]
pub struct EngineFactory {
    slots: HashMap<String, RegisteredPack>,
    active_pack: Option<String>,
    active_editor: EditorId,
    user_words: Arc<Mutex<UserWordStore>>,
    shared_ngram: SharedCharNgram,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            active_pack: None,
            active_editor: EditorId::NONE,
            user_words: UserWordStore::shared(),
            shared_ngram: SharedCharNgram::new(),
        }
    }

    pub fn with_user_words(store: Arc<Mutex<UserWordStore>>) -> Self {
        Self {
            slots: HashMap::new(),
            active_pack: None,
            active_editor: EditorId::NONE,
            user_words: store,
            shared_ngram: SharedCharNgram::new(),
        }
    }

    pub fn user_words(&self) -> Arc<Mutex<UserWordStore>> {
        self.user_words.clone()
    }

    pub fn shared_ngram(&self) -> SharedCharNgram {
        self.shared_ngram.clone()
    }

    pub fn set_user_words_path(&self, path: impl AsRef<Path>) {
        let opened = UserWordStore::open_or_create(path.as_ref());
        let mut dst = self.user_words.lock();
        *dst = opened.lock().clone();
        dst.set_path(path.as_ref().to_path_buf());
    }

    fn attach_user_words(&self, slot: &mut EngineSlotInner) {
        match slot {
            EngineSlotInner::Latin(l) => {
                l.set_user_words(self.user_words.clone());
                l.set_shared_ngram(self.shared_ngram.clone());
            }
            EngineSlotInner::DataDriven(d) => {
                d.set_user_words(self.user_words.clone());
                d.set_shared_ngram(self.shared_ngram.clone());
            }
        }
    }

    pub fn register(&mut self, spec: &LangPackEngineSpec) -> HotResult<()> {
        let scheme_dir = Path::new(&spec.install_path).join("scheme");
        let mut engines = HashMap::new();

        if scheme_dir.is_dir() {
            for entry in fs::read_dir(&scheme_dir).map_err(|_| EngineError::Internal)? {
                let entry = entry.map_err(|_| EngineError::Internal)?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("bin") {
                    continue;
                }
                let scheme_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("latin")
                    .to_string();
                let bytes = fs::read(&path).map_err(|_| EngineError::Internal)?;
                let desc =
                    SchemeDesc::from_bytes(&bytes).map_err(|_| EngineError::PackInvalid)?;
                let mut slot = match desc.transform {
                    TransformKind::LatinPredict => {
                        let mut latin = LatinPredictEngine::new(spec.pack_id.clone());
                        latin.load_lexicon(&spec.pack_id, &spec.lexicon_path)?;
                        EngineSlotInner::Latin(latin)
                    }
                    TransformKind::RuleChain | TransformKind::Table => {
                        let mut engine = DataDrivenEngine::new(spec.pack_id.clone(), desc);
                        engine.load_lexicon(&spec.pack_id, &spec.lexicon_path)?;
                        EngineSlotInner::DataDriven(engine)
                    }
                };
                self.attach_user_words(&mut slot);
                engines.insert(scheme_id, slot);
            }
        }

        if engines.is_empty() {
            let mut latin = LatinPredictEngine::new(spec.pack_id.clone());
            latin.load_lexicon(&spec.pack_id, &spec.lexicon_path)?;
            let mut slot = EngineSlotInner::Latin(latin);
            self.attach_user_words(&mut slot);
            engines.insert(spec.default_scheme_id.clone(), slot);
        }

        self.slots.insert(
            spec.pack_id.clone(),
            RegisteredPack {
                default_scheme_id: spec.default_scheme_id.clone(),
                active_scheme_id: spec.default_scheme_id.clone(),
                engines,
            },
        );
        Ok(())
    }

    pub fn register_latin_pack(
        &mut self,
        pack_id: &str,
        lexicon_path: &str,
    ) -> Result<(), yc_types::EngineError> {
        let install = Path::new(lexicon_path)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| ".".into());
        self.register(&LangPackEngineSpec {
            pack_id: pack_id.to_string(),
            lexicon_path: lexicon_path.to_string(),
            install_path: install,
            engine_kind: "data_driven".into(),
            default_scheme_id: "latin".into(),
        })
    }

    pub fn unregister(&mut self, pack_id: &str) {
        self.slots.remove(pack_id);
        if self.active_pack.as_deref() == Some(pack_id) {
            self.active_pack = None;
        }
    }

    pub fn set_active_pack(&mut self, pack_id: Option<String>) {
        self.active_pack = pack_id;
    }

    pub fn active_pack_id(&self) -> Option<&str> {
        self.active_pack.as_deref()
    }

    pub fn create(&mut self, pack_id: &str, scheme_id: &str) -> HotResult<()> {
        let pack = self
            .slots
            .get_mut(pack_id)
            .ok_or(EngineError::Unsupported)?;
        if !pack.engines.contains_key(scheme_id) {
            return Err(EngineError::Unsupported);
        }
        pack.active_scheme_id = scheme_id.to_string();
        self.active_pack = Some(pack_id.to_string());
        Ok(())
    }

    fn with_active<F, R>(&mut self, f: F) -> HotResult<R>
    where
        F: FnOnce(&mut EngineSlotInner) -> HotResult<R>,
    {
        let pack_id = self.active_pack.clone().ok_or(EngineError::Unsupported)?;
        let pack = self
            .slots
            .get_mut(&pack_id)
            .ok_or(EngineError::Unsupported)?;
        let engine = pack
            .engines
            .get_mut(&pack.active_scheme_id)
            .ok_or(EngineError::Unsupported)?;
        f(engine)
    }

    pub fn has_active_pack(&self) -> bool {
        self.active_pack.is_some()
    }

    pub fn reset_active(&mut self, editor_id: EditorId) {
        let _ = self.with_active(|e| {
            match e {
                EngineSlotInner::Latin(l) => l.reset(editor_id),
                EngineSlotInner::DataDriven(d) => d.reset(editor_id),
            }
            Ok(())
        });
    }

    pub fn set_active_editor(&mut self, editor_id: EditorId) {
        if self.active_editor != editor_id {
            self.active_editor = editor_id;
            if self.has_active_pack() {
                self.reset_active(editor_id);
            }
        }
    }

    pub fn remove_active_session(&mut self, _editor_id: EditorId) {}

    pub fn feed_active(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
        input_mode: &InputMode,
    ) -> HotResult<EngineStep> {
        self.with_active(|e| match e {
            EngineSlotInner::Latin(l) => l.feed(editor_id, key_code, input_mode),
            EngineSlotInner::DataDriven(d) => d.feed(editor_id, key_code, input_mode),
        })
    }

    pub fn backspace_active(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        self.with_active(|e| match e {
            EngineSlotInner::Latin(l) => l.backspace(editor_id),
            EngineSlotInner::DataDriven(d) => d.backspace(editor_id),
        })
    }

    pub fn select_active(
        &mut self,
        editor_id: EditorId,
        candidate_id: u32,
    ) -> HotResult<EngineStep> {
        self.with_active(|e| match e {
            EngineSlotInner::Latin(l) => l.select(editor_id, candidate_id),
            EngineSlotInner::DataDriven(d) => d.select(editor_id, candidate_id),
        })
    }

    pub fn update_active_candidates(&mut self, cands: Vec<Candidate>) {
        let _ = self.with_active(|e| {
            match e {
                EngineSlotInner::Latin(l) => l.replace_cand_pool_keep_page(cands),
                EngineSlotInner::DataDriven(d) => d.replace_cand_pool_keep_page(cands),
            }
            Ok(())
        });
    }

    /// Apply background pinyin lookup if ready. Returns true when cand pool changed.
    pub fn poll_async_lookup(&mut self) -> bool {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.poll_async_lookup(),
                EngineSlotInner::Latin(_) => false,
            })
        })
        .unwrap_or(false)
    }

    pub fn needs_llm_fallback(&mut self) -> bool {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.needs_llm_fallback(),
                EngineSlotInner::Latin(_) => false,
            })
        })
        .unwrap_or(false)
    }

    /// Append AI candidates for `query`. Returns false if composing mismatch.
    pub fn inject_ai_candidates(&mut self, query: &str, texts: &[String]) -> bool {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.inject_ai_candidates(query, texts),
                EngineSlotInner::Latin(_) => false,
            })
        })
        .unwrap_or(false)
    }

    /// Wait for in-flight lookup (space/select). Returns true if pool updated.
    pub fn flush_async_lookup(&mut self, timeout_ms: u64) -> bool {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.flush_async_lookup(timeout_ms),
                EngineSlotInner::Latin(_) => false,
            })
        })
        .unwrap_or(false)
    }

    /// Composing text of active engine (for rerank after poll).
    pub fn active_composing_text(&mut self) -> String {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.composing_text().to_string(),
                EngineSlotInner::Latin(_) => String::new(),
            })
        })
        .unwrap_or_default()
    }

    pub fn active_cand_pool_clone(&mut self) -> Vec<Candidate> {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::DataDriven(d) => d.cand_pool_clone(),
                EngineSlotInner::Latin(_) => Vec::new(),
            })
        })
        .unwrap_or_default()
    }

    pub fn page_next_active(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        self.with_active(|e| match e {
            EngineSlotInner::Latin(l) => l.page_next(editor_id),
            EngineSlotInner::DataDriven(d) => d.page_next(editor_id),
        })
    }

    pub fn page_prev_active(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        self.with_active(|e| match e {
            EngineSlotInner::Latin(l) => l.page_prev(editor_id),
            EngineSlotInner::DataDriven(d) => d.page_prev(editor_id),
        })
    }

    pub fn active_cand_meta(&mut self) -> (u32, u32) {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::Latin(l) => (l.cand_page(), l.cand_total()),
                EngineSlotInner::DataDriven(d) => (d.cand_page(), d.cand_total()),
            })
        })
        .unwrap_or((0, 0))
    }

    pub fn active_paged_candidates(&mut self) -> Vec<Candidate> {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::Latin(l) => l.current_paged_step().candidates,
                EngineSlotInner::DataDriven(d) => d.current_paged_step().candidates,
            })
        })
        .unwrap_or_default()
    }

    pub fn active_query_key(&mut self) -> String {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::Latin(l) => l.last_query_key().to_string(),
                EngineSlotInner::DataDriven(d) => d.last_query_key().to_string(),
            })
        })
        .unwrap_or_default()
    }

    pub fn touch_user_word(&mut self, pinyin: &str, word: &str) {
        let lang = self
            .active_pack
            .as_deref()
            .map(|id| {
                let id = id.to_ascii_lowercase();
                if id.starts_with("zh") || id.contains("zh-") {
                    "zh".to_string()
                } else if id.starts_with("en") || id.contains("en-") {
                    "en".to_string()
                } else if id.starts_with("vi") || id.contains("vi-") {
                    "vi".to_string()
                } else if id.starts_with("th") || id.contains("th-") {
                    "th".to_string()
                } else {
                    id.split('-').next().unwrap_or("").to_string()
                }
            })
            .unwrap_or_default();
        self.user_words.lock().touch_lang(&lang, pinyin, word);
        UserWordStore::schedule_flush_shared(self.user_words.clone());
    }

    pub fn touch_user_word_lang(&mut self, lang: &str, query_key: &str, word: &str) {
        self.user_words.lock().touch_lang(lang, query_key, word);
        UserWordStore::schedule_flush_shared(self.user_words.clone());
    }

    /// Lexicon association suffixes for `prefix`.
    /// Prefer active engine, then zh DataDriven packs, then any other loaded lexicon.
    pub fn associate(&mut self, prefix: &str, limit: usize) -> Vec<Candidate> {
        let primary = self
            .with_active(|e| {
                Ok(match e {
                    EngineSlotInner::Latin(l) => l.associate(prefix, limit),
                    EngineSlotInner::DataDriven(d) => d.associate(prefix, limit),
                })
            })
            .unwrap_or_default();
        if !primary.is_empty() {
            return primary;
        }

        let mut zh_ids: Vec<String> = self
            .slots
            .keys()
            .filter(|id| id.contains("zh"))
            .cloned()
            .collect();
        zh_ids.sort();
        for id in zh_ids {
            if let Some(cands) = self.associate_in_pack(&id, prefix, limit, true) {
                return cands;
            }
        }
        let mut other_ids: Vec<String> = self
            .slots
            .keys()
            .filter(|id| !id.contains("zh"))
            .cloned()
            .collect();
        other_ids.sort();
        for id in other_ids {
            if let Some(cands) = self.associate_in_pack(&id, prefix, limit, false) {
                return cands;
            }
        }
        Vec::new()
    }

    fn associate_in_pack(
        &mut self,
        pack_id: &str,
        prefix: &str,
        limit: usize,
        data_driven_only: bool,
    ) -> Option<Vec<Candidate>> {
        let pack = self.slots.get_mut(pack_id)?;
        for engine in pack.engines.values_mut() {
            let cands = match engine {
                EngineSlotInner::DataDriven(d) => d.associate(prefix, limit),
                EngineSlotInner::Latin(l) if !data_driven_only => l.associate(prefix, limit),
                EngineSlotInner::Latin(_) => Vec::new(),
            };
            if !cands.is_empty() {
                return Some(cands);
            }
        }
        None
    }

    /// Accumulated association context after select (empty when composing).
    pub fn assoc_context(&mut self) -> String {
        self.with_active(|e| {
            Ok(match e {
                EngineSlotInner::Latin(_) => String::new(),
                EngineSlotInner::DataDriven(d) => d.assoc_context().to_string(),
            })
        })
        .unwrap_or_default()
    }

    pub fn clear_assoc_context(&mut self) {
        let _ = self.with_active(|e| {
            match e {
                EngineSlotInner::Latin(_) => {}
                EngineSlotInner::DataDriven(d) => d.clear_assoc(),
            }
            Ok(())
        });
    }

    pub fn set_assoc_active(&mut self, active: bool) {
        let _ = self.with_active(|e| {
            match e {
                EngineSlotInner::Latin(_) => {}
                EngineSlotInner::DataDriven(d) => d.set_assoc_active(active),
            }
            Ok(())
        });
    }
}

impl Default for EngineFactory {
    fn default() -> Self {
        Self::new()
    }
}
