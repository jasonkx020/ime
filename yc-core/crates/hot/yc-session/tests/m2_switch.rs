mod common;

use yc_engine::EngineFactory;
use yc_handwriting::HandwritingService;
use yc_session::{Scheduler, SessionManager};
use yc_types::{
    EditorFingerprint, EditorId, InputScheme, KeyboardLayout, UiCommand, UserAction,
};

fn fp(field: u64, input_type: u32) -> EditorFingerprint {
    EditorFingerprint {
        package_name: "test".into(),
        field_id: field,
        input_type,
        ime_options: 0,
        hint_hash: 0,
    }
}

fn setup_basic() -> (SessionManager, Scheduler, HandwritingService, EditorId) {
    let mut sessions = SessionManager::new();
    let mut scheduler = Scheduler::new(EngineFactory::new());
    let mut handwriting = HandwritingService::new();
    let id = sessions.create(fp(1, 0));
    sessions.activate(id);
    scheduler.on_session_created(id);
    handwriting.begin(id);
    (sessions, scheduler, handwriting, id)
}

fn setup() -> (SessionManager, Scheduler, HandwritingService, EditorId) {
    let mut sessions = SessionManager::new();
    let mut scheduler = Scheduler::new(EngineFactory::new());
    if common::zh_pack_root().exists() {
        common::setup_zh_pack(&mut scheduler);
    }
    let mut handwriting = HandwritingService::new();
    let id = sessions.create(fp(1, 0));
    sessions.activate(id);
    scheduler.on_session_created(id);
    handwriting.begin(id);
    let _ = scheduler.handle(
        &mut sessions,
        &mut handwriting,
        id,
        UserAction::Init,
    );
    if common::zh_pack_root().exists() {
        common::activate_pinyin26(&mut scheduler, &mut sessions, &mut handwriting, id);
    }
    (sessions, scheduler, handwriting, id)
}

#[test]
fn switch_layout_clears_composing_and_reloads_keyboard() {
    if !common::zh_pack_root().exists() {
        return;
    }
    let (mut sessions, mut scheduler, mut handwriting, id) = setup();
    for ch in "ni".chars() {
        let _ = scheduler.handle(
            &mut sessions,
            &mut handwriting,
            id,
            UserAction::KeyPress { key_code: ch as u32 },
        );
    }
    assert_eq!(sessions.composing(id).text, "ni");

    let outcome = scheduler
        .switch_layout(&mut sessions, &mut handwriting, id, KeyboardLayout::Qwerty)
        .unwrap();
    assert!(sessions.composing(id).text.is_empty());
    assert!(matches!(
        outcome.commands.first(),
        Some(UiCommand::ReloadKeyboard {
            layout: KeyboardLayout::Qwerty,
            ..
        })
    ));
    assert_eq!(outcome.snapshot.input_mode.layout, KeyboardLayout::Qwerty);
}

#[test]
fn toggle_ascii_produces_reload() {
    let (mut sessions, mut scheduler, mut handwriting, id) = setup_basic();
    let outcome = scheduler
        .toggle_ascii(&mut sessions, &mut handwriting, id)
        .unwrap();
    assert!(outcome.snapshot.input_mode.ascii_mode);
    assert!(matches!(
        outcome.commands.first(),
        Some(UiCommand::ReloadKeyboard { .. })
    ));
}

#[test]
fn switch_scheme_updates_mode() {
    let (mut sessions, mut scheduler, mut handwriting, id) = setup_basic();
    let outcome = scheduler
        .switch_scheme(&mut sessions, &mut handwriting, id, InputScheme::Qwerty)
        .unwrap();
    assert_eq!(outcome.snapshot.input_mode.scheme, InputScheme::Qwerty);
    assert_eq!(outcome.snapshot.input_mode.layout, KeyboardLayout::Qwerty);
}
