use std::ffi::CString;
use std::sync::{Mutex, MutexGuard};

use yc_ffi::{
    parse_arena, yc_core_init, yc_core_shutdown, yc_hot_arena_ptr, yc_hot_latest_seq, yc_hot_submit,
    yc_session_begin, yc_session_validate,
};
use yc_types::{HotActionType, YC_ERR_BUSY, YC_OK, YcHotAction};

static FFI_TEST_LOCK: Mutex<()> = Mutex::new(());

fn ffi_setup() -> MutexGuard<'static, ()> {
    let lock = FFI_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    yc_core_shutdown();
    let dir = CString::new(".").unwrap();
    assert_eq!(yc_core_init(dir.as_ptr()), YC_OK);
    lock
}

#[test]
fn ffi_smoke_hot_path() {
    let _guard = ffi_setup();
    let editor_id = yc_session_begin(42);
    assert_ne!(editor_id, 0);
    assert_eq!(yc_session_validate(editor_id), 1);

    let action = YcHotAction {
        editor_id,
        client_seq: 1,
        action_type: HotActionType::Init as u32,
        key_code: 0,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&action), YC_OK);

    let mut seq = 0u64;
    assert_eq!(yc_hot_latest_seq(editor_id, &mut seq), YC_OK);
    assert!(seq > 0);

    assert!(!yc_hot_arena_ptr().is_null());
    yc_core_shutdown();
}

#[test]
fn ffi_keypress_without_langpack_returns_busy() {
    let _guard = ffi_setup();
    let editor_id = yc_session_begin(1);

    let action = YcHotAction {
        editor_id,
        client_seq: 0,
        action_type: HotActionType::KeyPress as u32,
        key_code: b'n' as u32,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&action), YC_ERR_BUSY);

    let ptr = yc_hot_arena_ptr();
    assert!(!ptr.is_null());
    let slice = unsafe { std::slice::from_raw_parts(ptr, yc_ffi::yc_hot_arena_size()) };
    let snap = parse_arena(slice).unwrap();
    assert!(snap.composing.is_empty());
    yc_core_shutdown();
}
