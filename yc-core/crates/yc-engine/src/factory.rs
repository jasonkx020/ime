use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::data_driven::DataDrivenEngine;
use crate::latin::LatinPredictEngine;
use crate::InputEngine;
use yc_scheme::TransformKind;
use yc_scheme::SchemeDesc;
use yc_types::{EditorId, EngineError, EngineStep, HotResult, InputMode, LangPackEngineSpec};

#[derive(Debug)]
enum EngineSlotInner {
    Latin(LatinPredictEngine),
    DataDriven(DataDrivenEngine),
}

#[derive(Debug)]
struct RegisteredPack {
    default_scheme_id: String,
    active_scheme_id: String,
    engines: HashMap<String, EngineSlotInner>,
}

#[derive(Debug)]
pub struct EngineFactory {
    slots: HashMap<String, RegisteredPack>,
    active_pack: Option<String>,
    active_editor: EditorId,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            active_pack: None,
            active_editor: EditorId::NONE,
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
                let slot = match desc.transform {
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
                engines.insert(scheme_id, slot);
            }
        }

        if engines.is_empty() {
            let mut latin = LatinPredictEngine::new(spec.pack_id.clone());
            latin.load_lexicon(&spec.pack_id, &spec.lexicon_path)?;
            engines.insert(
                spec.default_scheme_id.clone(),
                EngineSlotInner::Latin(latin),
            );
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
}

impl Default for EngineFactory {
    fn default() -> Self {
        Self::new()
    }
}
