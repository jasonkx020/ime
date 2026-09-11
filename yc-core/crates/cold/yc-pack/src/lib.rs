//! LangPack / SkinPack build, verify, and manifest types (M3/M3.5).

mod build;
mod manifest;
mod skin;
mod verify;

pub use build::{
    build_langpack_dir, build_skin_dir, extract_manifest_from_pack, extract_skin_from_pack,
    install_pack_to_dir, PackBuildOutput,
};
pub use manifest::{LangPackManifest, LexiconRef, PackScheme, PackToml};
pub use skin::{SkinColors, SkinManifest, SkinToml};
pub use verify::{sha256_file, verify_pack_signature};

pub const MANIFEST_FB: &str = "manifest.fb";
pub const SIGNATURE_FILE: &str = "signature";
