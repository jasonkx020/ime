use crate::PinyinEngine;
use yc_types::{InputScheme, Language};

#[derive(Debug)]
pub struct EngineFactory {
    engine: PinyinEngine,
}

impl EngineFactory {
    pub fn new() -> Self {
        Self {
            engine: PinyinEngine::new(),
        }
    }

    pub fn engine_mut(&mut self) -> &mut PinyinEngine {
        &mut self.engine
    }

    pub fn default_lang(&self) -> Language {
        Language::Zh
    }

    pub fn default_scheme(&self) -> InputScheme {
        InputScheme::PinyinFull
    }
}

impl Default for EngineFactory {
    fn default() -> Self {
        Self::new()
    }
}
