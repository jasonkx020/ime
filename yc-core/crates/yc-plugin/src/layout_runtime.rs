use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use yc_layout::LayoutView;
use yc_types::EngineError;

#[derive(Debug, Clone)]
pub struct LayoutRuntime {
    cache: HashMap<(String, String), LayoutView>,
}

impl LayoutRuntime {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn open(&mut self, pack_path: &Path, layout_id: &str) -> Result<LayoutView, EngineError> {
        let key = (pack_path.to_string_lossy().into_owned(), layout_id.to_string());
        if let Some(view) = self.cache.get(&key) {
            return Ok(view.clone());
        }
        let bin_path = pack_path.join(format!("layouts/{layout_id}.bin"));
        let bytes = fs::read(&bin_path).map_err(|_| EngineError::Internal)?;
        let view = LayoutView::from_bytes(&bytes).map_err(|_| EngineError::PackInvalid)?;
        self.cache.insert(key, view.clone());
        Ok(view)
    }

    pub fn preload_pack(&mut self, pack_path: &Path, layout_ids: &[String]) {
        for id in layout_ids {
            let _ = self.open(pack_path, id);
        }
    }

    pub fn clear_pack(&mut self, pack_path: &Path) {
        let prefix = pack_path.to_string_lossy().into_owned();
        self.cache.retain(|(p, _), _| p != &prefix);
    }
}

impl Default for LayoutRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub fn layout_bin_path(install_path: &Path, layout_id: &str) -> PathBuf {
    install_path.join(format!("layouts/{layout_id}.bin"))
}
