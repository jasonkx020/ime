use yc_engine::EngineFactory;
use yc_session::{Scheduler, SessionManager};
use yc_types::{
    EditorFingerprint, EditorId, InputScheme, KeyboardLayout, UserAction, UiCommand,
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

fn setup() -> (SessionManager, Scheduler, EditorId) {
    let mut sessions = SessionManager::new();
    let mut scheduler = Scheduler::new(EngineFactory::new());
    let id = sessions.create(fp(1, 0));
    sessions.activate(id);
    scheduler.on_session_created(id);
    let _ = scheduler.handle(&mut sessions, id, UserAction::Init);
    (sessions, scheduler, id)
}

#[test]
fn switch_layout_clears_composing_and_reloads_keyboard() {
    let (mut sessions, mut scheduler, id) = setup();
    for ch in "ni".chars() {
        let _ = scheduler.handle(
            &mut sessions,
            id,
            UserAction::KeyPress { key_code: ch as u32 },
        );
    }
    assert_eq!(sessions.composing(id).text, "ni");

    let outcome = scheduler
        .switch_layout(&mut sessions, id, KeyboardLayout::Qwerty)
        .unwrap();
    assert!(sessions.composing(id).text.is_empty());
    assert!(matches!(
        outcome.commands.first(),
        Some(UiCommand::ReloadKeyboard {
            layout: KeyboardLayout::Qwerty
        })
    ));
    assert_eq!(outcome.snapshot.input_mode.layout, KeyboardLayout::Qwerty);
}

#[test]
fn toggle_ascii_produces_reload() {
    let (mut sessions, mut scheduler, id) = setup();
    let outcome = scheduler.toggle_ascii(&mut sessions, id).unwrap();
    assert!(outcome.snapshot.input_mode.ascii_mode);
    assert!(matches!(
        outcome.commands.first(),
        Some(UiCommand::ReloadKeyboard { .. })
    ));
}

#[test]
fn switch_scheme_updates_mode() {
    let (mut sessions, mut scheduler, id) = setup();
    let outcome = scheduler
        .switch_scheme(&mut sessions, id, InputScheme::Qwerty)
        .unwrap();
    assert_eq!(outcome.snapshot.input_mode.scheme, InputScheme::Qwerty);
    assert_eq!(outcome.snapshot.input_mode.layout, KeyboardLayout::Qwerty);
}
