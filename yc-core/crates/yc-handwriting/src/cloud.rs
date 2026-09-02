use yc_types::{Candidate, HandwritingResult, StrokeBatch};

pub trait CloudHwRecognizer {
    fn infer(&self, batch: &StrokeBatch) -> HandwritingResult;
}

#[derive(Debug, Default)]
pub struct StubCloudRecognizer;

impl CloudHwRecognizer for StubCloudRecognizer {
    fn infer(&self, _batch: &StrokeBatch) -> HandwritingResult {
        HandwritingResult {
            candidates: vec![Candidate {
                id: 0,
                text: "云识别".into(),
                source: yc_types::CandidateSource::Handwriting,
                score: 0.85,
            }],
            recognized_text: Some("云识别".into()),
            confidence: 0.85,
            used_cloud: true,
            needs_cloud_confirm: false,
        }
    }
}
