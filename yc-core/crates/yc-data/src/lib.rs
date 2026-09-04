//! Cold-path repository and async IO (M3/M3.5).

mod repository;
mod runtime;
mod user_words;

pub use repository::Repository;
pub use runtime::{ColdCallback, ColdPathRuntime};
pub use user_words::open_user_words;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use yc_pack::build_skin_dir;
    use yc_types::{ColdKind, EditorId};

    fn fixture_skin_src() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/skins/samsung-light")
    }

    #[test]
    fn cold_skin_callback() {
        let dir = std::env::temp_dir().join("yc_data_test_skin");
        let _ = std::fs::remove_dir_all(&dir);
        let rt = ColdPathRuntime::new(dir.clone());
        let done = Arc::new(Mutex::new(false));
        let done2 = done.clone();
        rt.set_callback(Box::new(move |_tid, _eid, payload, err| {
            if err == 0 && !payload.is_empty() {
                *done2.lock().unwrap() = true;
            }
        }));
        let src = fixture_skin_src();
        if !src.exists() {
            return;
        }
        let skin_path = dir.join("samsung-light.imeskin");
        build_skin_dir(&src, &skin_path).expect("build skin");
        let path = skin_path.to_string_lossy();
        rt.submit(EditorId(1), ColdKind::Skin.raw(), path.as_bytes())
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(*done.lock().unwrap());
    }

    #[test]
    fn cold_submit_returns_task_id() {
        let dir = std::env::temp_dir().join("yc_data_test_submit");
        let rt = ColdPathRuntime::new(dir);
        let tid = rt
            .submit(EditorId(1), ColdKind::HandwritingCloud.raw(), b"{}")
            .unwrap();
        assert!(tid.0 > 0);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
