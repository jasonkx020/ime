use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorId(pub u64);

impl EditorId {
    pub const NONE: EditorId = EditorId(0);

    pub fn raw(self) -> u64 {
        self.0
    }

    pub fn from_raw(id: u64) -> Self {
        EditorId(id)
    }
}

impl fmt::Display for EditorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EditorId({})", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFingerprint {
    pub package_name: String,
    pub field_id: u64,
    pub input_type: u32,
    pub ime_options: u32,
    pub hint_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLevel {
    Normal,
    Sensitive,
    ForbiddenCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStopReason {
    FinishInput = 0,
    SwitchField = 1,
    KeyboardHide = 2,
    EditorInfoDowngrade = 3,
    ProcessRecycle = 4,
}

impl SessionStopReason {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::SwitchField,
            2 => Self::KeyboardHide,
            3 => Self::EditorInfoDowngrade,
            4 => Self::ProcessRecycle,
            _ => Self::FinishInput,
        }
    }
}
