use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::ZipArchive;

use crate::manifest::{manifest_to_bytes, PackToml, LangPackManifest};
use crate::skin::{skin_to_bytes, SkinManifest, SkinToml};
use crate::verify::sha256_bytes;
use crate::{MANIFEST_FB, SIGNATURE_FILE};

pub struct PackBuildOutput {
    pub path: PathBuf,
    pub manifest: LangPackManifest,
    pub signature: String,
}

pub fn build_langpack_dir(src: &Path, out: &Path) -> std::io::Result<PackBuildOutput> {
    let pack_toml = src.join("pack.toml");
    let text = fs::read_to_string(&pack_toml)?;
    let pack: PackToml = toml::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut manifest = pack.to_manifest();

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut lexicon_src = src.join(&pack.lexicon.file);
    if !lexicon_src.exists() {
        for fallback in ["lexicon/zh_words.core.tsv", "lexicon/zh_words.sample.tsv"] {
            let candidate = src.join(fallback);
            if candidate.exists() {
                lexicon_src = candidate;
                break;
            }
        }
    }
    if lexicon_src.exists() {
        let sample = src.join("lexicon/zh_words.sample.tsv");
        let dat = if sample.is_file()
            && lexicon_src.extension().and_then(|s| s.to_str()) == Some("tsv")
        {
            yc_lexicon::compile_merged_tsv(&[&lexicon_src, &sample]).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?
        } else {
            yc_lexicon::compile_tsv_to_dat(&lexicon_src).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e)
            })?
        };
        let dat_rel = manifest.lexicon.effective_dat_path();
        zip.start_file(&dat_rel, opts)?;
        zip.write_all(&dat)?;
    }

    let mut layout_ids = Vec::new();
    if let Some(layouts) = src.join("layouts").read_dir().ok() {
        for entry in layouts.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layout");
                layout_ids.push(stem.to_string());
                let bin_name = format!("layouts/{stem}.bin");
                let data = yc_layout::compile_layout_yaml(&path)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                zip.start_file(&bin_name, opts)?;
                zip.write_all(&data)?;
            }
        }
    }
    manifest.layout_ids = layout_ids;

    for scheme in &pack.schemes {
        if scheme.file.is_empty() {
            continue;
        }
        let yaml_path = src.join(&scheme.file);
        if !yaml_path.is_file() {
            continue;
        }
        let bin_name = format!("scheme/{}.bin", scheme.id);
        let data = yc_scheme::compile_scheme_yaml(&yaml_path, src)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        zip.start_file(&bin_name, opts)?;
        zip.write_all(&data)?;
    }

    if let Some(strings_rel) = manifest.strings_path.clone() {
        let p = src.join(&strings_rel);
        if p.exists() {
            zip.start_file(&strings_rel, opts)?;
            zip.write_all(&fs::read(p)?)?;
        }
    }

    let manifest_bytes = manifest_to_bytes(&manifest);
    zip.start_file(MANIFEST_FB, opts)?;
    zip.write_all(&manifest_bytes)?;

    zip.finish()?;

    let sig = sha256_bytes(&fs::read(out)?);
    let sig_path = out.with_extension("imepack.sig");
    fs::write(&sig_path, &sig)?;

    Ok(PackBuildOutput {
        path: out.to_path_buf(),
        manifest,
        signature: sig,
    })
}

pub fn build_skin_dir(src: &Path, out: &Path) -> std::io::Result<SkinManifest> {
    let skin_toml = src.join("skin.toml");
    let text = fs::read_to_string(&skin_toml)?;
    let skin: SkinToml = toml::from_str(&text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let manifest = skin.to_manifest();

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let bytes = skin_to_bytes(&manifest);
    zip.start_file(MANIFEST_FB, opts)?;
    zip.write_all(&bytes)?;
    zip.finish()?;

    Ok(manifest)
}

pub fn extract_manifest_from_pack(pack_path: &Path) -> std::io::Result<LangPackManifest> {
    let file = File::open(pack_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut manifest_file = archive.by_name(MANIFEST_FB)?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    crate::manifest::manifest_from_bytes(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn extract_skin_from_pack(pack_path: &Path) -> std::io::Result<SkinManifest> {
    let file = File::open(pack_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut manifest_file = archive.by_name(MANIFEST_FB)?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    crate::skin::skin_from_bytes(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn install_pack_to_dir(pack_path: &Path, dest_root: &Path) -> std::io::Result<LangPackManifest> {
    let manifest = extract_manifest_from_pack(pack_path)?;
    let dest = dest_root.join(&manifest.id);
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    fs::create_dir_all(&dest)?;

    let file = File::open(pack_path)?;
    let mut archive = ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = dest.join(file.mangled_name());
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(manifest)
}
