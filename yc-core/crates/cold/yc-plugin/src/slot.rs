use std::path::{Path, PathBuf};

use yc_pack::LangPackManifest;

/// Runtime view of an installed/enabled language pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangPackSlot {
    pub pack_id: String,
    pub lang_tag: String,
    pub display_name: String,
    pub default_scheme_id: String,
    pub default_layout_id: String,
    pub install_path: PathBuf,
    pub lexicon_dat_rel: String,
    pub strings_rel: Option<String>,
    pub layout_ids: Vec<String>,
    pub engine_kind: String,
}

impl LangPackSlot {
    pub fn from_manifest(manifest: &LangPackManifest, install_path: PathBuf) -> Self {
        let default_scheme = manifest.schemes.first();
        Self {
            pack_id: manifest.id.clone(),
            lang_tag: manifest.lang.clone(),
            display_name: manifest.display_name.clone(),
            default_scheme_id: default_scheme
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "latin".into()),
            default_layout_id: default_scheme
                .map(|s| s.default_layout_id.clone())
                .unwrap_or_else(|| "layout_qwerty".into()),
            install_path,
            lexicon_dat_rel: manifest.lexicon.effective_dat_path().to_string(),
            strings_rel: manifest.strings_path.clone(),
            layout_ids: manifest.layout_ids.clone(),
            engine_kind: manifest.engine.clone(),
        }
    }

    pub fn lexicon_path(&self) -> PathBuf {
        self.install_path.join(&self.lexicon_dat_rel)
    }

    pub fn layout_path(&self, layout_id: &str) -> PathBuf {
        self.install_path
            .join(format!("layouts/{layout_id}.bin"))
    }

    pub fn scheme_for_id<'a>(&self, manifest: &'a LangPackManifest, scheme_id: &str) -> Option<&'a yc_pack::PackScheme> {
        manifest.schemes.iter().find(|s| s.id == scheme_id)
    }
}

pub fn scan_layout_ids(install_path: &Path) -> Vec<String> {
    let layouts_dir = install_path.join("layouts");
    let Ok(entries) = std::fs::read_dir(&layouts_dir) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            e.path()
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".bin"))
                .map(String::from)
        })
        .collect();
    ids.sort();
    ids
}
