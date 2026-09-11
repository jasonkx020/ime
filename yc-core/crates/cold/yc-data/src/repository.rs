use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use yc_types::{EngineError, HotResult};

#[derive(Debug, Default)]
pub struct Repository {
    kv: HashMap<String, Vec<u8>>,
    data_dir: PathBuf,
}

impl Repository {
    pub fn new(data_dir: PathBuf) -> Self {
        if let Err(e) = fs::create_dir_all(&data_dir) {
            eprintln!("repo mkdir: {e}");
        }
        Self {
            kv: HashMap::new(),
            data_dir,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn langpacks_dir(&self) -> PathBuf {
        self.data_dir.join("langpacks")
    }

    pub fn set_config(&mut self, key: &str, value: &[u8]) -> HotResult<()> {
        self.kv.insert(key.to_string(), value.to_vec());
        let path = self.data_dir.join(format!("{key}.cfg"));
        fs::write(path, value).map_err(|_| EngineError::Internal)?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Option<Vec<u8>> {
        self.kv.get(key).cloned().or_else(|| {
            let path = self.data_dir.join(format!("{key}.cfg"));
            fs::read(path).ok()
        })
    }
}
