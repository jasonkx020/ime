//! AI assist request/response types (P2 cold path).

use crate::session::EditorId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AiMode {
    SmartReply = 0,
    HighEqReply = 1,
    Compose = 2,
    Rewrite = 3,
    Polish = 4,
}

impl AiMode {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::SmartReply),
            1 => Some(Self::HighEqReply),
            2 => Some(Self::Compose),
            3 => Some(Self::Rewrite),
            4 => Some(Self::Polish),
            _ => None,
        }
    }

    pub fn raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskReq {
    pub editor_id: u64,
    pub mode: u32,
    pub scene_id: String,
    #[serde(default)]
    pub peer_message: String,
    #[serde(default)]
    pub background_note: String,
    #[serde(default)]
    pub selection_text: String,
    #[serde(default)]
    pub user_intent: String,
}

impl TaskReq {
    pub fn editor(&self) -> EditorId {
        EditorId::from_raw(self.editor_id)
    }

    pub fn ai_mode(&self) -> Option<AiMode> {
        AiMode::from_raw(self.mode)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AiVariant {
    pub text: String,
    #[serde(default)]
    pub tone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AiOutput {
    pub variants: Vec<AiVariant>,
    #[serde(default)]
    pub local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RedactedPreview {
    pub summary: String,
    pub fields: Vec<RedactedField>,
    pub will_use_cloud: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RedactedField {
    pub name: String,
    pub value: String,
}
