mod arena;
mod arena_read;
mod core;

pub use arena_read::{parse_arena, ArenaCandidate, ArenaSnapshot};

use std::ffi::CStr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::OnceLock;

use parking_lot::Mutex;
use yc_types::{HotActionType, YC_ERR_INTERNAL, YC_OK, YcHotAction, EditorId, SessionStopReason, UserAction};

use crate::core::CoreState;

static CORE: OnceLock<Mutex<Option<CoreState>>> = OnceLock::new();

fn core_lock() -> &'static Mutex<Option<CoreState>> {
    CORE.get_or_init(|| Mutex::new(None))
}

fn ffi_guard<F: FnOnce() -> i32>(f: F) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => YC_ERR_INTERNAL,
    }
}

fn with_core_mut<F: FnOnce(&mut CoreState) -> i32>(f: F) -> i32 {
    let guard = core_lock().lock();
    match guard.as_ref() {
        Some(_) => {
            drop(guard);
            let mut guard = core_lock().lock();
            if let Some(state) = guard.as_mut() {
                f(state)
            } else {
                YC_ERR_INTERNAL
            }
        }
        None => YC_ERR_INTERNAL,
    }
}

#[no_mangle]
pub extern "C" fn yc_core_init(data_dir: *const i8) -> i32 {
    ffi_guard(|| {
        let path = if data_dir.is_null() {
            PathBuf::from(".")
        } else {
            PathBuf::from(
                unsafe { CStr::from_ptr(data_dir) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        let mut guard = core_lock().lock();
        *guard = Some(CoreState::new(path));
        YC_OK
    })
}

#[no_mangle]
pub extern "C" fn yc_core_shutdown() {
    let mut guard = core_lock().lock();
    *guard = None;
}

/// Simplified session start for M1 (shell passes field_id).
#[no_mangle]
pub extern "C" fn yc_session_begin(field_id: u64) -> u64 {
    yc_session_begin_with_input(field_id, 0)
}

/// Session start with Android-style input_type flags (M2).
#[no_mangle]
pub extern "C" fn yc_session_begin_with_input(field_id: u64, input_type: u32) -> u64 {
    let mut guard = core_lock().lock();
    match guard.as_mut() {
        Some(state) => state.begin_session(field_id, input_type).raw(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn yc_hot_submit(action: *const YcHotAction) -> i32 {
    ffi_guard(|| {
        if action.is_null() {
            return YC_ERR_INTERNAL;
        }
        let action = unsafe { &*action };
        let editor_id = EditorId::from_raw(action.editor_id);
        let action_type = match HotActionType::from_raw(action.action_type) {
            Some(t) => t,
            None => return YC_ERR_INTERNAL,
        };
        let user_action = match UserAction::from_hot_action(
            action_type,
            action.key_code,
            action.candidate_id,
        ) {
            Some(a) => a,
            None => return YC_ERR_INTERNAL,
        };
        with_core_mut(|state| state.submit_action(editor_id, user_action))
    })
}

#[no_mangle]
pub extern "C" fn yc_hot_arena_ptr() -> *const u8 {
    let guard = core_lock().lock();
    match guard.as_ref() {
        Some(state) => state.arena.ptr(),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub extern "C" fn yc_hot_arena_size() -> usize {
    let guard = core_lock().lock();
    match guard.as_ref() {
        Some(state) => state.arena.size(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn yc_hot_latest_seq(editor_id: u64, out_seq: *mut u64) -> i32 {
    ffi_guard(|| {
        if out_seq.is_null() {
            return YC_ERR_INTERNAL;
        }
        with_core_mut(|state| {
            let seq = state
                .services
                .sessions
                .latest_seq(EditorId::from_raw(editor_id));
            unsafe {
                *out_seq = seq;
            }
            YC_OK
        })
    })
}

#[no_mangle]
pub extern "C" fn yc_session_get_active() -> u64 {
    let guard = core_lock().lock();
    match guard.as_ref() {
        Some(state) => state.services.sessions.get_active().raw(),
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn yc_session_validate(editor_id: u64) -> i32 {
    let guard = core_lock().lock();
    match guard.as_ref() {
        Some(state) => {
            if state
                .services
                .sessions
                .validate(EditorId::from_raw(editor_id))
            {
                1
            } else {
                0
            }
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn yc_session_stop(editor_id: u64, reason: u32) {
    let mut guard = core_lock().lock();
    if let Some(state) = guard.as_mut() {
        state.stop_session(
            EditorId::from_raw(editor_id),
            SessionStopReason::from_raw(reason),
        );
    }
}
