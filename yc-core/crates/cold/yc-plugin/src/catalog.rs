//! Remote Catalog client (yc-admin compatible JSON).

use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use yc_types::{EngineError, HotResult};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CatalogEntry {
    pub pack_id: String,
    pub lang: String,
    pub version: u32,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub min_host_version: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteCatalog {
    pub catalog_version: u32,
    pub fetched_at: i64,
    pub entries: Vec<CatalogEntry>,
}

impl RemoteCatalog {
    pub fn find(&self, pack_id: &str) -> Option<&CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| e.pack_id == pack_id)
            .max_by_key(|e| e.version)
    }
}

pub fn fetch_catalog_json(url: &str) -> HotResult<RemoteCatalog> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|_| EngineError::Internal)?;
    let mut cat: RemoteCatalog = resp.into_json().map_err(|_| EngineError::Internal)?;
    if cat.fetched_at == 0 {
        cat.fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
    }
    Ok(cat)
}

pub fn download_imepack(url: &str, dest: &Path, expect_sha256: &str) -> HotResult<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| EngineError::Internal)?;
    }
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|_| EngineError::Internal)?;
    let tmp = dest.with_extension("imepack.part");
    {
        let mut file = File::create(&tmp).map_err(|_| EngineError::Internal)?;
        let mut reader = resp.into_reader();
        copy(&mut reader, &mut file).map_err(|_| EngineError::Internal)?;
        file.flush().map_err(|_| EngineError::Internal)?;
    }
    let bytes = fs::read(&tmp).map_err(|_| EngineError::Internal)?;
    if !expect_sha256.is_empty() {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expect_sha256) {
            let _ = fs::remove_file(&tmp);
            return Err(EngineError::PackInvalid);
        }
    }
    fs::rename(&tmp, dest).map_err(|_| EngineError::Internal)?;
    Ok(())
}

pub fn cache_catalog_path(data_dir: &Path) -> PathBuf {
    data_dir.join("catalog").join("index.json")
}

pub fn save_catalog_cache(data_dir: &Path, cat: &RemoteCatalog) -> HotResult<()> {
    let path = cache_catalog_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| EngineError::Internal)?;
    }
    let bytes = serde_json::to_vec_pretty(cat).map_err(|_| EngineError::Internal)?;
    fs::write(path, bytes).map_err(|_| EngineError::Internal)?;
    Ok(())
}

pub fn load_catalog_cache(data_dir: &Path) -> Option<RemoteCatalog> {
    let path = cache_catalog_path(data_dir);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_catalog_json() {
        let raw = r#"{
          "catalog_version": 3,
          "fetched_at": 1,
          "entries": [{
            "pack_id": "zh-pack-v1",
            "lang": "zh",
            "version": 3,
            "url": "http://127.0.0.1:8080/cdn/langpacks/zh-pack-v1-v3.imepack",
            "sha256": "abc",
            "size_bytes": 10,
            "min_host_version": "0.1.0",
            "display_name": "中文"
          }]
        }"#;
        let cat: RemoteCatalog = serde_json::from_str(raw).unwrap();
        assert_eq!(cat.entries[0].pack_id, "zh-pack-v1");
        assert_eq!(cat.find("zh-pack-v1").unwrap().version, 3);
    }
}
