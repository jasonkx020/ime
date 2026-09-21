mod arena;
mod arena_read;
mod cold;
mod core;
mod handlers;

pub use arena_read::{parse_arena, ArenaCandidate, ArenaCommand, ArenaSnapshot};

use std::ffi::{c_char, CStr};
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
pub extern "C" fn yc_core_init(data_dir: *const c_char) -> i32 {
    ffi_guard(|| {
        let path = if data_dir.is_null() {
            PathBuf::from(".")
        } else {
            PathBuf::from(
                unsafe { CStr::from_ptr(data_dir.cast()) }
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

/// Push one stroke (normalized points) for handwriting mode (M2.5).
#[no_mangle]
pub extern "C" fn yc_hw_push_stroke(
    editor_id: u64,
    points: *const yc_types::YcStrokePoint,
    point_count: u32,
    session_stroke_id: u64,
    canvas_width: u32,
    canvas_height: u32,
    writing_mode: u32,
) -> i32 {
    ffi_guard(|| {
        use yc_types::{Stroke, StrokePoint, WritingMode, YC_ERR_BUSY, YC_ERR_INTERNAL,
                       YC_ERR_SESSION, YC_OK, MAX_HW_POINTS};

        if points.is_null() || point_count == 0 || point_count as usize > MAX_HW_POINTS {
            return YC_ERR_INTERNAL;
        }
        let wm = match WritingMode::from_raw(writing_mode) {
            Some(m) => m,
            None => return YC_ERR_INTERNAL,
        };
        let slice = unsafe { std::slice::from_raw_parts(points, point_count as usize) };
        let stroke = Stroke {
            points: slice
                .iter()
                .map(|p| StrokePoint {
                    x: p.x,
                    y: p.y,
                    t: p.t,
                    pressure: p.pressure,
                })
                .collect(),
        };
        with_core_mut(|state| {
            let id = EditorId::from_raw(editor_id);
            if !state.services.sessions.validate(id) {
                return YC_ERR_SESSION;
            }
            match state.push_hw_stroke(id, stroke, canvas_width, canvas_height, wm, session_stroke_id)
            {
                YC_OK => YC_OK,
                YC_ERR_SESSION => YC_ERR_SESSION,
                _ => YC_ERR_BUSY,
            }
        })
    })
}

/// Apply shell-side Handwritten (NCNN) recognition results.
///
/// `texts` is `count` consecutive UTF-8 C strings (each null-terminated) packed
/// into a single buffer of `texts_bytes` length. `scores` length must be `count`.
/// `flags` bit0 = needs_cloud_confirm.
#[no_mangle]
pub extern "C" fn yc_hw_apply_result(
    editor_id: u64,
    count: u32,
    texts: *const u8,
    texts_bytes: u32,
    scores: *const f32,
    flags: u32,
) -> i32 {
    ffi_guard(|| {
        use yc_types::{YC_ERR_BUSY, YC_ERR_INTERNAL, YC_ERR_SESSION, YC_OK};

        if count == 0 || count > 200 {
            return YC_ERR_INTERNAL;
        }
        if texts.is_null() || scores.is_null() || texts_bytes == 0 {
            return YC_ERR_INTERNAL;
        }
        let blob = unsafe { std::slice::from_raw_parts(texts, texts_bytes as usize) };
        let score_slice = unsafe { std::slice::from_raw_parts(scores, count as usize) };
        let mut parsed: Vec<String> = Vec::with_capacity(count as usize);
        let mut offset = 0usize;
        for _ in 0..count {
            if offset >= blob.len() {
                return YC_ERR_INTERNAL;
            }
            let end = blob[offset..]
                .iter()
                .position(|&b| b == 0)
                .map(|i| offset + i)
                .unwrap_or(blob.len());
            let s = std::str::from_utf8(&blob[offset..end]).unwrap_or("");
            parsed.push(s.to_string());
            offset = end + 1;
        }
        let needs_cloud = (flags & 0x1) != 0;
        let recognized = if parsed.len() > 1 {
            Some(parsed.join(""))
        } else {
            None
        };
        with_core_mut(|state| {
            let id = EditorId::from_raw(editor_id);
            match state.apply_hw_result(id, &parsed, score_slice, recognized, needs_cloud) {
                YC_OK => YC_OK,
                YC_ERR_SESSION => YC_ERR_SESSION,
                _ => YC_ERR_BUSY,
            }
        })
    })
}

/// Cold-path submit (M3/M3.5; requires `data` feature).
#[no_mangle]
pub extern "C" fn yc_cold_submit(
    editor_id: u64,
    kind: u32,
    payload: *const u8,
    payload_len: usize,
) -> i32 {
    ffi_guard(|| {
        if payload.is_null() && payload_len > 0 {
            return YC_ERR_INTERNAL;
        }
        let slice = if payload_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(payload, payload_len) }
        };
        #[cfg(feature = "data")]
        {
            with_core_mut(|state| crate::cold::cold_submit(&state.cold, editor_id, kind, slice))
        }
        #[cfg(not(feature = "data"))]
        {
            crate::cold::cold_submit(editor_id, kind, slice)
        }
    })
}

#[no_mangle]
pub extern "C" fn yc_cold_cancel(task_id: u32) -> i32 {
    ffi_guard(|| {
        #[cfg(feature = "data")]
        {
            with_core_mut(|state| crate::cold::cold_cancel(&state.cold, task_id))
        }
        #[cfg(not(feature = "data"))]
        {
            crate::cold::cold_cancel(task_id)
        }
    })
}

/// Register shell callback for cold-path completion (M3).
#[no_mangle]
pub extern "C" fn yc_cold_set_callback(
    callback: Option<
        extern "C" fn(
            task_id: i32,
            editor_id: u64,
            payload: *const u8,
            payload_len: usize,
            err: i32,
        ),
    >,
) -> i32 {
    ffi_guard(|| {
        #[cfg(feature = "data")]
        {
            with_core_mut(|state| {
                if let Some(cb) = callback {
                    state.cold.set_callback(Box::new(
                        move |task_id, editor_id, payload, err| {
                            cb(
                                task_id.0 as i32,
                                editor_id.raw(),
                                payload.as_ptr(),
                                payload.len(),
                                err,
                            );
                        },
                    ));
                } else {
                    state.cold.set_callback(Box::new(|_, _, _, _| {}));
                }
                YC_OK
            })
        }
        #[cfg(not(feature = "data"))]
        {
            let _ = callback;
            YC_ERR_INTERNAL
        }
    })
}

/// Sync enabled lang packs from PluginHost into Scheduler (call after cold enable).
#[no_mangle]
pub extern "C" fn yc_core_sync_lang_packs() -> i32 {
    ffi_guard(|| {
        #[cfg(feature = "data")]
        {
            with_core_mut(|state| state.sync_lang_packs())
        }
        #[cfg(not(feature = "data"))]
        {
            YC_ERR_INTERNAL
        }
    })
}

/// Synchronously install + enable an `.imepack` and sync into the hot path.
#[no_mangle]
pub extern "C" fn yc_core_install_langpack(pack_path: *const c_char) -> i32 {
    ffi_guard(|| {
        if pack_path.is_null() {
            return YC_ERR_INTERNAL;
        }
        let c_path = unsafe { CStr::from_ptr(pack_path) };
        let Ok(path) = c_path.to_str() else {
            return YC_ERR_INTERNAL;
        };
        #[cfg(feature = "data")]
        {
            with_core_mut(|state| state.install_and_enable_langpack(path))
        }
        #[cfg(not(feature = "data"))]
        {
            let _ = path;
            YC_ERR_INTERNAL
        }
    })
}

/// Cold-path personalization apply. JSON arrays (UTF-8):
/// - pairs_json: `[{"prev","next","delta"}]`
/// - deltas_json: `[{"query_key","word","boost"}]` score deltas
/// - boosts_json: `[{"lang","query_key","word","freq"}]` UserWordStore freq floors
/// Null / empty string → skip that slice. Does not block or clear composing.
#[no_mangle]
pub extern "C" fn yc_personalization_apply(
    pairs_json: *const c_char,
    deltas_json: *const c_char,
    boosts_json: *const c_char,
) -> i32 {
    ffi_guard(|| {
        #[cfg(feature = "ai")]
        {
            fn cstr_opt(p: *const c_char) -> Option<String> {
                if p.is_null() {
                    return None;
                }
                let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
                if s.trim().is_empty() {
                    None
                } else {
                    Some(s)
                }
            }
            let pairs_s = cstr_opt(pairs_json);
            let deltas_s = cstr_opt(deltas_json);
            let boosts_s = cstr_opt(boosts_json);
            with_core_mut(|state| {
                let pairs_raw = pairs_s.clone().unwrap_or_else(|| "[]".into());
                let deltas_raw = deltas_s.clone().unwrap_or_else(|| "[]".into());
                let pairs = parse_pairs(pairs_s.as_deref().unwrap_or("[]"));
                let deltas = parse_deltas(deltas_s.as_deref().unwrap_or("[]"));
                let boosts = parse_boosts(boosts_s.as_deref().unwrap_or("[]"));
                if pairs.is_err() || deltas.is_err() || boosts.is_err() {
                    return YC_ERR_INTERNAL;
                }
                let pairs = pairs.unwrap();
                let deltas = deltas.unwrap();
                let boosts = boosts.unwrap();
                state
                    .services
                    .scheduler
                    .apply_personalization(&pairs, &deltas);
                state.services.scheduler.apply_user_boosts(&boosts);
                // Persist for next process start (best-effort).
                let _ = std::fs::write(state.data_dir.join("prefer_pairs.json"), pairs_raw);
                let _ = std::fs::write(state.data_dir.join("score_deltas.json"), deltas_raw);
                YC_OK
            })
        }
        #[cfg(not(feature = "ai"))]
        {
            let _ = (pairs_json, deltas_json, boosts_json);
            YC_ERR_INTERNAL
        }
    })
}

#[cfg(feature = "ai")]
pub(crate) fn parse_pairs(raw: &str) -> Result<Vec<(String, String, f32)>, ()> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|_| ())?;
    let arr = v.as_array().ok_or(())?;
    let mut out = Vec::new();
    for e in arr.iter().take(80) {
        let prev = e.get("prev").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let next = e.get("next").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let delta = e
            .get("delta")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0) as f32;
        if prev.is_empty() || next.is_empty() || !delta.is_finite() {
            continue;
        }
        out.push((prev, next, delta.clamp(-5.0, 5.0)));
    }
    Ok(out)
}

#[cfg(feature = "ai")]
pub(crate) fn parse_deltas(raw: &str) -> Result<Vec<(String, String, f32)>, ()> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|_| ())?;
    let arr = v.as_array().ok_or(())?;
    let mut out = Vec::new();
    for e in arr.iter().take(80) {
        let qk = e
            .get("query_key")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let word = e.get("word").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let boost = e
            .get("boost")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0) as f32;
        if qk.is_empty() || word.is_empty() || !boost.is_finite() {
            continue;
        }
        out.push((qk, word, boost.clamp(-5.0, 5.0)));
    }
    Ok(out)
}

#[cfg(feature = "ai")]
fn parse_boosts(raw: &str) -> Result<Vec<(String, String, String, u32)>, ()> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|_| ())?;
    let arr = v.as_array().ok_or(())?;
    let mut out = Vec::new();
    for e in arr.iter().take(80) {
        let lang = e
            .get("lang")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let qk = e
            .get("query_key")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let word = e.get("word").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let freq = e.get("freq").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        if qk.is_empty() || word.is_empty() || freq == 0 {
            continue;
        }
        out.push((lang, qk, word, freq.min(10_000)));
    }
    Ok(out)
}

/// Poll async pinyin lookup.
/// Returns 1 if arena candidates were updated, 2 if lookup still in-flight, 0 if idle.
#[no_mangle]
pub extern "C" fn yc_hot_poll_lookup() -> i32 {
    ffi_guard(|| with_core_mut(|state| state.poll_async_lookup()))
}

/// Append LLM candidates for `query`. `texts_json` is a JSON string array.
#[no_mangle]
pub extern "C" fn yc_hot_inject_ai_candidates(
    query: *const c_char,
    texts_json: *const c_char,
) -> i32 {
    ffi_guard(|| {
        if query.is_null() || texts_json.is_null() {
            return YC_ERR_INTERNAL;
        }
        let q = unsafe { CStr::from_ptr(query) }.to_string_lossy().into_owned();
        let texts = unsafe { CStr::from_ptr(texts_json) }
            .to_string_lossy()
            .into_owned();
        with_core_mut(|state| state.inject_ai_candidates(&q, &texts))
    })
}
