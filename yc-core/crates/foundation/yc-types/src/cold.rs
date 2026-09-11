//! Cold-path request kinds (M3/M3.5/M4).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ColdKind {
    Skin = 0,
    LangPackInstall = 1,
    LangPackEnable = 2,
    LangPackDisable = 3,
    LangPackCatalog = 4,
    HandwritingCloud = 5,
    AiPolish = 6,
    AiAssist = 7,
}

impl ColdKind {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Skin),
            1 => Some(Self::LangPackInstall),
            2 => Some(Self::LangPackEnable),
            3 => Some(Self::LangPackDisable),
            4 => Some(Self::LangPackCatalog),
            5 => Some(Self::HandwritingCloud),
            6 => Some(Self::AiPolish),
            7 => Some(Self::AiAssist),
            _ => None,
        }
    }

    pub fn raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LangPackState {
    Installed,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LangPackInfo {
    pub id: String,
    pub lang: String,
    pub version: u32,
    pub display_name: String,
    pub state: LangPackState,
}
