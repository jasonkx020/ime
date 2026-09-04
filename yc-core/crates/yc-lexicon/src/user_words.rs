//! Per-device user word learning (freq table).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use yc_types::{Candidate, CandidateSource, MAX_CANDIDATES};

#[derive(Debug, Default, Clone)]
pub struct UserWordStore {
    /// key = "pinyin\tword" -> freq
    freqs: HashMap<String, u32>,
    path: Option<PathBuf>,
}

impl UserWordStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new()))
    }

    pub fn open_or_create(path: impl AsRef<Path>) -> Arc<Mutex<Self>> {
        let path = path.as_ref().to_path_buf();
        let mut store = Self::new();
        store.path = Some(path.clone());
        if let Ok(text) = fs::read_to_string(&path) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("pinyin") {
                    continue;
                }
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 3 {
                    continue;
                }
                let pinyin = parts[0].trim().to_ascii_lowercase();
                let word = parts[1].trim();
                let freq: u32 = parts[2].parse().unwrap_or(1);
                if !pinyin.is_empty() && !word.is_empty() {
                    store.freqs.insert(entry_key(&pinyin, word), freq);
                }
            }
        }
        Arc::new(Mutex::new(store))
    }

    pub fn touch(&mut self, pinyin: &str, word: &str) {
        let pinyin = pinyin.trim().to_ascii_lowercase();
        let word = word.trim();
        if pinyin.is_empty() || word.is_empty() {
            return;
        }
        let key = entry_key(&pinyin, word);
        let e = self.freqs.entry(key).or_insert(0);
        *e = e.saturating_add(1).max(1);
        let _ = self.flush();
    }

    pub fn freq(&self, pinyin: &str, word: &str) -> u32 {
        self.freqs
            .get(&entry_key(&pinyin.to_ascii_lowercase(), word.trim()))
            .copied()
            .unwrap_or(0)
    }

    pub fn boost_for_prefix(&self, prefix: &str) -> Vec<(String, String, u32)> {
        let prefix = prefix.trim().to_ascii_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (k, &freq) in &self.freqs {
            let Some((py, word)) = k.split_once('\t') else {
                continue;
            };
            if py.starts_with(&prefix) || prefix.starts_with(py) {
                out.push((py.to_string(), word.to_string(), freq));
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
        out
    }

    pub fn flush(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut lines = vec!["pinyin\tword\tfreq".to_string()];
        let mut entries: Vec<_> = self.freqs.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (k, freq) in entries {
            let Some((py, word)) = k.split_once('\t') else {
                continue;
            };
            lines.push(format!("{py}\t{word}\t{freq}"));
        }
        fs::write(path, lines.join("\n")).map_err(|e| e.to_string())
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }
}

fn entry_key(pinyin: &str, word: &str) -> String {
    format!("{pinyin}\t{word}")
}

/// Merge lexicon candidates with user-word boosts; reassign ids 0..MAX_CANDIDATES.
pub fn merge_user_boosts(
    prefix: &str,
    mut candidates: Vec<Candidate>,
    store: &UserWordStore,
) -> Vec<Candidate> {
    let prefix = prefix.trim().to_ascii_lowercase();
    for c in &mut candidates {
        let f = store.freq(&prefix, &c.text);
        if f > 0 {
            c.score += f as f32 * 0.15;
            if c.source == CandidateSource::Lexicon {
                c.source = CandidateSource::User;
            }
        }
    }
    for (py, word, freq) in store.boost_for_prefix(&prefix) {
        if candidates.iter().any(|c| c.text == word) {
            continue;
        }
        if !(py.starts_with(&prefix) || prefix == py) {
            continue;
        }
        candidates.push(Candidate {
            id: 0,
            text: word,
            source: CandidateSource::User,
            score: 0.9 + freq as f32 * 0.15,
        });
    }
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.text.cmp(&b.text))
    });
    candidates
        .into_iter()
        .take(MAX_CANDIDATES as usize)
        .enumerate()
        .map(|(i, mut c)| {
            c.id = i as u32;
            c
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_boosts_ta() {
        let mut s = UserWordStore::new();
        s.touch("ta", "他");
        s.touch("ta", "他");
        assert_eq!(s.freq("ta", "他"), 2);
        let cands = vec![
            Candidate {
                id: 0,
                text: "他们".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
            },
            Candidate {
                id: 1,
                text: "他".into(),
                source: CandidateSource::Lexicon,
                score: 0.9,
            },
        ];
        let out = merge_user_boosts("ta", cands, &s);
        assert_eq!(out[0].text, "他");
        assert_eq!(out[0].source, CandidateSource::User);
    }
}
