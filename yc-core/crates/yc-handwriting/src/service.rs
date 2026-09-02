use std::collections::HashMap;

use yc_types::{
    Candidate, EditorId, EngineError, HandwritingResult, HotResult, PrivacyLevel, Stroke,
    StrokeBatch, UiCommand, WritingMode,
};

use crate::cloud::{CloudHwRecognizer, StubCloudRecognizer};
use crate::recognizer::OnDeviceRecognizer;

#[derive(Debug, Clone)]
struct HwSession {
    strokes: Vec<Stroke>,
    undo_stack: Vec<Vec<Stroke>>,
    candidates: Vec<Candidate>,
    session_stroke_id: u64,
    canvas_width: u32,
    canvas_height: u32,
    writing_mode: WritingMode,
    pending_cloud: bool,
}

impl HwSession {
    fn new() -> Self {
        Self {
            strokes: Vec::new(),
            undo_stack: Vec::new(),
            candidates: Vec::new(),
            session_stroke_id: 0,
            canvas_width: 320,
            canvas_height: 240,
            writing_mode: WritingMode::SingleChar,
            pending_cloud: false,
        }
    }

    fn wipe(&mut self) {
        self.strokes.clear();
        self.undo_stack.clear();
        self.candidates.clear();
        self.session_stroke_id = 0;
        self.pending_cloud = false;
    }

    fn batch(&self, editor_id: EditorId) -> StrokeBatch {
        StrokeBatch {
            editor_id,
            session_stroke_id: self.session_stroke_id,
            strokes: self.strokes.clone(),
            canvas_width: self.canvas_width,
            canvas_height: self.canvas_height,
            writing_mode: self.writing_mode,
        }
    }
}

#[derive(Debug)]
pub struct HandwritingService {
    recognizer: OnDeviceRecognizer,
    cloud: StubCloudRecognizer,
    sessions: HashMap<u64, HwSession>,
}

impl HandwritingService {
    pub fn new() -> Self {
        Self {
            recognizer: OnDeviceRecognizer::new(),
            cloud: StubCloudRecognizer,
            sessions: HashMap::new(),
        }
    }

    pub fn is_allowed(&self, privacy: PrivacyLevel) -> bool {
        privacy != PrivacyLevel::ForbiddenCloud
    }

    pub fn begin(&mut self, editor_id: EditorId) {
        self.sessions
            .entry(editor_id.raw())
            .or_insert_with(HwSession::new);
    }

    pub fn remove_session(&mut self, editor_id: EditorId) {
        if let Some(mut session) = self.sessions.remove(&editor_id.raw()) {
            session.wipe();
        }
    }

    pub fn push_batch(&mut self, batch: StrokeBatch) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&batch.editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.canvas_width = batch.canvas_width;
        session.canvas_height = batch.canvas_height;
        session.writing_mode = batch.writing_mode;
        session.session_stroke_id = batch.session_stroke_id;
        session.undo_stack.push(session.strokes.clone());
        session.strokes.extend(batch.strokes);
        session.pending_cloud = false;
        Ok(())
    }

    pub fn push_stroke(&mut self, editor_id: EditorId, stroke: Stroke) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.undo_stack.push(session.strokes.clone());
        session.strokes.push(stroke);
        session.pending_cloud = false;
        Ok(())
    }

    pub fn recognize(
        &mut self,
        editor_id: EditorId,
        privacy: PrivacyLevel,
    ) -> HotResult<HandwritingResult> {
        let session = self
            .sessions
            .get(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        let batch = session.batch(editor_id);
        let result = self.recognizer.infer(&batch);
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        if result.needs_cloud_confirm && privacy == PrivacyLevel::Normal {
            session.pending_cloud = true;
            session.candidates.clear();
        } else {
            session.pending_cloud = false;
            session.candidates = result.candidates.clone();
        }
        Ok(result)
    }

    pub fn confirm_cloud(&mut self, editor_id: EditorId) -> HotResult<HandwritingResult> {
        let session = self
            .sessions
            .get(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        if !session.pending_cloud {
            return Err(EngineError::Unsupported);
        }
        let batch = session.batch(editor_id);
        let result = self.cloud.infer(&batch);
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.pending_cloud = false;
        session.candidates = result.candidates.clone();
        Ok(result)
    }

    pub fn dismiss_cloud(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.pending_cloud = false;
        session.candidates.clear();
        Ok(())
    }

    pub fn pending_cloud(&self, editor_id: EditorId) -> bool {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.pending_cloud)
            .unwrap_or(false)
    }

    pub fn candidates(&self, editor_id: EditorId) -> Vec<Candidate> {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.candidates.clone())
            .unwrap_or_default()
    }

    pub fn clear(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.undo_stack.push(session.strokes.clone());
        session.strokes.clear();
        session.candidates.clear();
        session.pending_cloud = false;
        Ok(())
    }

    pub fn undo(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        if let Some(prev) = session.undo_stack.pop() {
            session.strokes = prev;
            session.candidates.clear();
            session.pending_cloud = false;
        }
        Ok(())
    }

    pub fn select_candidate(
        &mut self,
        editor_id: EditorId,
        candidate_id: u32,
    ) -> HotResult<Vec<UiCommand>> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        let text = session
            .candidates
            .iter()
            .find(|c| c.id == candidate_id)
            .map(|c| c.text.clone())
            .ok_or(EngineError::Unsupported)?;
        session.strokes.clear();
        session.candidates.clear();
        session.undo_stack.clear();
        session.session_stroke_id += 1;
        session.pending_cloud = false;
        Ok(vec![UiCommand::Commit { text }])
    }

    pub fn stroke_count(&self, editor_id: EditorId) -> usize {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.strokes.len())
            .unwrap_or(0)
    }
}

impl Default for HandwritingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod m3_hw_cloud {
    use super::*;
    use yc_types::WritingMode;

    #[test]
    fn continuous_low_confidence_needs_cloud_confirm() {
        let mut svc = HandwritingService::new();
        let id = EditorId(1);
        svc.begin(id);
        let batch = StrokeBatch {
            editor_id: id,
            session_stroke_id: 1,
            strokes: vec![Stroke { points: vec![] }],
            canvas_width: 320,
            canvas_height: 240,
            writing_mode: WritingMode::Continuous,
        };
        svc.push_batch(batch).unwrap();
        let result = svc.recognize(id, PrivacyLevel::Normal).unwrap();
        assert!(result.needs_cloud_confirm);
        assert!(svc.pending_cloud(id));
        assert!(svc.candidates(id).is_empty());
        let cloud = svc.confirm_cloud(id).unwrap();
        assert!(cloud.used_cloud);
        assert!(!svc.candidates(id).is_empty());
    }

    #[test]
    fn password_field_rejects_cloud_path() {
        let mut svc = HandwritingService::new();
        let id = EditorId(2);
        svc.begin(id);
        let batch = StrokeBatch {
            editor_id: id,
            session_stroke_id: 1,
            strokes: vec![Stroke { points: vec![] }],
            canvas_width: 320,
            canvas_height: 240,
            writing_mode: WritingMode::Continuous,
        };
        svc.push_batch(batch).unwrap();
        let result = svc.recognize(id, PrivacyLevel::ForbiddenCloud).unwrap();
        if result.needs_cloud_confirm {
            assert!(!svc.pending_cloud(id));
        }
    }
}
