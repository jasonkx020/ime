use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkinColors {
    pub keyboard_bg: String,
    pub key_normal: String,
    pub key_utility: String,
    pub key_accent: String,
    pub key_pressed: String,
    pub cand_text: String,
    pub composing_text: String,
    pub toolbar_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkinManifest {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub colors: SkinColors,
    pub key_radius: f32,
    pub key_font_size: f32,
    pub cand_font_size: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkinToml {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub colors: SkinColors,
    pub key_radius: f32,
    pub key_font_size: f32,
    pub cand_font_size: f32,
}

impl SkinToml {
    pub fn to_manifest(&self) -> SkinManifest {
        SkinManifest {
            id: self.id.clone(),
            version: self.version,
            name: self.name.clone(),
            colors: self.colors.clone(),
            key_radius: self.key_radius,
            key_font_size: self.key_font_size,
            cand_font_size: self.cand_font_size,
        }
    }
}

pub fn skin_to_bytes(m: &SkinManifest) -> Vec<u8> {
    serde_json::to_vec(m).expect("skin json")
}

pub fn skin_from_bytes(bytes: &[u8]) -> Result<SkinManifest, serde_json::Error> {
    serde_json::from_slice(bytes)
}
