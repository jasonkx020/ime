use serde::{Deserialize, Serialize};

/// UI theme tokens aligned with KEYBOARD_UI_DESIGN §11.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub skin_id: String,
    pub keyboard_bg: u32,
    pub key_normal: u32,
    pub key_utility: u32,
    pub key_accent: u32,
    pub key_pressed: u32,
    pub cand_text: u32,
    pub composing_text: u32,
    pub toolbar_text: u32,
    pub key_radius: f32,
    pub key_font_size: f32,
    pub cand_font_size: f32,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self::samsung_light()
    }
}

impl ThemeTokens {
    pub fn samsung_light() -> Self {
        Self {
            skin_id: "samsung-light".into(),
            keyboard_bg: 0xFFE8EAED,
            key_normal: 0xFFFFFFFF,
            key_utility: 0xFFDDE0E4,
            key_accent: 0xFF1A73E8,
            key_pressed: 0xFFC8CCD2,
            cand_text: 0xFF202124,
            composing_text: 0xFF1A73E8,
            toolbar_text: 0xFF5F6368,
            key_radius: 12.0,
            key_font_size: 16.0,
            cand_font_size: 15.0,
        }
    }

    pub fn from_skin_manifest(m: &yc_pack::SkinManifest) -> Self {
        Self {
            skin_id: m.id.clone(),
            keyboard_bg: parse_hex_color(&m.colors.keyboard_bg),
            key_normal: parse_hex_color(&m.colors.key_normal),
            key_utility: parse_hex_color(&m.colors.key_utility),
            key_accent: parse_hex_color(&m.colors.key_accent),
            key_pressed: parse_hex_color(&m.colors.key_pressed),
            cand_text: parse_hex_color(&m.colors.cand_text),
            composing_text: parse_hex_color(&m.colors.composing_text),
            toolbar_text: parse_hex_color(&m.colors.toolbar_text),
            key_radius: m.key_radius,
            key_font_size: m.key_font_size,
            cand_font_size: m.cand_font_size,
        }
    }

    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("theme json")
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

fn parse_hex_color(s: &str) -> u32 {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
        return 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
    }
    0xFF000000
}
