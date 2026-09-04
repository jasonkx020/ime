//! LangPack OTA plugin host (M3.5).

mod catalog;
mod host;
mod layout_runtime;
mod registry;
mod slot;

pub use catalog::{CatalogEntry, RemoteCatalog};
pub use host::PluginHost;
pub use layout_runtime::{layout_bin_path, LayoutRuntime};
pub use registry::{hash_pack_id, LangPackLoader, LangPackRegistry, validate_slot};
pub use slot::LangPackSlot;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use yc_pack::build_langpack_dir;

    fn fixture_src() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/vi-v1")
    }

    #[test]
    fn install_returns_unsupported_on_missing() {
        let mut host = PluginHost::new(PathBuf::from("."));
        assert!(host.install_lang_pack("/nonexistent.pack").is_err());
    }

    #[test]
    fn install_enable_fixture() {
        let src = fixture_src();
        if !src.exists() {
            return;
        }
        let out = std::env::temp_dir().join("yc_test_vi.imepack");
        let built = build_langpack_dir(&src, &out).expect("build");
        let data = std::env::temp_dir().join("yc_plugin_test");
        let _ = std::fs::remove_dir_all(&data);
        let mut host = PluginHost::new(data);
        let m = host.install_lang_pack(out.to_str().unwrap()).expect("install");
        assert_eq!(m.id, built.manifest.id);
        host.enable(&m.id).unwrap();
        assert!(host.is_enabled(&m.id));
        assert!(host.lexicon_path(&m.id).unwrap().exists());
    }
}
