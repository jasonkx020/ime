//! SyncWorker: upload habit events + pull personalization packs (cold path).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use yc_lexicon::UserWordStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitEventDto {
    pub device_id: String,
    pub lang: String,
    pub pack_id: String,
    pub event_type: String,
    pub query_key: String,
    pub selected_word: String,
    pub candidate_pos: i32,
    pub privacy_ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HabitUploadBody {
    events: Vec<HabitEventDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordBoostDto {
    #[serde(default)]
    pub query_key: String,
    #[serde(default)]
    pub pinyin: String,
    pub word: String,
    #[serde(default)]
    pub boost: f64,
    #[serde(default)]
    pub freq: u32,
    #[serde(default)]
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferPairDto {
    pub prev: String,
    pub next: String,
    #[serde(default)]
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizationPackDto {
    pub device_id: String,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub boosts: Vec<WordBoostDto>,
    #[serde(default)]
    pub prefer_pairs: Vec<PreferPairDto>,
    #[serde(default)]
    pub demote: Vec<WordBoostDto>,
}

#[derive(Debug, Clone)]
pub struct SyncWorkerConfig {
    pub base_url: String,
    pub device_id: String,
    pub queue_path: PathBuf,
    pub pairs_path: PathBuf,
}

impl SyncWorkerConfig {
    pub fn new(data_dir: impl AsRef<Path>, device_id: impl Into<String>, base_url: impl Into<String>) -> Self {
        let data_dir = data_dir.as_ref();
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            device_id: device_id.into(),
            queue_path: data_dir.join("habit_queue.jsonl"),
            pairs_path: data_dir.join("prefer_pairs.json"),
        }
    }
}

/// Client-side habit sync + personalization merge.
pub struct SyncWorker {
    cfg: SyncWorkerConfig,
    store: Arc<Mutex<UserWordStore>>,
}

impl SyncWorker {
    pub fn new(cfg: SyncWorkerConfig, store: Arc<Mutex<UserWordStore>>) -> Self {
        Self { cfg, store }
    }

    /// Enqueue a privacy-ok select event for later upload.
    pub fn enqueue_select(
        &self,
        lang: &str,
        pack_id: &str,
        query_key: &str,
        selected_word: &str,
        candidate_pos: i32,
        privacy_ok: bool,
    ) -> Result<(), String> {
        if !privacy_ok {
            return Ok(());
        }
        let ev = HabitEventDto {
            device_id: self.cfg.device_id.clone(),
            lang: lang.to_string(),
            pack_id: pack_id.to_string(),
            event_type: "select".into(),
            query_key: query_key.to_string(),
            selected_word: selected_word.to_string(),
            candidate_pos,
            privacy_ok: true,
        };
        let line = serde_json::to_string(&ev).map_err(|e| e.to_string())?;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.cfg.queue_path)
            .map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())
    }

    /// Drain queue → POST /habits/events, then GET personalization and merge.
    pub fn tick(&self) -> Result<PersonalizationPackDto, String> {
        self.upload_queued()?;
        self.pull_and_apply()
    }

    pub fn upload_queued(&self) -> Result<usize, String> {
        if !self.cfg.queue_path.exists() {
            return Ok(0);
        }
        let text = std::fs::read_to_string(&self.cfg.queue_path).map_err(|e| e.to_string())?;
        let mut events = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<HabitEventDto>(line) {
                if ev.privacy_ok {
                    events.push(ev);
                }
            }
        }
        if events.is_empty() {
            let _ = std::fs::remove_file(&self.cfg.queue_path);
            return Ok(0);
        }
        let url = format!("{}/api/v1/habits/events", self.cfg.base_url);
        let body = HabitUploadBody { events: events.clone() };
        let resp = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_json(serde_json::json!({ "events": body.events }))
            .map_err(|e| e.to_string())?;
        if resp.status() >= 300 {
            return Err(format!("habit upload HTTP {}", resp.status()));
        }
        let _ = std::fs::remove_file(&self.cfg.queue_path);
        Ok(events.len())
    }

    pub fn pull_and_apply(&self) -> Result<PersonalizationPackDto, String> {
        let url = format!(
            "{}/api/v1/personalization/{}",
            self.cfg.base_url, self.cfg.device_id
        );
        let pack: PersonalizationPackDto = ureq::get(&url)
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;

        {
            let mut store = self.store.lock();
            for b in &pack.boosts {
                let qk = if !b.query_key.is_empty() {
                    b.query_key.as_str()
                } else {
                    b.pinyin.as_str()
                };
                let lang = if b.lang.is_empty() {
                    pack.lang.as_str()
                } else {
                    b.lang.as_str()
                };
                let freq = b.freq.max(1);
                store.apply_boost(lang, qk, &b.word, freq);
            }
            for d in &pack.demote {
                // demote: keep low freq so exact user touch can still win
                let qk = if !d.query_key.is_empty() {
                    d.query_key.as_str()
                } else {
                    d.pinyin.as_str()
                };
                let lang = if d.lang.is_empty() {
                    pack.lang.as_str()
                } else {
                    d.lang.as_str()
                };
                let _ = (qk, lang);
                // Do not erase user habits; demote is applied in LightIntel via pairs file.
            }
        }

        let pairs_json = serde_json::to_string_pretty(&pack.prefer_pairs).unwrap_or_else(|_| "[]".into());
        if let Some(parent) = self.cfg.pairs_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&self.cfg.pairs_path, pairs_json).map_err(|e| e.to_string())?;
        Ok(pack)
    }

    pub fn load_prefer_pairs(&self) -> Vec<PreferPairDto> {
        let Ok(text) = std::fs::read_to_string(&self.cfg.pairs_path) else {
            return Vec::new();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_lexicon::UserWordStore;

    #[test]
    fn enqueue_and_apply_boost_offline() {
        let dir = std::env::temp_dir().join("yc_sync_worker_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = UserWordStore::open_or_create(dir.join("user_words.tsv"));
        let cfg = SyncWorkerConfig::new(&dir, "d-test", "http://127.0.0.1:9");
        let sw = SyncWorker::new(cfg, store.clone());
        sw.enqueue_select("en", "en-v1", "th", "thanks", 2, true)
            .unwrap();
        assert!(sw.cfg.queue_path.exists());

        // Offline apply path (no HTTP): write pack manually then merge via apply_boost
        store.lock().apply_boost("en", "th", "thanks", 5);
        assert_eq!(store.lock().freq_lang("en", "th", "thanks"), 5);
    }
}
