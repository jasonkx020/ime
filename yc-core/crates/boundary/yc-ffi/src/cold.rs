#[cfg(feature = "data")]
use yc_data::ColdPathRuntime;
#[cfg(feature = "data")]
use yc_types::{EditorId, YC_OK};
use yc_types::YC_ERR_BUSY;

#[cfg(feature = "data")]
pub fn cold_submit(runtime: &ColdPathRuntime, editor_id: u64, kind: u32, payload: &[u8]) -> i32 {
    match runtime.submit(EditorId::from_raw(editor_id), kind, payload) {
        Ok(_task) => YC_OK,
        Err(yc_types::EngineError::Busy) => YC_ERR_BUSY,
        Err(_) => YC_ERR_BUSY,
    }
}

#[cfg(not(feature = "data"))]
pub fn cold_submit(editor_id: u64, kind: u32, payload: &[u8]) -> i32 {
    let _ = (editor_id, kind, payload);
    YC_ERR_BUSY
}

#[cfg(feature = "data")]
pub fn cold_cancel(runtime: &ColdPathRuntime, task_id: u32) -> i32 {
    use yc_types::TaskId;
    match runtime.cancel(TaskId(task_id as u64)) {
        Ok(()) => YC_OK,
        Err(_) => YC_ERR_BUSY,
    }
}

#[cfg(not(feature = "data"))]
pub fn cold_cancel(task_id: u32) -> i32 {
    let _ = task_id;
    YC_ERR_BUSY
}
