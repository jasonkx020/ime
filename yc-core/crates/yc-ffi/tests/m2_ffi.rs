use std::ffi::CString;
use std::sync::{Mutex, MutexGuard};

use yc_ffi::{
    parse_arena, yc_core_init, yc_core_shutdown, yc_hot_arena_ptr, yc_hot_arena_size,
    yc_hot_submit, yc_session_begin_with_input, yc_session_validate,
};
use yc_types::{HotActionType, KeyboardLayout, YC_OK, YcHotAction, VARIATION_PASSWORD};

static FFI_TEST_LOCK: Mutex<()> = Mutex::new(());

fn ffi_setup() -> MutexGuard<'static, ()> {
    let lock = FFI_TEST_LOCK.lock().unwrap();
    yc_core_shutdown();
    let dir = CString::new(".").unwrap();
    assert_eq!(yc_core_init(dir.as_ptr()), YC_OK);
    lock
}

#[test]
fn ffi_switch_layout_smoke() {
    let _guard = ffi_setup();
    let editor_id = yc_session_begin_with_input(1, 0);
    assert_ne!(editor_id, 0);
    assert_eq!(yc_session_validate(editor_id), 1);

    let action = YcHotAction {
        editor_id,
        client_seq: 0,
        action_type: HotActionType::SwitchLayout as u32,
        key_code: KeyboardLayout::Qwerty.raw(),
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&action), YC_OK);

    let ptr = yc_hot_arena_ptr();
    assert!(!ptr.is_null());
    let size = yc_hot_arena_size();
    let slice = unsafe { std::slice::from_raw_parts(ptr, size) };
    let snapshot = parse_arena(slice).unwrap();
    assert!(snapshot.candidates.is_empty());
    assert!(snapshot.composing.is_empty());

    yc_core_shutdown();
}

#[test]
fn ffi_password_session() {
    let _guard = ffi_setup();
    let editor_id = yc_session_begin_with_input(99, VARIATION_PASSWORD);
    assert_ne!(editor_id, 0);
    let action = YcHotAction {
        editor_id,
        client_seq: 0,
        action_type: HotActionType::KeyPress as u32,
        key_code: b'a' as u32,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&action), YC_OK);
    yc_core_shutdown();
}
