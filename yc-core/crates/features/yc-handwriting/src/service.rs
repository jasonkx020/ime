use std::collections::HashMap;

use yc_types::{
    Candidate, EditorId, EngineError, HandwritingResult, HotResult, PrivacyLevel, Stroke,
    StrokeBatch, UiCommand, WritingMode,
};

use crate::cloud::{CloudHwRecognizer, StubCloudRecognizer};
use crate::recognizer::OnDeviceRecognizer;
use crate::segment::{segment_strokes, DEFAULT_GAP_MS, DEFAULT_GAP_NORM};
use yc_types::CandidateSource;

/// Arena page size (ImmSnapshot / YC_MAX_CANDIDATES).
pub const HW_PAGE_SIZE: usize = 9;
/// Max OCR / handwriting candidates retained in the session pool (paged in CandBar).
pub const HW_MAX_CANDIDATES: usize = 200;

#[derive(Debug, Clone)]
struct HwSession {
    strokes: Vec<Stroke>,
    undo_stack: Vec<Vec<Stroke>>,
    /// Full candidate pool (up to HW_MAX_CANDIDATES); ImmSnapshot shows one page.
    candidates: Vec<Candidate>,
    cand_page: u32,
    session_stroke_id: u64,
    canvas_width: u32,
    canvas_height: u32,
    writing_mode: WritingMode,
    pending_cloud: bool,
    /// Accumulated committed text for lexicon association.
    assoc_context: String,
    /// True when candidates are association suffixes (not OCR).
    assoc_active: bool,
}

impl HwSession {
    fn new() -> Self {
        Self {
            strokes: Vec::new(),
            undo_stack: Vec::new(),
            candidates: Vec::new(),
            cand_page: 0,
            session_stroke_id: 0,
            canvas_width: 320,
            canvas_height: 240,
            writing_mode: WritingMode::SingleChar,
            pending_cloud: false,
            assoc_context: String::new(),
            assoc_active: false,
        }
    }

    fn wipe(&mut self) {
        self.strokes.clear();
        self.undo_stack.clear();
        self.candidates.clear();
        self.cand_page = 0;
        self.session_stroke_id = 0;
        self.pending_cloud = false;
        self.assoc_context.clear();
        self.assoc_active = false;
    }

    fn total_pages(&self) -> u32 {
        if self.candidates.is_empty() {
            0
        } else {
            ((self.candidates.len() + HW_PAGE_SIZE - 1) / HW_PAGE_SIZE) as u32
        }
    }

    fn paged_candidates(&self) -> Vec<Candidate> {
        let start = self.cand_page as usize * HW_PAGE_SIZE;
        self.candidates
            .iter()
            .skip(start)
            .take(HW_PAGE_SIZE)
            .cloned()
            .collect()
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
        self.store_result(editor_id, &result, privacy)?;
        Ok(result)
    }

    /// Inject shell-side Handwritten (NCNN) candidates into the session.
    pub fn apply_external_result(
        &mut self,
        editor_id: EditorId,
        texts: &[String],
        scores: &[f32],
        recognized_text: Option<String>,
        needs_cloud_confirm: bool,
        privacy: PrivacyLevel,
    ) -> HotResult<HandwritingResult> {
        let _ = self
            .sessions
            .get(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        let n = texts.len().min(scores.len()).min(HW_MAX_CANDIDATES);
        let candidates: Vec<Candidate> = (0..n)
            .filter(|&i| !texts[i].is_empty())
            .map(|i| Candidate {
                id: i as u32,
                text: texts[i].clone(),
                source: CandidateSource::Handwriting,
                score: scores[i],
                code_len: 0,
            })
            .collect();
        let confidence = candidates.first().map(|c| c.score).unwrap_or(0.0);
        let result = HandwritingResult {
            candidates,
            recognized_text,
            confidence,
            used_cloud: false,
            needs_cloud_confirm,
        };
        self.store_result(editor_id, &result, privacy)?;
        Ok(result)
    }

    fn store_result(
        &mut self,
        editor_id: EditorId,
        result: &HandwritingResult,
        privacy: PrivacyLevel,
    ) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        if result.needs_cloud_confirm && privacy == PrivacyLevel::Normal {
            session.pending_cloud = true;
            session.candidates.clear();
            session.cand_page = 0;
        } else {
            session.pending_cloud = false;
            session.candidates = result.candidates.clone();
            session.cand_page = 0;
            // OCR / template results replace association context.
            session.assoc_context.clear();
            session.assoc_active = false;
        }
        Ok(())
    }

    /// Segment current strokes for Continuous recognition (shell / tests).
    pub fn segmented_strokes(&self, editor_id: EditorId) -> Vec<Vec<Stroke>> {
        let Some(session) = self.sessions.get(&editor_id.raw()) else {
            return Vec::new();
        };
        segment_strokes(&session.strokes, DEFAULT_GAP_MS, DEFAULT_GAP_NORM)
    }

    pub fn strokes(&self, editor_id: EditorId) -> Vec<Stroke> {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.strokes.clone())
            .unwrap_or_default()
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
        session.cand_page = 0;
        Ok(result)
    }

    pub fn dismiss_cloud(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.pending_cloud = false;
        session.candidates.clear();
        session.cand_page = 0;
        Ok(())
    }

    pub fn pending_cloud(&self, editor_id: EditorId) -> bool {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.pending_cloud)
            .unwrap_or(false)
    }

    /// Current ImmSnapshot page (stable ids from the full pool).
    pub fn candidates(&self, editor_id: EditorId) -> Vec<Candidate> {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.paged_candidates())
            .unwrap_or_default()
    }

    pub fn cand_page(&self, editor_id: EditorId) -> u32 {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.cand_page)
            .unwrap_or(0)
    }

    pub fn total_pages(&self, editor_id: EditorId) -> u32 {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.total_pages())
            .unwrap_or(0)
    }

    pub fn page_next(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        let total = session.total_pages();
        if total > 0 && session.cand_page + 1 < total {
            session.cand_page += 1;
        }
        Ok(())
    }

    pub fn page_prev(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        if session.cand_page > 0 {
            session.cand_page -= 1;
        }
        Ok(())
    }

    pub fn clear(&mut self, editor_id: EditorId) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        session.undo_stack.push(session.strokes.clone());
        session.strokes.clear();
        session.candidates.clear();
        session.cand_page = 0;
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
            session.cand_page = 0;
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
        let was_assoc = session.assoc_active;
        if clears_hw_assoc_context(&text) {
            session.assoc_context.clear();
            session.assoc_active = false;
        } else if was_assoc {
            session.assoc_context.push_str(&text);
        } else {
            session.assoc_context = text.clone();
        }
        session.strokes.clear();
        session.candidates.clear();
        session.cand_page = 0;
        session.undo_stack.clear();
        session.session_stroke_id += 1;
        session.pending_cloud = false;
        Ok(vec![UiCommand::Commit { text }])
    }

    pub fn assoc_context(&self, editor_id: EditorId) -> String {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.assoc_context.clone())
            .unwrap_or_default()
    }

    pub fn clear_assoc_context(&mut self, editor_id: EditorId) {
        if let Some(session) = self.sessions.get_mut(&editor_id.raw()) {
            session.assoc_context.clear();
            session.assoc_active = false;
        }
    }

    /// Inject lexicon association suffixes after select (source should be Hot).
    pub fn set_association_candidates(
        &mut self,
        editor_id: EditorId,
        mut candidates: Vec<Candidate>,
    ) -> HotResult<()> {
        let session = self
            .sessions
            .get_mut(&editor_id.raw())
            .ok_or(EngineError::SessionInvalid)?;
        for (i, c) in candidates.iter_mut().enumerate() {
            c.id = i as u32;
            c.source = CandidateSource::Hot;
        }
        let n = candidates.len().min(HW_MAX_CANDIDATES);
        session.candidates = candidates.into_iter().take(n).collect();
        session.cand_page = 0;
        session.assoc_active = !session.candidates.is_empty();
        Ok(())
    }

    pub fn stroke_count(&self, editor_id: EditorId) -> usize {
        self.sessions
            .get(&editor_id.raw())
            .map(|s| s.strokes.len())
            .unwrap_or(0)
    }
}

fn clears_hw_assoc_context(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '。' | '！' | '？' | '；' | '，' | '.' | '!' | '?' | ';' | ',' | '\n' | '\r'
        )
    })
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
