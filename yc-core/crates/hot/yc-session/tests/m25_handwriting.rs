use yc_handwriting::templates;
use yc_session::CoreServices;
use yc_types::{
    CandidateSource, EditorFingerprint, InputScheme, KeyboardLayout, UserAction, UiCommand,
    WritingMode,
};

fn fp(field: u64) -> EditorFingerprint {
    EditorFingerprint {
        package_name: "test".into(),
        field_id: field,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    }
}

fn setup_hw() -> (CoreServices, yc_types::EditorId) {
    let mut core = CoreServices::new();
    let id = core.sessions.create(fp(1));
    core.sessions.activate(id);
    core.scheduler.on_session_created(id);
    core.handwriting.begin(id);
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::OpenHandwriting,
        )
        .unwrap();
    (core, id)
}

#[test]
fn open_handwriting_switches_to_handwriting_pad() {
    let (core, id) = setup_hw();
    let mode = core.sessions.input_mode(id).unwrap();
    assert_eq!(mode.scheme, InputScheme::Handwriting);
    assert_eq!(mode.layout, KeyboardLayout::HandwritingPad);
}

#[test]
fn push_does_not_auto_recognize() {
    let (mut core, id) = setup_hw();
    let strokes = templates::template_strokes("你").unwrap();
    let batch = yc_types::StrokeBatch {
        editor_id: id,
        session_stroke_id: 1,
        strokes,
        canvas_width: 320,
        canvas_height: 240,
        writing_mode: WritingMode::SingleChar,
    };
    let outcome = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PushStrokeBatch { batch },
        )
        .unwrap();
    assert!(outcome.snapshot.candidates.is_empty());
    assert!(core.handwriting.stroke_count(id) > 0);
}

#[test]
fn recognize_produces_handwriting_candidates() {
    let (mut core, id) = setup_hw();
    let strokes = templates::template_strokes("你").unwrap();
    let batch = yc_types::StrokeBatch {
        editor_id: id,
        session_stroke_id: 1,
        strokes,
        canvas_width: 320,
        canvas_height: 240,
        writing_mode: WritingMode::SingleChar,
    };
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PushStrokeBatch { batch },
        )
        .unwrap();
    let outcome = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::RecognizeHandwriting,
        )
        .unwrap();
    assert!(!outcome.snapshot.candidates.is_empty());
    assert_eq!(
        outcome.snapshot.candidates[0].source,
        CandidateSource::Handwriting
    );
    assert_eq!(outcome.snapshot.candidates[0].text, "你");
}

#[test]
fn apply_external_handwritten_result() {
    let (mut core, id) = setup_hw();
    let texts = vec!["你".into(), "好".into()];
    let scores = [0.9f32, 0.4f32];
    let outcome = core
        .scheduler
        .apply_handwriting_result(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            &texts,
            &scores,
            None,
            false,
        )
        .unwrap();
    assert_eq!(outcome.snapshot.candidates.len(), 2);
    assert_eq!(outcome.snapshot.candidates[0].text, "你");
    assert_eq!(outcome.snapshot.status_flags & 0x2, 0);
}

#[test]
fn handwriting_pool_pages_30() {
    let (mut core, id) = setup_hw();
    let texts: Vec<String> = (0..30).map(|i| format!("字{i}")).collect();
    let scores: Vec<f32> = (0..30).map(|i| 1.0 - i as f32 * 0.01).collect();
    let outcome = core
        .scheduler
        .apply_handwriting_result(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            &texts,
            &scores,
            None,
            false,
        )
        .unwrap();
    assert_eq!(outcome.snapshot.candidates.len(), 9);
    assert_eq!(outcome.snapshot.cand_page, 0);
    assert_eq!(outcome.snapshot.cand_total, 4); // ceil(30/9)
    assert_eq!(outcome.snapshot.candidates[0].id, 0);
    assert_eq!(outcome.snapshot.candidates[0].text, "字0");

    let page1 = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PageNext,
        )
        .unwrap();
    assert_eq!(page1.snapshot.cand_page, 1);
    assert_eq!(page1.snapshot.candidates.len(), 9);
    assert_eq!(page1.snapshot.candidates[0].id, 9);
    assert_eq!(page1.snapshot.candidates[0].text, "字9");

    // Select by stable pool id (15 is on page 1)
    let commit = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::SelectCandidate { candidate_id: 15 },
        )
        .unwrap();
    assert!(matches!(
        commit.commands.first(),
        Some(UiCommand::Commit { text }) if text == "字15"
    ));
}

#[test]
fn select_hw_candidate_commits_and_clears() {
    let (mut core, id) = setup_hw();
    let strokes = templates::template_strokes("好").unwrap();
    let batch = yc_types::StrokeBatch {
        editor_id: id,
        session_stroke_id: 1,
        strokes,
        canvas_width: 320,
        canvas_height: 240,
        writing_mode: WritingMode::SingleChar,
    };
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PushStrokeBatch { batch },
        )
        .unwrap();
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::RecognizeHandwriting,
        )
        .unwrap();

    let outcome = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::SelectCandidate { candidate_id: 0 },
        )
        .unwrap();
    assert!(matches!(
        outcome.commands.first(),
        Some(UiCommand::Commit { text }) if text == "好"
    ));
    assert!(core.handwriting.candidates(id).is_empty());
    assert_eq!(core.handwriting.stroke_count(id), 0);
}

#[test]
fn clear_and_undo_handwriting() {
    let (mut core, id) = setup_hw();
    let strokes = templates::template_strokes("一").unwrap();
    let batch = yc_types::StrokeBatch {
        editor_id: id,
        session_stroke_id: 1,
        strokes,
        canvas_width: 320,
        canvas_height: 240,
        writing_mode: WritingMode::SingleChar,
    };
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PushStrokeBatch { batch },
        )
        .unwrap();
    assert_eq!(core.handwriting.stroke_count(id), 1);

    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::UndoHandwriting,
        )
        .unwrap();
    assert_eq!(core.handwriting.stroke_count(id), 0);

    let strokes2 = templates::template_strokes("人").unwrap();
    let batch2 = yc_types::StrokeBatch {
        editor_id: id,
        session_stroke_id: 2,
        strokes: strokes2,
        canvas_width: 320,
        canvas_height: 240,
        writing_mode: WritingMode::SingleChar,
    };
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::PushStrokeBatch { batch: batch2 },
        )
        .unwrap();
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::ClearHandwriting,
        )
        .unwrap();
    assert_eq!(core.handwriting.stroke_count(id), 0);
}

#[test]
fn password_field_blocks_handwriting() {
    use yc_types::VARIATION_PASSWORD;

    let mut core = CoreServices::new();
    let fp = EditorFingerprint {
        package_name: "test".into(),
        field_id: 1,
        input_type: VARIATION_PASSWORD,
        ime_options: 0,
        hint_hash: 0,
    };
    let id = core.sessions.create(fp);
    core.sessions.activate(id);
    core.scheduler.on_session_created(id);
    let result = core.scheduler.handle(
        &mut core.sessions,
        &mut core.handwriting,
        id,
        UserAction::OpenHandwriting,
    );
    assert!(result.is_err());
}
