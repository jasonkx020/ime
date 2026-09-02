use yc_types::{Candidate, CandidateSource, HandwritingResult, Stroke, StrokeBatch, WritingMode};

use crate::templates::{self, GlyphTemplate};

const SAMPLE_POINTS: usize = 16;

#[derive(Debug)]
pub struct OnDeviceRecognizer;

impl OnDeviceRecognizer {
    pub fn new() -> Self {
        Self
    }

    pub fn infer(&self, batch: &StrokeBatch) -> HandwritingResult {
        let mut scored: Vec<(f32, GlyphTemplate)> = templates::templates()
            .into_iter()
            .map(|tpl| (Self::match_score(batch, &tpl), tpl))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let top_confidence = scored.first().map(|(s, _)| *s).unwrap_or(0.0);
        let candidates: Vec<Candidate> = scored
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, (score, tpl))| Candidate {
                id: i as u32,
                text: tpl.text.to_string(),
                source: CandidateSource::Handwriting,
                score: *score,
            })
            .collect();

        let recognized_text = if batch.writing_mode == WritingMode::Continuous {
            scored.first().map(|(_, tpl)| tpl.text.to_string())
        } else {
            None
        };

        HandwritingResult {
            candidates,
            recognized_text,
            confidence: top_confidence,
            used_cloud: false,
        }
    }

    fn match_score(batch: &StrokeBatch, template: &GlyphTemplate) -> f32 {
        if batch.strokes.is_empty() {
            return 0.0;
        }
        if batch.strokes.len() != template.strokes.len() {
            let stroke_penalty = (batch.strokes.len() as i32 - template.strokes.len() as i32).unsigned_abs();
            let base = 1.0 / (1.0 + stroke_penalty as f32);
            if base < 0.2 {
                return base * 0.5;
            }
        }

        let tpl_strokes = &template.strokes;
        let n = batch.strokes.len().min(tpl_strokes.len());
        if n == 0 {
            return 0.0;
        }
        let mut total = 0.0f32;
        for i in 0..n {
            total += stroke_distance(&batch.strokes[i], &tpl_strokes[i]);
        }
        let avg = total / n as f32;
        (1.0 - avg).clamp(0.0, 1.0)
    }
}

impl Default for OnDeviceRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

fn stroke_distance(a: &Stroke, b: &Stroke) -> f32 {
    let sa = resample_stroke(a, SAMPLE_POINTS);
    let sb = resample_stroke(b, SAMPLE_POINTS);
    let mut sum = 0.0f32;
    for (pa, pb) in sa.iter().zip(sb.iter()) {
        let dx = pa.x - pb.x;
        let dy = pa.y - pb.y;
        sum += (dx * dx + dy * dy).sqrt();
    }
    sum / SAMPLE_POINTS as f32
}

fn resample_stroke(stroke: &Stroke, count: usize) -> Vec<yc_types::StrokePoint> {
    if stroke.points.is_empty() || count == 0 {
        return Vec::new();
    }
    if stroke.points.len() == 1 {
        return vec![stroke.points[0]; count];
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        let idx = t * (stroke.points.len() - 1) as f32;
        let lo = idx.floor() as usize;
        let hi = (lo + 1).min(stroke.points.len() - 1);
        let frac = idx - lo as f32;
        let a = &stroke.points[lo];
        let b = &stroke.points[hi];
        out.push(yc_types::StrokePoint {
            x: a.x + (b.x - a.x) * frac,
            y: a.y + (b.y - a.y) * frac,
            t: a.t,
            pressure: 1.0,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::EditorId;

    fn demo_batch(text: &str) -> StrokeBatch {
        StrokeBatch {
            editor_id: EditorId(1),
            session_stroke_id: 1,
            strokes: templates::template_strokes(text).unwrap(),
            canvas_width: 320,
            canvas_height: 240,
            writing_mode: WritingMode::SingleChar,
        }
    }

    #[test]
    fn recognizes_ni_template() {
        let rec = OnDeviceRecognizer::new();
        let result = rec.infer(&demo_batch("你"));
        assert!(!result.candidates.is_empty());
        assert_eq!(result.candidates[0].text, "你");
        assert_eq!(result.candidates[0].source, CandidateSource::Handwriting);
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn returns_multiple_candidates() {
        let rec = OnDeviceRecognizer::new();
        let result = rec.infer(&demo_batch("好"));
        assert!(result.candidates.len() >= 3);
    }
}
