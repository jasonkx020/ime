//! Domain types and C ABI structs shared across the yc-core workspace.

mod action;
mod editor_info;
mod error;
mod ffi;
mod handwriting;
mod mode;
mod session;
mod snapshot;

pub use action::{HotActionType, UserAction};
pub use handwriting::{
    HandwritingResult, Stroke, StrokeBatch, StrokePoint, WritingMode,
};
pub use editor_info::{
    is_email_field, is_number_field, is_password_field, CLASS_NUMBER, VARIATION_EMAIL,
    VARIATION_PASSWORD,
};
pub use error::{EngineError, HotResult};
pub use ffi::{
    YC_ERR_BUSY, YC_ERR_INTERNAL, YC_ERR_SESSION, YC_OK, YcCandidateSlot, YcHotAction, YcHotHeader,
    YcStrokePoint, MAX_CANDIDATES, MAX_CAND_TEXT_LEN, MAX_COMPOSING_LEN, MAX_HW_POINTS,
    MAX_HW_STROKES,
};
pub use mode::{InputMode, InputScheme, KeyboardLayout, Language};
pub use session::{
    EditorFingerprint, EditorId, PrivacyLevel, SessionStopReason, TaskId,
};
pub use snapshot::{Candidate, CandidateSource, ComposingText, EngineStep, HotOutcome, ImmSnapshot, UiCommand};
