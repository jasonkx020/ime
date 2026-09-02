mod common;

use yc_session::CoreServices;
use yc_types::{EditorFingerprint, UserAction};

#[test]
fn session_isolation_composing() {
    let mut core = CoreServices::new();
    if !common::zh_pack_root().exists() {
        return;
    }
    common::setup_zh_pack(&mut core.scheduler);

    let fp_a = EditorFingerprint {
        package_name: "app".into(),
        field_id: 1,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    };
    let fp_b = EditorFingerprint {
        package_name: "app".into(),
        field_id: 2,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    };

    let a = core.sessions.create(fp_a);
    let b = core.sessions.create(fp_b);
    core.sessions.activate(a);
    core.scheduler.on_session_created(a);
    common::activate_pinyin26(
        &mut core.scheduler,
        &mut core.sessions,
        &mut core.handwriting,
        a,
    );

    common::type_keys(
        &mut core.scheduler,
        &mut core.sessions,
        &mut core.handwriting,
        a,
        "nihao",
    );

    core.sessions.activate(b);
    core.scheduler.on_session_created(b);
    let outcome = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            b,
            UserAction::Init,
        )
        .unwrap();

    assert!(outcome.snapshot.composing.text.is_empty());
    assert!(outcome.snapshot.candidates.is_empty());
}

#[test]
fn hot_path_select_commit() {
    let mut core = CoreServices::new();
    if !common::zh_pack_root().exists() {
        return;
    }
    common::setup_zh_pack(&mut core.scheduler);

    let fp = EditorFingerprint {
        package_name: "app".into(),
        field_id: 1,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    };
    let id = core.sessions.create(fp);
    core.sessions.activate(id);
    core.scheduler.on_session_created(id);
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
        "nihao",
    );

    let outcome = core
        .scheduler
        .handle(
            &mut core.sessions,
            &mut core.handwriting,
            id,
            UserAction::SelectCandidate { candidate_id: 0 },
        )
        .unwrap();

    assert!(outcome
        .commands
        .iter()
        .any(|c| matches!(c, yc_types::UiCommand::Commit { text } if text == "你好")));
}
