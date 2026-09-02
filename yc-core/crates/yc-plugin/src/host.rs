use std::collections::HashMap;
use std::path::{Path, PathBuf};

use yc_pack::{LangPackManifest, install_pack_to_dir};
use yc_types::{EngineError, HotResult, LangPackInfo, LangPackState};

use crate::registry::LangPackRegistry;
use crate::slot::LangPackSlot;

#[derive(Debug)]
struct InstalledPack {
    manifest: LangPackManifest,
    path: PathBuf,
    state: LangPackState,
}

#[derive(Debug)]
pub struct PluginHost {
    data_dir: PathBuf,
    installed: HashMap<String, InstalledPack>,
    registry: LangPackRegistry,
}

impl PluginHost {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            installed: HashMap::new(),
            registry: LangPackRegistry::new(),
        }
    }

    pub fn registry(&self) -> &LangPackRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut LangPackRegistry {
        &mut self.registry
    }

    pub fn register_installed(&mut self, manifest: &LangPackManifest) -> HotResult<()> {
        let path = self.data_dir.join("langpacks").join(&manifest.id);
        self.installed.insert(
            manifest.id.clone(),
            InstalledPack {
                manifest: manifest.clone(),
                path,
                state: LangPackState::Installed,
            },
        );
        Ok(())
    }

    pub fn install_lang_pack(&mut self, pack_path: &str) -> HotResult<LangPackManifest> {
        let dest = self.data_dir.join("langpacks");
        let manifest = install_pack_to_dir(Path::new(pack_path), &dest)
            .map_err(|_| EngineError::Internal)?;
        self.register_installed(&manifest)?;
        Ok(manifest)
    }

    pub fn enable_lang_pack(&mut self, pack_id: &str) -> HotResult<LangPackSlot> {
        self.enable(pack_id)
    }

    pub fn disable_lang_pack(&mut self, pack_id: &str) -> HotResult<()> {
        self.disable(pack_id)
    }

    pub fn enable(&mut self, pack_id: &str) -> HotResult<LangPackSlot> {
        let (manifest, path) = {
            let pack = self
                .installed
                .get_mut(pack_id)
                .ok_or(EngineError::Unsupported)?;
            pack.state = LangPackState::Enabled;
            (pack.manifest.clone(), pack.path.clone())
        };
        let slot = self.registry.enable(&manifest, path);
        Ok(slot)
    }

    pub fn disable(&mut self, pack_id: &str) -> HotResult<()> {
        let pack = self
            .installed
            .get_mut(pack_id)
            .ok_or(EngineError::Unsupported)?;
        pack.state = LangPackState::Disabled;
        self.registry.disable(pack_id);
        Ok(())
    }

    pub fn uninstall(&mut self, pack_id: &str) -> HotResult<()> {
        self.registry.disable(pack_id);
        if let Some(pack) = self.installed.remove(pack_id) {
            let _ = std::fs::remove_dir_all(pack.path);
        }
        Ok(())
    }

    pub fn list_installed(&self) -> Vec<LangPackInfo> {
        self.installed
            .values()
            .map(|p| LangPackInfo {
                id: p.manifest.id.clone(),
                lang: p.manifest.lang.clone(),
                version: p.manifest.version,
                display_name: p.manifest.display_name.clone(),
                state: p.state.clone(),
            })
            .collect()
    }

    pub fn list_enabled(&self) -> Vec<LangPackInfo> {
        self.list_installed()
            .into_iter()
            .filter(|p| p.state == LangPackState::Enabled)
            .collect()
    }

    pub fn list_enabled_slots(&self) -> Vec<&LangPackSlot> {
        self.registry.list_enabled()
    }

    pub fn list_catalog_local(&self) -> Vec<LangPackInfo> {
        self.list_installed()
    }

    pub fn manifest(&self, pack_id: &str) -> Option<&LangPackManifest> {
        self.installed.get(pack_id).map(|p| &p.manifest)
    }

    pub fn install_path(&self, pack_id: &str) -> Option<PathBuf> {
        self.installed.get(pack_id).map(|p| p.path.clone())
    }

    pub fn slot(&self, pack_id: &str) -> Option<&LangPackSlot> {
        self.registry.get(pack_id)
    }

    pub fn lexicon_path(&self, pack_id: &str) -> Option<PathBuf> {
        if let Some(slot) = self.registry.get(pack_id) {
            return Some(slot.lexicon_path());
        }
        self.installed.get(pack_id).map(|p| {
            p.path.join(p.manifest.lexicon.effective_dat_path())
        })
    }

    pub fn is_enabled(&self, pack_id: &str) -> bool {
        self.installed
            .get(pack_id)
            .map(|p| p.state == LangPackState::Enabled)
            .unwrap_or(false)
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}
