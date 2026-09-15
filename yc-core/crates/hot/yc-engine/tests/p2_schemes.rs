use std::path::PathBuf;

use yc_engine::{DataDrivenEngine, EngineFactory};
use yc_pack::build_langpack_dir;
use yc_scheme::compile_scheme_yaml;
use yc_scheme::SchemeDesc;
use yc_engine::InputEngine;
use yc_types::{EditorId, LangPackEngineSpec};

#[test]
fn telex_aw_rule_from_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/vi-v1");
    let scheme = root.join("schemes/vi_telex.yaml");
    if !scheme.exists() {
        return;
    }
    let bin = compile_scheme_yaml(&scheme, &root).expect("compile");
    let desc = SchemeDesc::from_bytes(&bin).unwrap();
    assert_eq!(desc.apply_rule_chain("aw"), "ă");
}

#[test]
fn pinyin_table_syllable_validation() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
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
    let mut factory = EngineFactory::new();
    factory.register(&spec).expect("register");
    factory
        .create(&built.manifest.id, "pinyin_full")
        .expect("scheme");

    let scheme_bin = install_path.join("scheme/pinyin_full.bin");
    let bytes = std::fs::read(scheme_bin).unwrap();
    let desc = SchemeDesc::from_bytes(&bytes).unwrap();
    assert!(yc_engine::is_valid_prefix("n", &desc.syllables));
    assert!(yc_engine::is_valid_prefix("ni", &desc.syllables));
    assert!(yc_engine::is_valid_prefix("nihao", &desc.syllables));
}

#[test]
fn zh_pack_nihao_candidates() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh2.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh2_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);
    engine.reset(editor);
    let mut last_step = None;
    for ch in "nihao".chars() {
        last_step = Some(
            engine
                .feed(editor, ch as u32, &Default::default())
                .unwrap(),
        );
    }
    let step = last_step.unwrap();
    assert!(step.candidates.iter().any(|c| c.text == "你好"));
}

#[test]
fn zh_pack_ta_candidates() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh_ta.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_ta_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);
    engine.reset(editor);
    let mut last_step = None;
    for ch in "ta".chars() {
        last_step = Some(
            engine
                .feed(editor, ch as u32, &Default::default())
                .unwrap(),
        );
    }
    let step = last_step.unwrap();
    assert!(
        step.candidates.iter().any(|c| c.text == "他"),
        "ta should include 他, got: {:?}",
        step.candidates.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn zh_pack_yi_paging_selects_beyond_first_page() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh_yi_page.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_yi_page_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);
    engine.reset(editor);
    for ch in "yi".chars() {
        engine
            .feed(editor, ch as u32, &Default::default())
            .unwrap();
    }
    assert!(
        engine.cand_total() > 9,
        "yi pool should exceed one page, got {}",
        engine.cand_total()
    );
    let first_page0 = engine.current_paged_step().candidates[0].text.clone();
    engine.page_next(editor).unwrap();
    assert_eq!(engine.cand_page(), 1);
    let page1 = engine.current_paged_step().candidates;
    assert!(!page1.is_empty());
    assert_ne!(page1[0].text, first_page0);
    let pick = page1[0].text.clone();
    let step = engine.select(editor, 0).unwrap();
    assert!(
        step.commands.iter().any(|c| matches!(
            c,
            yc_types::UiCommand::Commit { text } if text == &pick
        )),
        "select page-local 0 should commit {pick}"
    );
}

#[test]
fn zh_pack_polyphone_hang_xing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh_poly.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_poly_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);

    engine.reset(editor);
    for ch in "hang".chars() {
        engine
            .feed(editor, ch as u32, &Default::default())
            .unwrap();
    }
    assert!(
        engine
            .current_paged_step()
            .candidates
            .iter()
            .any(|c| c.text == "行")
            || {
                let mut found = false;
                let pages = ((engine.cand_total() + 8) / 9).max(1);
                for _ in 0..pages {
                    if engine
                        .current_paged_step()
                        .candidates
                        .iter()
                        .any(|c| c.text == "行")
                    {
                        found = true;
                        break;
                    }
                    let _ = engine.page_next(editor);
                }
                found
            },
        "hang should include 行"
    );

    engine.reset(editor);
    for ch in "xing".chars() {
        engine
            .feed(editor, ch as u32, &Default::default())
            .unwrap();
    }
    let mut found_xing = false;
    let pages = ((engine.cand_total() + 8) / 9).max(1);
    for _ in 0..pages {
        if engine
            .current_paged_step()
            .candidates
            .iter()
            .any(|c| c.text == "行")
        {
            found_xing = true;
            break;
        }
        let _ = engine.page_next(editor);
    }
    assert!(found_xing, "xing should include 行");
}

#[test]
fn zh_pack_jianpin_nh_nihao() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh_jp.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_jp_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    assert!(yc_engine::is_valid_pinyin_input("nh", &desc.syllables));
    assert!(!yc_engine::is_valid_prefix("nh", &desc.syllables));

    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);
    engine.reset(editor);
    let mut last = None;
    for ch in "nh".chars() {
        last = Some(
            engine
                .feed(editor, ch as u32, &Default::default())
                .expect("jianpin feed"),
        );
    }
    let step = last.unwrap();
    assert!(
        step.candidates.iter().any(|c| c.text == "你好"),
        "nh should yield 你好, got {:?}",
        step.candidates.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
}

#[test]
fn ascii_mode_accepts_http_and_commits_raw() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/langpacks/zh-pack-v1");
    if !root.exists() {
        return;
    }
    let pack_path = std::env::temp_dir().join("yc_engine_zh_ascii.imepack");
    let built = build_langpack_dir(&root, &pack_path).expect("build");
    let data = std::env::temp_dir().join("yc_engine_zh_ascii_install");
    let _ = std::fs::remove_dir_all(&data);
    yc_pack::install_pack_to_dir(&pack_path, &data).expect("install");
    let install_path = data.join(&built.manifest.id);
    let scheme_bin = std::fs::read(install_path.join("scheme/pinyin_full.bin")).unwrap();
    let desc = SchemeDesc::from_bytes(&scheme_bin).unwrap();
    let mut engine = DataDrivenEngine::new(built.manifest.id.clone(), desc);
    engine
        .load_lexicon(
            &built.manifest.id,
            &install_path
                .join(built.manifest.lexicon.effective_dat_path())
                .to_string_lossy(),
        )
        .unwrap();
    let editor = EditorId::from_raw(1);
    engine.reset(editor);
    let mut mode = yc_types::InputMode::default();
    mode.ascii_mode = true;
    for ch in "http".chars() {
        let step = engine.feed(editor, ch as u32, &mode).expect("ascii feed");
        assert!(step.candidates.is_empty());
        assert_eq!(step.composing.text.chars().last(), Some(ch));
    }
    let step = engine.feed(editor, b' ' as u32, &mode).unwrap();
    assert!(
        matches!(
            step.commands.first(),
            Some(yc_types::UiCommand::Commit { text }) if text == "http"
        ),
        "space should commit raw http"
    );
}
