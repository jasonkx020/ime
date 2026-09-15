//! Continuous handwriting stroke segmentation by time / spatial gaps.

use yc_types::Stroke;

/// Default pen-up gap (ms) that starts a new character cluster.
pub const DEFAULT_GAP_MS: u64 = 280;
/// Normalized canvas distance between stroke endpoints that forces a split.
pub const DEFAULT_GAP_NORM: f32 = 0.35;

/// Split strokes into character clusters for Continuous mode.
pub fn segment_strokes(
    strokes: &[Stroke],
    gap_ms: u64,
    gap_norm: f32,
) -> Vec<Vec<Stroke>> {
    if strokes.is_empty() {
        return Vec::new();
    }
    let mut clusters: Vec<Vec<Stroke>> = Vec::new();
    let mut current: Vec<Stroke> = Vec::new();
    let mut prev_end_t: Option<u64> = None;
    let mut prev_end_xy: Option<(f32, f32)> = None;

    for stroke in strokes {
        let (start_t, start_xy) = stroke_start(stroke);
        let split = if let (Some(pt), Some((px, py))) = (prev_end_t, prev_end_xy) {
            let dt = start_t.saturating_sub(pt);
            let dx = start_xy.0 - px;
            let dy = start_xy.1 - py;
            let dist = (dx * dx + dy * dy).sqrt();
            dt >= gap_ms || dist >= gap_norm
        } else {
            false
        };
        if split && !current.is_empty() {
            clusters.push(std::mem::take(&mut current));
        }
        let (end_t, end_xy) = stroke_end(stroke);
        prev_end_t = Some(end_t);
        prev_end_xy = Some(end_xy);
        current.push(stroke.clone());
    }
    if !current.is_empty() {
        clusters.push(current);
    }
    clusters
}

fn stroke_start(stroke: &Stroke) -> (u64, (f32, f32)) {
    match stroke.points.first() {
        Some(p) => (p.t, (p.x, p.y)),
        None => (0, (0.5, 0.5)),
    }
}

fn stroke_end(stroke: &Stroke) -> (u64, (f32, f32)) {
    match stroke.points.last() {
        Some(p) => (p.t, (p.x, p.y)),
        None => (0, (0.5, 0.5)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::StrokePoint;

    fn pt(x: f32, y: f32, t: u64) -> StrokePoint {
        StrokePoint {
            x,
            y,
            t,
            pressure: 1.0,
        }
    }

    #[test]
    fn splits_on_time_gap() {
        let strokes = vec![
            Stroke {
                points: vec![pt(0.2, 0.2, 0), pt(0.3, 0.3, 50)],
            },
            Stroke {
                points: vec![pt(0.7, 0.2, 400), pt(0.8, 0.3, 450)],
            },
        ];
        let clusters = segment_strokes(&strokes, 280, 0.9);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn keeps_close_strokes_together() {
        let strokes = vec![
            Stroke {
                points: vec![pt(0.2, 0.2, 0), pt(0.3, 0.3, 50)],
            },
            Stroke {
                points: vec![pt(0.35, 0.2, 80), pt(0.4, 0.3, 120)],
            },
        ];
        let clusters = segment_strokes(&strokes, 280, 0.35);
        assert_eq!(clusters.len(), 1);
    }
}
