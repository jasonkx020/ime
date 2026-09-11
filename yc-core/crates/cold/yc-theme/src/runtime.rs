use std::path::Path;
use std::sync::{Arc, RwLock};

use yc_pack::extract_skin_from_pack;
use yc_types::{EngineError, HotResult, TaskId};

use crate::tokens::ThemeTokens;

#[derive(Debug)]
pub struct ThemeRuntime {
    current: Arc<RwLock<ThemeTokens>>,
}

impl ThemeRuntime {
    pub fn new() -> Self {
        Self {
            current: Arc::new(RwLock::new(ThemeTokens::default())),
        }
    }

    pub fn current(&self) -> ThemeTokens {
        self.current.read().unwrap().clone()
    }

    pub fn fallback_default(&self) -> ThemeTokens {
        ThemeTokens::samsung_light()
    }

    pub fn load_pack(&self, path: &Path) -> HotResult<ThemeTokens> {
        let manifest = extract_skin_from_pack(path).map_err(|_| EngineError::Internal)?;
        let tokens = ThemeTokens::from_skin_manifest(&manifest);
        *self.current.write().unwrap() = tokens.clone();
        Ok(tokens)
    }

    pub fn apply_tokens(&self, tokens: ThemeTokens) {
        *self.current.write().unwrap() = tokens;
    }
}

impl Default for ThemeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub type ThemeTaskId = TaskId;
