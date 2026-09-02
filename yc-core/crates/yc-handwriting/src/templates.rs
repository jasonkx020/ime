//! Hardcoded normalized stroke templates for M2.5 demo (你、好、我、一、人).

use yc_types::{Stroke, StrokePoint};

pub struct GlyphTemplate {
    pub text: &'static str,
    pub strokes: Vec<Stroke>,
}

fn stroke(points: &[(f32, f32, u64)]) -> Stroke {
    Stroke {
        points: points
            .iter()
            .map(|(x, y, t)| StrokePoint {
                x: *x,
                y: *y,
                t: *t,
                pressure: 1.0,
            })
            .collect(),
    }
}

fn all_templates() -> Vec<GlyphTemplate> {
    vec![
        GlyphTemplate {
            text: "一",
            strokes: vec![stroke(&[
                (0.2, 0.5, 0),
                (0.35, 0.5, 10),
                (0.5, 0.5, 20),
                (0.65, 0.5, 30),
                (0.8, 0.5, 40),
            ])],
        },
        GlyphTemplate {
            text: "人",
            strokes: vec![
                stroke(&[
                    (0.5, 0.2, 0),
                    (0.45, 0.45, 15),
                    (0.4, 0.7, 30),
                    (0.35, 0.85, 45),
                ]),
                stroke(&[
                    (0.5, 0.2, 50),
                    (0.55, 0.45, 65),
                    (0.65, 0.7, 80),
                    (0.75, 0.85, 95),
                ]),
            ],
        },
        GlyphTemplate {
            text: "好",
            strokes: vec![
                stroke(&[
                    (0.25, 0.2, 0),
                    (0.25, 0.5, 15),
                    (0.25, 0.75, 30),
                    (0.25, 0.9, 45),
                ]),
                stroke(&[(0.1, 0.45, 50), (0.25, 0.45, 60), (0.4, 0.45, 70)]),
                stroke(&[
                    (0.65, 0.25, 80),
                    (0.65, 0.55, 95),
                    (0.65, 0.8, 110),
                    (0.65, 0.9, 120),
                ]),
            ],
        },
        GlyphTemplate {
            text: "你",
            strokes: vec![
                stroke(&[
                    (0.3, 0.15, 0),
                    (0.3, 0.4, 12),
                    (0.3, 0.65, 24),
                    (0.3, 0.85, 36),
                    (0.3, 0.95, 48),
                ]),
                stroke(&[
                    (0.55, 0.2, 55),
                    (0.55, 0.45, 67),
                    (0.55, 0.7, 79),
                    (0.7, 0.85, 91),
                    (0.8, 0.9, 103),
                ]),
            ],
        },
        GlyphTemplate {
            text: "我",
            strokes: vec![
                stroke(&[
                    (0.35, 0.15, 0),
                    (0.35, 0.45, 15),
                    (0.35, 0.75, 30),
                    (0.35, 0.9, 45),
                ]),
                stroke(&[
                    (0.15, 0.35, 50),
                    (0.35, 0.35, 60),
                    (0.55, 0.35, 70),
                    (0.75, 0.35, 80),
                ]),
                stroke(&[
                    (0.6, 0.5, 90),
                    (0.65, 0.65, 100),
                    (0.7, 0.8, 110),
                    (0.75, 0.9, 120),
                ]),
            ],
        },
    ]
}

pub fn templates() -> Vec<GlyphTemplate> {
    all_templates()
}

pub fn template_strokes(text: &str) -> Option<Vec<Stroke>> {
    all_templates()
        .into_iter()
        .find(|t| t.text == text)
        .map(|t| t.strokes)
}
