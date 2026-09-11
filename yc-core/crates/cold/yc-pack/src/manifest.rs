use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackScheme {
    pub id: String,
    pub name: String,
    pub default_layout_id: String,
    #[serde(default)]
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LexiconRef {
    pub file: String,
    pub format: String,
    #[serde(default)]
    pub dat_path: String,
}

impl LexiconRef {
    /// Compiled lexicon path inside `.imepack` (e.g. `lexicon/vi_words.dat`).
    pub fn dat_relpath_from_source(source: &str) -> String {
        let stem = std::path::Path::new(source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("lexicon");
        format!("lexicon/{stem}.dat")
    }

    pub fn effective_dat_path(&self) -> String {
        if self.dat_path.is_empty() {
            Self::dat_relpath_from_source(&self.file)
        } else {
            self.dat_path.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LangPackManifest {
    pub id: String,
    pub version: u32,
    pub min_host_version: String,
    pub lang: String,
    pub display_name: String,
    pub schemes: Vec<PackScheme>,
    pub lexicon: LexiconRef,
    #[serde(default)]
    pub strings_path: Option<String>,
    #[serde(default)]
    pub layout_ids: Vec<String>,
    #[serde(default = "default_engine")]
    pub engine: String,
}

fn default_engine() -> String {
    "data_driven".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackToml {
    pub id: String,
    pub version: u32,
    pub min_host_version: String,
    pub lang: String,
    pub display_name: String,
    pub schemes: Vec<PackScheme>,
    pub lexicon: LexiconRef,
    #[serde(default)]
    pub build: Option<PackBuildSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackBuildSection {
    pub strings: Option<String>,
}

impl PackToml {
    pub fn to_manifest(&self) -> LangPackManifest {
        let mut lexicon = self.lexicon.clone();
        lexicon.dat_path = LexiconRef::dat_relpath_from_source(&lexicon.file);
        let strings_path = self.build.as_ref().and_then(|b| b.strings.clone());
        LangPackManifest {
            id: self.id.clone(),
            version: self.version,
            min_host_version: self.min_host_version.clone(),
            lang: self.lang.clone(),
            display_name: self.display_name.clone(),
            schemes: self.schemes.clone(),
            lexicon,
            strings_path,
            layout_ids: Vec::new(),
            engine: "data_driven".into(),
        }
    }
}

pub fn manifest_to_bytes(m: &LangPackManifest) -> Vec<u8> {
    serde_json::to_vec(m).expect("manifest json")
}

pub fn manifest_from_bytes(bytes: &[u8]) -> Result<LangPackManifest, serde_json::Error> {
    serde_json::from_slice(bytes)
}
