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

/// Pack-level layout & geometry (symbol / shift / key-area height).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackLayouts {
    #[serde(default)]
    pub symbol_layout_id: Option<String>,
    #[serde(default)]
    pub shift_layout_id: Option<String>,
    /// Key-area height in dp (IME overlay, not toolbar).
    #[serde(default)]
    pub keyboard_height_dp: Option<u32>,
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
    #[serde(default)]
    pub layouts: PackLayouts,
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
    #[serde(default)]
    pub layouts: PackLayouts,
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
            layouts: self.layouts.clone(),
        }
    }
}

pub fn manifest_to_bytes(m: &LangPackManifest) -> Vec<u8> {
    serde_json::to_vec(m).expect("manifest json")
}

pub fn manifest_from_bytes(bytes: &[u8]) -> Result<LangPackManifest, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_toml_layouts_roundtrip_to_manifest() {
        let toml = r#"
id = "zh-pack-v1"
version = 3
min_host_version = "0.1.0"
lang = "zh"
display_name = "中文拼音"

[[schemes]]
id = "pinyin_full"
name = "全拼"
default_layout_id = "layout_pinyin26"
file = "schemes/pinyin_full.yaml"

[lexicon]
file = "lexicon/zh_words.tsv"
format = "dat"

[layouts]
symbol_layout_id = "layout_symbol"
shift_layout_id = "layout_pinyin26_shift"
keyboard_height_dp = 267
"#;
        let pack: PackToml = toml::from_str(toml).expect("parse");
        let m = pack.to_manifest();
        assert_eq!(m.layouts.symbol_layout_id.as_deref(), Some("layout_symbol"));
        assert_eq!(
            m.layouts.shift_layout_id.as_deref(),
            Some("layout_pinyin26_shift")
        );
        assert_eq!(m.layouts.keyboard_height_dp, Some(267));
        let bytes = manifest_to_bytes(&m);
        let back = manifest_from_bytes(&bytes).expect("json");
        assert_eq!(back.layouts, m.layouts);
    }
}
