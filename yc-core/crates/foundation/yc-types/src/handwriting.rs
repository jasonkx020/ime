use crate::session::EditorId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingMode {
    SingleChar = 0,
    Continuous = 1,
}

impl WritingMode {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::SingleChar),
            1 => Some(Self::Continuous),
            _ => None,
        }
    }

    pub fn raw(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    pub t: u64,
    pub pressure: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub points: Vec<StrokePoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeBatch {
    pub editor_id: EditorId,
    pub session_stroke_id: u64,
    pub strokes: Vec<Stroke>,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub writing_mode: WritingMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HandwritingResult {
    pub candidates: Vec<crate::snapshot::Candidate>,
    pub recognized_text: Option<String>,
    pub confidence: f32,
    pub used_cloud: bool,
    pub needs_cloud_confirm: bool,
}
