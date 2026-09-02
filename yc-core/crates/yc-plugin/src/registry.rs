use std::collections::HashMap;
use std::path::PathBuf;

use yc_pack::LangPackManifest;
use yc_types::{EngineError, HotResult};

use crate::slot::{scan_layout_ids, LangPackSlot};

/// Builds slots and tracks enabled packs for Scheduler / EngineFactory.
#[derive(Debug, Default)]
pub struct LangPackRegistry {
    enabled: HashMap<String, LangPackSlot>,
}

impl LangPackRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build_slot(manifest: &LangPackManifest, install_path: PathBuf) -> LangPackSlot {
        let mut slot = LangPackSlot::from_manifest(manifest, install_path.clone());
        if slot.layout_ids.is_empty() {
            slot.layout_ids = scan_layout_ids(&install_path);
        }
        slot
    }

    pub fn enable(&mut self, manifest: &LangPackManifest, install_path: PathBuf) -> LangPackSlot {
        let slot = Self::build_slot(manifest, install_path);
        self.enabled.insert(slot.pack_id.clone(), slot.clone());
        slot
    }

    pub fn disable(&mut self, pack_id: &str) -> Option<LangPackSlot> {
        self.enabled.remove(pack_id)
    }

    pub fn get(&self, pack_id: &str) -> Option<&LangPackSlot> {
        self.enabled.get(pack_id)
    }

    pub fn list_enabled_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.enabled.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn list_enabled(&self) -> Vec<&LangPackSlot> {
        self.enabled.values().collect()
    }

    pub fn find_by_hash(&self, pack_id_hash: u32) -> Option<&LangPackSlot> {
        self.enabled.values().find(|s| hash_pack_id(&s.pack_id) == pack_id_hash)
    }
}

pub fn hash_pack_id(id: &str) -> u32 {
    id.bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
}

pub struct LangPackLoader;

impl LangPackLoader {
    pub fn slot_from_installed(
        manifest: &LangPackManifest,
        install_path: PathBuf,
    ) -> LangPackSlot {
        LangPackRegistry::build_slot(manifest, install_path)
    }
}

pub fn validate_slot(slot: &LangPackSlot) -> HotResult<()> {
    if !slot.lexicon_path().exists() {
        return Err(EngineError::Internal);
    }
    Ok(())
}
