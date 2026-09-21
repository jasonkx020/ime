//! Per-device user word learning (freq table), keyed by lang + query_key.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use yc_types::{Candidate, CandidateSource, MAX_CANDIDATE_POOL};

#[derive(Debug, Default, Clone)]
pub struct UserWordStore {
    /// key = "lang\tquery_key\tword" -> freq
    freqs: HashMap<String, u32>,
    path: Option<PathBuf>,
    dirty: bool,
}

static FLUSH_SCHEDULED: AtomicBool = AtomicBool::new(false);

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
                if line.is_empty()
                    || line.starts_with("pinyin")
                    || line.starts_with("lang\t")
                    || line.starts_with("query_key")
                {
                    continue;
                }
                let parts: Vec<&str> = line.split('\t').collect();
                // New: lang \t query_key \t word \t freq
                // Legacy: query_key \t word \t freq  (lang defaults empty → treated as "")
                if parts.len() >= 4 {
                    let lang = parts[0].trim().to_ascii_lowercase();
                    let qk = parts[1].trim().to_ascii_lowercase();
                    let word = parts[2].trim();
                    let freq: u32 = parts[3].parse().unwrap_or(1);
                    if !qk.is_empty() && !word.is_empty() {
                        store.freqs.insert(entry_key(&lang, &qk, word), freq);
                    }
                } else if parts.len() >= 3 {
                    let qk = parts[0].trim().to_ascii_lowercase();
                    let word = parts[1].trim();
                    let freq: u32 = parts[2].parse().unwrap_or(1);
                    if !qk.is_empty() && !word.is_empty() {
                        store.freqs.insert(entry_key("", &qk, word), freq);
                    }
                }
            }
        }
        Arc::new(Mutex::new(store))
    }

    pub fn touch(&mut self, query_key: &str, word: &str) {
        self.touch_lang("", query_key, word);
    }

    pub fn touch_lang(&mut self, lang: &str, query_key: &str, word: &str) {
        let lang = lang.trim().to_ascii_lowercase();
        let query_key = query_key.trim().to_ascii_lowercase();
        let word = word.trim();
        if query_key.is_empty() || word.is_empty() {
            return;
        }
        let key = entry_key(&lang, &query_key, word);
        let e = self.freqs.entry(key).or_insert(0);
        *e = e.saturating_add(1).max(1);
        self.dirty = true;
    }

    /// Apply a personalization boost as absolute-ish freq (at least `freq`).
    pub fn apply_boost(&mut self, lang: &str, query_key: &str, word: &str, freq: u32) {
        let lang = lang.trim().to_ascii_lowercase();
        let query_key = query_key.trim().to_ascii_lowercase();
        let word = word.trim();
        if query_key.is_empty() || word.is_empty() || freq == 0 {
            return;
        }
        let key = entry_key(&lang, &query_key, word);
        let e = self.freqs.entry(key).or_insert(0);
        *e = (*e).max(freq);
        self.dirty = true;
    }

    /// Debounced background flush of a shared store (hot path never waits on disk).
    pub fn schedule_flush_shared(store: Arc<Mutex<Self>>) {
        if !FLUSH_SCHEDULED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return;
        }
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(400));
            FLUSH_SCHEDULED.store(false, Ordering::SeqCst);
            let mut guard = store.lock();
            if !guard.dirty {
                return;
            }
            guard.dirty = false;
            let _ = guard.flush_now();
        });
    }

    pub fn flush(&self) -> Result<(), String> {
        self.flush_now()
    }

    fn flush_now(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut lines = vec!["lang\tquery_key\tword\tfreq".to_string()];
        let mut entries: Vec<_> = self.freqs.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (k, freq) in entries {
            let parts: Vec<&str> = k.splitn(3, '\t').collect();
            if parts.len() != 3 {
                continue;
            }
            lines.push(format!("{}\t{}\t{}\t{freq}", parts[0], parts[1], parts[2]));
        }
        let tmp = path.with_extension("tsv.tmp");
        fs::write(&tmp, lines.join("\n")).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn freq(&self, query_key: &str, word: &str) -> u32 {
        self.freq_lang("", query_key, word)
    }

    pub fn freq_lang(&self, lang: &str, query_key: &str, word: &str) -> u32 {
        self.freqs
            .get(&entry_key(
                &lang.trim().to_ascii_lowercase(),
                &query_key.trim().to_ascii_lowercase(),
                word.trim(),
            ))
            .copied()
            .unwrap_or(0)
    }

    pub fn boost_for_prefix(&self, prefix: &str) -> Vec<(String, String, u32)> {
        self.boost_for_prefix_lang("", prefix)
    }

    /// Returns (query_key, word, freq) for entries matching lang + prefix.
    pub fn boost_for_prefix_lang(&self, lang: &str, prefix: &str) -> Vec<(String, String, u32)> {
        let lang = lang.trim().to_ascii_lowercase();
        let prefix = prefix.trim().to_ascii_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (k, &freq) in &self.freqs {
            let Some((entry_lang, rest)) = k.split_once('\t') else {
                continue;
            };
            let Some((qk, word)) = rest.split_once('\t') else {
                continue;
            };
            if entry_lang != lang {
                continue;
            }
            if qk.starts_with(&prefix) || prefix.starts_with(qk) {
                out.push((qk.to_string(), word.to_string(), freq));
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
        out
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }
}

fn entry_key(lang: &str, query_key: &str, word: &str) -> String {
    format!("{lang}\t{query_key}\t{word}")
}

/// Score bump so a single user selection outranks default lexicon top (~1.0).
const USER_EXACT_BASE: f32 = 2.0;
const USER_FREQ_WEIGHT: f32 = 0.15;

/// Merge lexicon candidates with user-word boosts; reassign ids 0..pool.
pub fn merge_user_boosts(
    prefix: &str,
    candidates: Vec<Candidate>,
    store: &UserWordStore,
) -> Vec<Candidate> {
    merge_user_boosts_lang("", prefix, candidates, store)
}

pub fn merge_user_boosts_lang(
    lang: &str,
    prefix: &str,
    mut candidates: Vec<Candidate>,
    store: &UserWordStore,
) -> Vec<Candidate> {
    let prefix = prefix.trim().to_ascii_lowercase();
    for c in &mut candidates {
        let f = store.freq_lang(lang, &prefix, &c.text);
        if f > 0 {
            c.score += USER_EXACT_BASE + f as f32 * USER_FREQ_WEIGHT;
            if c.source == CandidateSource::Lexicon {
                c.source = CandidateSource::User;
            }
        }
    }
    for (qk, word, freq) in store.boost_for_prefix_lang(lang, &prefix) {
        if candidates.iter().any(|c| c.text == word) {
            continue;
        }
        if !(qk.starts_with(&prefix) || prefix == qk) {
            continue;
        }
        let exact = qk == prefix;
        let score = if exact {
            USER_EXACT_BASE + freq as f32 * USER_FREQ_WEIGHT
        } else {
            0.9 + freq as f32 * USER_FREQ_WEIGHT
        };
        candidates.push(Candidate {
            id: 0,
            text: word,
            source: CandidateSource::User,
            score,
            code_len: prefix.len() as u32,
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
        .take(MAX_CANDIDATE_POOL)
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
        s.touch_lang("zh", "ta", "他");
        s.touch_lang("zh", "ta", "他");
        assert_eq!(s.freq_lang("zh", "ta", "他"), 2);
        let cands = vec![
            Candidate {
                id: 0,
                text: "他们".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
                code_len: 0,
            },
            Candidate {
                id: 1,
                text: "他".into(),
                source: CandidateSource::Lexicon,
                score: 0.9,
                code_len: 0,
            },
        ];
        let out = merge_user_boosts_lang("zh", "ta", cands, &s);
        assert_eq!(out[0].text, "他");
        assert_eq!(out[0].source, CandidateSource::User);
    }

    #[test]
    fn lang_isolation_en_vs_zh() {
        let mut s = UserWordStore::new();
        s.touch_lang("en", "th", "thanks");
        s.touch_lang("zh", "th", "他");
        assert_eq!(s.freq_lang("en", "th", "thanks"), 1);
        assert_eq!(s.freq_lang("zh", "th", "他"), 1);
        assert_eq!(s.freq_lang("en", "th", "他"), 0);
        let en = merge_user_boosts_lang(
            "en",
            "th",
            vec![Candidate {
                id: 0,
                text: "the".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
                code_len: 0,
            }],
            &s,
        );
        assert!(en.iter().any(|c| c.text == "thanks"));
        assert!(!en.iter().any(|c| c.text == "他"));
    }

    #[test]
    fn one_touch_tao_promotes_tao_char() {
        let mut s = UserWordStore::new();
        s.touch("tao", "陶");
        assert_eq!(s.freq("tao", "陶"), 1);
        let cands = vec![
            Candidate {
                id: 0,
                text: "桃".into(),
                source: CandidateSource::Lexicon,
                score: 1.0,
                code_len: 0,
            },
            Candidate {
                id: 1,
                text: "逃".into(),
                source: CandidateSource::Lexicon,
                score: 0.999,
                code_len: 0,
            },
            Candidate {
                id: 2,
                text: "陶".into(),
                source: CandidateSource::Lexicon,
                score: 0.998,
                code_len: 0,
            },
            Candidate {
                id: 3,
                text: "涛".into(),
                source: CandidateSource::Lexicon,
                score: 0.997,
                code_len: 0,
            },
        ];
        let out = merge_user_boosts("tao", cands, &s);
        assert_eq!(out[0].text, "陶");
        assert_eq!(out[0].source, CandidateSource::User);
    }

    #[test]
    fn persist_roundtrip_keeps_habit() {
        let path = std::env::temp_dir().join("yc_user_words_roundtrip.tsv");
        let _ = fs::remove_file(&path);
        {
            let store = UserWordStore::open_or_create(&path);
            let mut g = store.lock();
            g.touch_lang("zh", "tao", "陶");
            g.flush().unwrap();
        }
        let reopened = UserWordStore::open_or_create(&path);
        assert_eq!(reopened.lock().freq_lang("zh", "tao", "陶"), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn english_prefix_hello_from_en_pack() {
        let sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/en-v1/lexicon/en_words.tsv");
        if !sample.exists() {
            return;
        }
        let dat = crate::compile_tsv_to_dat(&sample).unwrap();
        let lex = crate::DatLexicon::from_bytes(dat).unwrap();
        let cands = lex.lookup("hel");
        assert!(
            cands.iter().any(|c| c.text == "hello"),
            "expected hello in {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
    }
}
