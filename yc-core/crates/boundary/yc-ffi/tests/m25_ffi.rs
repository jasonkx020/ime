use std::ffi::CString;
use std::sync::{Mutex, MutexGuard};

use yc_ffi::{
    yc_core_init, yc_core_shutdown, yc_hot_submit, yc_hw_push_stroke, yc_session_begin_with_input,
};
use yc_handwriting::templates;
use yc_types::{HotActionType, YC_OK, YcHotAction, YcStrokePoint, WritingMode};

static FFI_TEST_LOCK: Mutex<()> = Mutex::new(());

fn ffi_setup() -> MutexGuard<'static, ()> {
    let lock = FFI_TEST_LOCK.lock().unwrap();
    yc_core_shutdown();
    let dir = CString::new(".").unwrap();
    assert_eq!(yc_core_init(dir.as_ptr()), YC_OK);
    lock
}

#[test]
fn ffi_handwriting_recognize_and_select() {
    let _guard = ffi_setup();
    let editor_id = yc_session_begin_with_input(1, 0);
    assert_ne!(editor_id, 0);

    let open = YcHotAction {
        editor_id,
        client_seq: 1,
        action_type: HotActionType::OpenHandwriting as u32,
        key_code: 0,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&open), YC_OK);

    let strokes = templates::template_strokes("你").unwrap();
    for stroke in strokes {
        let points: Vec<YcStrokePoint> = stroke
            .points
            .iter()
            .map(|p| YcStrokePoint {
                x: p.x,
                y: p.y,
                t: p.t,
                pressure: p.pressure,
            })
            .collect();
        assert_eq!(
            yc_hw_push_stroke(
                editor_id,
                points.as_ptr(),
                points.len() as u32,
                1,
                320,
                240,
                WritingMode::SingleChar.raw(),
            ),
            YC_OK
        );
    }

    let recognize = YcHotAction {
        editor_id,
        client_seq: 2,
        action_type: HotActionType::RecognizeHandwriting as u32,
        key_code: 0,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&recognize), YC_OK);

    let select = YcHotAction {
        editor_id,
        client_seq: 3,
        action_type: HotActionType::SelectCandidate as u32,
        key_code: 0,
        candidate_id: 0,
        flags: 0,
        reserved: [0; 8],
    };
    assert_eq!(yc_hot_submit(&select), YC_OK);
    yc_core_shutdown();
}
