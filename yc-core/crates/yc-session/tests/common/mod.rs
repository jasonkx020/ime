//! Shared test helpers for session integration tests.

use std::path::PathBuf;

use yc_session::{EnabledLangPack, Scheduler};
use yc_types::{KeyboardLayout, LangPackEngineSpec, UserAction};

pub fn zh_pack_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/langpacks/zh-pack-v1")
}

pub fn setup_zh_pack(scheduler: &mut Scheduler) -> String {
    let root = zh_pack_root();
    if !root.exists() {
        return String::new();
    }
    let pack_path = std::env::temp_dir().join(format!("yc_session_zh_{}.imepack", std::process::id()));
    let built = yc_pack::build_langpack_dir(&root, &pack_path).expect("build zh-pack");
    let data = std::env::temp_dir().join(format!("yc_session_zh_install_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install zh-pack");
    let install_path = data.join(&built.manifest.id);
    let spec = LangPackEngineSpec {
        pack_id: built.manifest.id.clone(),
        lexicon_path: install_path
            .join(built.manifest.lexicon.effective_dat_path())
            .to_string_lossy()
            .into(),
        install_path: install_path.to_string_lossy().into(),
        engine_kind: "data_driven".into(),
        default_scheme_id: "pinyin_full".into(),
    };
    scheduler.factory_mut().register(&spec).expect("register zh");
    scheduler.set_enabled_packs(vec![EnabledLangPack {
        pack_id: built.manifest.id.clone(),
        lang_tag: "zh".into(),
        default_scheme_id: "pinyin_full".into(),
        default_layout_id: "layout_pinyin26".into(),
    }]);
    scheduler
        .factory_mut()
        .create(&built.manifest.id, "pinyin_full")
        .expect("activate pinyin");
    built.manifest.id
}

pub fn activate_pinyin26(
    scheduler: &mut Scheduler,
    sessions: &mut yc_session::SessionManager,
    handwriting: &mut yc_handwriting::HandwritingService,
    editor_id: yc_types::EditorId,
) {
    let _ = scheduler.switch_layout(
        sessions,
        handwriting,
        editor_id,
        KeyboardLayout::Pinyin26,
    );
}

pub fn type_keys(
    scheduler: &mut Scheduler,
    sessions: &mut yc_session::SessionManager,
    handwriting: &mut yc_handwriting::HandwritingService,
    editor_id: yc_types::EditorId,
    text: &str,
) {
    for ch in text.chars() {
        scheduler
            .handle(
                sessions,
                handwriting,
                editor_id,
                UserAction::KeyPress {
                    key_code: ch as u32,
                },
            )
            .expect("key");
    }
}
