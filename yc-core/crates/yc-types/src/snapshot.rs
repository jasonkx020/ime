use zeroize::Zeroize;

use crate::mode::InputMode;
use crate::session::EditorId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    Lexicon,
    User,
    Hot,
    Ai,
    Handwriting,
    Emoji,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: u32,
    pub text: String,
    pub source: CandidateSource,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Zeroize)]
#[zeroize(drop)]
pub struct ComposingText {
    pub text: String,
    pub cursor: u32,
}

impl ComposingText {
    pub fn empty() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    Commit { text: String },
    SetComposing { text: String },
    FinishComposing,
    DeleteSurrounding { before: u32, after: u32 },
    ReloadKeyboard { layout: crate::mode::KeyboardLayout },
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineStep {
    pub composing: ComposingText,
    pub candidates: Vec<Candidate>,
    pub commands: Vec<UiCommand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImmSnapshot {
    pub editor_id: EditorId,
    pub seq: u64,
    pub input_mode: InputMode,
    pub composing: ComposingText,
    pub candidates: Vec<Candidate>,
    pub status_flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HotOutcome {
    pub snapshot: ImmSnapshot,
    pub commands: Vec<UiCommand>,
}
