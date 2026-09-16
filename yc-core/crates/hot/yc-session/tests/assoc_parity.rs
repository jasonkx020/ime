//! Pinyin and handwriting must share the same association algorithm output.

mod common;

use yc_session::CoreServices;
use yc_types::{EditorFingerprint, UserAction};

fn fp(field: u64) -> EditorFingerprint {
    EditorFingerprint {
        package_name: "test".into(),
        field_id: field,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    }
}

#[test]
fn pinyin_and_handwriting_assoc_ni_identical() {
    if !common::zh_pack_root().exists() {
        return;
    }
    let mut core = CoreServices::new();
    common::setup_zh_pack(&mut core.scheduler);

    let id = core.sessions.create(fp(1));
    core.sessions.activate(id);
    core.scheduler.on_session_created(id);

    // --- Pinyin: ni → select 你 ---
    common::activate_pinyin26(
        &mut core.scheduler,
        &mut core.sessions,
        &mut core.handwriting,
        id,
    );
    common::type_keys(
        &mut core.scheduler,
        &mut core.sessions,
        &mut core.handwriting,
        id,
        "ni",
    );
    let page = core.scheduler.factory_mut().active_paged_candidates();
    let ni_id = page
        .iter()
        .find(|c| c.text == "你")
        .map(|c| c.id)
        .expect("你 in pinyin candidates for ni");
    let py_out = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::SelectCandidate {
                candidate_id: ni_id,
            },
        )
        .expect("pinyin select");
    let py_texts: Vec<String> = py_out
        .snapshot
        .candidates
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        py_texts.iter().any(|t| t == "好" || t == "们"),
        "pinyin assoc after 你: {:?}",
        py_texts
    );

    // --- Handwriting: OCR 你 → select ---
    core.scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::OpenHandwriting,
        )
        .expect("open hw");
    core.scheduler
        .apply_handwriting_result(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            &["你".into()],
            &[0.99],
            Some("你".into()),
            false,
        )
        .expect("hw apply");
    let hw_out = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::SelectCandidate { candidate_id: 0 },
        )
        .expect("hw select");
    let hw_texts: Vec<String> = hw_out
        .snapshot
        .candidates
        .iter()
        .map(|c| c.text.clone())
        .collect();

    assert_eq!(
        py_texts, hw_texts,
        "pinyin and handwriting association for 你 must be identical"
    );
}
