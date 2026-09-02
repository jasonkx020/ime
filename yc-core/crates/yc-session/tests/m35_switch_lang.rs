use std::path::PathBuf;

use yc_engine::EngineFactory;
use yc_handwriting::HandwritingService;
use yc_pack::build_langpack_dir;
use yc_plugin::PluginHost;
use yc_session::{EnabledLangPack, Scheduler};
use yc_types::{EditorId, UserAction};

fn hash_pack(id: &str) -> u32 {
    id.bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
}

fn switch_lang_pack(pack_dir: &str, key: char) {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/langpacks")
        .join(pack_dir);
    if !src.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join(format!("yc_session_{pack_dir}.imepack"));
    let built = build_langpack_dir(&src, &pack_path).expect("build");
    let data = std::env::temp_dir().join(format!("yc_session_lang_{pack_dir}"));
    let _ = std::fs::remove_dir_all(&data);
    let mut host = PluginHost::new(data.clone());
    host.install_lang_pack(pack_path.to_str().unwrap()).unwrap();
    let slot = host.enable(&built.manifest.id).unwrap();

    let mut factory = EngineFactory::new();
    factory
        .register_latin_pack(
            &slot.pack_id,
            slot.lexicon_path().to_str().unwrap(),
        )
        .unwrap();
    let mut scheduler = Scheduler::new(factory);
    scheduler.set_enabled_packs(vec![EnabledLangPack {
        pack_id: slot.pack_id.clone(),
        lang_tag: slot.lang_tag.clone(),
        default_scheme_id: slot.default_scheme_id.clone(),
        default_layout_id: slot.default_layout_id.clone(),
    }]);

    let mut sessions = yc_session::SessionManager::new();
    let mut hw = HandwritingService::new();
    let fp = yc_types::EditorFingerprint {
        package_name: String::new(),
        field_id: 1,
        input_type: 0,
        ime_options: 0,
        hint_hash: 0,
    };
    let id = sessions.create(fp);
    sessions.activate(id);
    scheduler.on_session_created(id);

    let hash = hash_pack(&built.manifest.id);
    let outcome = scheduler
        .handle(
            &mut sessions,
            &mut hw,
            id,
            UserAction::SwitchLang { pack_id_hash: hash },
        )
        .expect("switch");
    assert_eq!(outcome.snapshot.input_mode.layout, yc_types::KeyboardLayout::Qwerty);
    assert_eq!(outcome.snapshot.input_mode.lang_tag, built.manifest.lang);
    assert_eq!(outcome.snapshot.input_mode.active_pack_id, built.manifest.id);

    let outcome = scheduler
        .handle(
            &mut sessions,
            &mut hw,
            id,
            UserAction::KeyPress {
                key_code: key as u32,
            },
        )
        .expect("key");
    assert!(!outcome.snapshot.composing.text.is_empty());
}

#[test]
fn switch_lang_vi_fixture() {
    switch_lang_pack("vi-v1", 'x');
}

#[test]
fn switch_lang_th_fixture() {
    switch_lang_pack("th-v1", 'h');
}
