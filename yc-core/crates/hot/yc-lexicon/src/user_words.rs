//! Per-device user word learning (freq table), keyed by lang + query_key.
//! Frequency decays with a half-life so Rank features stay aligned over time.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use yc_types::{Candidate, CandidateSource, MAX_CANDIDATE_POOL};

use crate::rank::{rank_candidates, user_boost_delta};

/// Half-life for touch frequency decay (days).
pub const USER_FREQ_HALF_LIFE_DAYS: f64 = 30.0;

#[derive(Debug, Clone, Copy)]
struct FreqEntry {
    freq: u32,
    /// Unix seconds of last touch / boost.
    last_touch: u64,
}

#[derive(Debug, Default, Clone)]
pub struct UserWordStore {
    /// key = "lang\tquery_key\tword" -> entry
    freqs: HashMap<String, FreqEntry>,
    path: Option<PathBuf>,
    dirty: bool,
}

static FLUSH_SCHEDULED: AtomicBool = AtomicBool::new(false);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Effective frequency after exponential decay.
pub fn decayed_freq(raw: u32, last_touch: u64, now: u64) -> u32 {
    if raw == 0 {
        return 0;
    }
    let age_days = now.saturating_sub(last_touch) as f64 / 86_400.0;
    let factor = 0.5_f64.powf(age_days / USER_FREQ_HALF_LIFE_DAYS);
    let v = (raw as f64 * factor).round();
    if v < 1.0 {
        // Keep a weak signal for recently-touched non-zero until very old.
        if age_days < USER_FREQ_HALF_LIFE_DAYS * 4.0 {
            1
        } else {
            0
        }
    } else {
        v.min(u32::MAX as f64) as u32
    }
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
        let now = now_secs();
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
                // New: lang \t query_key \t word \t freq [\t last_touch]
                // Legacy: query_key \t word \t freq  (lang defaults empty → treated as "")
                if parts.len() >= 4 {
                    let lang = parts[0].trim().to_ascii_lowercase();
                    let qk = parts[1].trim().to_ascii_lowercase();
                    let word = parts[2].trim();
                    let freq: u32 = parts[3].parse().unwrap_or(1);
                    let last = if parts.len() >= 5 {
                        parts[4].parse().unwrap_or(now)
                    } else {
                        now
                    };
                    if !qk.is_empty() && !word.is_empty() {
                        store.freqs.insert(
                            entry_key(&lang, &qk, word),
                            FreqEntry {
                                freq,
                                last_touch: last,
                            },
                        );
                    }
                } else if parts.len() >= 3 {
                    let qk = parts[0].trim().to_ascii_lowercase();
                    let word = parts[1].trim();
                    let freq: u32 = parts[2].parse().unwrap_or(1);
                    if !qk.is_empty() && !word.is_empty() {
                        store.freqs.insert(
                            entry_key("", &qk, word),
                            FreqEntry {
                                freq,
                                last_touch: now,
                            },
                        );
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
        let now = now_secs();
        let e = self.freqs.entry(key).or_insert(FreqEntry {
            freq: 0,
            last_touch: now,
        });
        // Decay then bump so old mass does not dominate forever.
        let effective = decayed_freq(e.freq, e.last_touch, now);
        e.freq = effective.saturating_add(1).max(1);
        e.last_touch = now;
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
        let now = now_secs();
        let e = self.freqs.entry(key).or_insert(FreqEntry {
            freq: 0,
            last_touch: now,
        });
        let effective = decayed_freq(e.freq, e.last_touch, now);
        e.freq = effective.max(freq);
        e.last_touch = now;
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
        let mut lines = vec!["lang\tquery_key\tword\tfreq\tlast_touch".to_string()];
        let mut entries: Vec<_> = self.freqs.iter().collect();
        entries.sort_by(|a, b| {
            b.1.freq
                .cmp(&a.1.freq)
                .then(a.0.cmp(b.0))
        });
        for (k, e) in entries {
            let parts: Vec<&str> = k.splitn(3, '\t').collect();
            if parts.len() != 3 {
                continue;
            }
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                parts[0], parts[1], parts[2], e.freq, e.last_touch
            ));
        }
        let tmp = path.with_extension("tsv.tmp");
        fs::write(&tmp, lines.join("\n")).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn freq(&self, query_key: &str, word: &str) -> u32 {
        self.freq_lang("", query_key, word)
    }

    /// Effective (decayed) frequency for Rank features.
    pub fn freq_lang(&self, lang: &str, query_key: &str, word: &str) -> u32 {
        let Some(e) = self.freqs.get(&entry_key(
            &lang.trim().to_ascii_lowercase(),
            &query_key.trim().to_ascii_lowercase(),
            word.trim(),
        )) else {
            return 0;
        };
        decayed_freq(e.freq, e.last_touch, now_secs())
    }

    /// Raw stored frequency (no decay); for tests / diagnostics.
    pub fn raw_freq_lang(&self, lang: &str, query_key: &str, word: &str) -> u32 {
        self.freqs
            .get(&entry_key(
                &lang.trim().to_ascii_lowercase(),
                &query_key.trim().to_ascii_lowercase(),
                word.trim(),
            ))
            .map(|e| e.freq)
            .unwrap_or(0)
    }

    pub fn boost_for_prefix(&self, prefix: &str) -> Vec<(String, String, u32)> {
        self.boost_for_prefix_lang("", prefix)
    }

    /// Returns (query_key, word, freq) for entries matching lang + query
    /// via string prefix or optional jianpin alignment.
    pub fn boost_for_prefix_lang(&self, lang: &str, prefix: &str) -> Vec<(String, String, u32)> {
        self.boost_for_query_lang(lang, prefix, None)
    }

    pub fn boost_for_query_lang(
        &self,
        lang: &str,
        query: &str,
        syllables: Option<&[String]>,
    ) -> Vec<(String, String, u32)> {
        let lang = lang.trim().to_ascii_lowercase();
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let now = now_secs();
        let mut out = Vec::new();
        for (k, e) in &self.freqs {
            let Some((entry_lang, rest)) = k.split_once('\t') else {
                continue;
            };
            let Some((qk, word)) = rest.split_once('\t') else {
                continue;
            };
            if entry_lang != lang {
                continue;
            }
            let prefix_hit = qk.starts_with(&query) || query.starts_with(qk);
            let jianpin_hit = syllables.is_some_and(|syls| {
                crate::pinyin_match::key_matches_jianpin(qk, &query, syls)
            });
            if prefix_hit || jianpin_hit {
                let freq = decayed_freq(e.freq, e.last_touch, now);
                if freq > 0 {
                    out.push((qk.to_string(), word.to_string(), freq));
                }
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
        out
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }
}

/// Personal typed_key → corrected_key learning (纠错词典).
#[derive(Debug, Default, Clone)]
pub struct UserCorrectionStore {
    /// lang\ttyped → (corrected_key, freq, last_touch)
    entries: HashMap<String, (String, u32, u64)>,
}

impl UserCorrectionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn touch(&mut self, lang: &str, typed: &str, corrected: &str) {
        let lang = lang.trim().to_ascii_lowercase();
        let typed = typed.trim().to_ascii_lowercase();
        let corrected = corrected.trim().to_ascii_lowercase();
        if typed.is_empty() || corrected.is_empty() || typed == corrected {
            return;
        }
        let key = format!("{lang}\t{typed}");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let e = self.entries.entry(key).or_insert((corrected.clone(), 0, now));
        if e.0 != corrected {
            e.0 = corrected;
            e.1 = 1;
        } else {
            e.1 = e.1.saturating_add(1);
        }
        e.2 = now;
    }

    pub fn lookup(&self, lang: &str, typed: &str) -> Option<String> {
        let key = format!(
            "{}\t{}",
            lang.trim().to_ascii_lowercase(),
            typed.trim().to_ascii_lowercase()
        );
        self.entries.get(&key).map(|(c, _, _)| c.clone())
    }
}

fn entry_key(lang: &str, query_key: &str, word: &str) -> String {
    format!("{lang}\t{query_key}\t{word}")
}

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
    candidates: Vec<Candidate>,
    store: &UserWordStore,
) -> Vec<Candidate> {
    merge_user_boosts_lang_syls(lang, prefix, candidates, store, None)
}

/// Like [`merge_user_boosts_lang`], also matching user keys via jianpin when
/// `syllables` is provided (`tlc` → `taoliuchang`).
pub fn merge_user_boosts_lang_syls(
    lang: &str,
    prefix: &str,
    mut candidates: Vec<Candidate>,
    store: &UserWordStore,
    syllables: Option<&[String]>,
) -> Vec<Candidate> {
    let prefix = prefix.trim().to_ascii_lowercase();
    for c in &mut candidates {
        let f = store.freq_lang(lang, &prefix, &c.text);
        if f > 0 {
            c.score += user_boost_delta(f);
            if c.source == CandidateSource::Lexicon {
                c.source = CandidateSource::User;
            }
        }
    }
    for (qk, word, freq) in store.boost_for_query_lang(lang, &prefix, syllables) {
        if candidates.iter().any(|c| c.text == word) {
            continue;
        }
        let prefix_hit = qk.starts_with(&prefix) || prefix == qk || prefix.starts_with(&qk);
        let jianpin_hit = syllables.is_some_and(|syls| {
            crate::pinyin_match::key_matches_jianpin(&qk, &prefix, syls)
        });
        if !prefix_hit && !jianpin_hit {
            continue;
        }
        let exact = qk == prefix;
        let score = if exact {
            user_boost_delta(freq)
        } else {
            0.9 + freq as f32 * crate::rank::USER_FREQ_WEIGHT
        };
        // Consume the learned key when composing starts with it (tao under taoliuchang → 3).
        let span = if prefix.starts_with(&qk) {
            qk.len() as u32
        } else {
            prefix.len() as u32
        };
        candidates.push(Candidate {
            id: 0,
            text: word,
            source: CandidateSource::User,
            score,
            code_len: span,
        });
    }
    rank_candidates(&mut candidates);
    candidates.truncate(MAX_CANDIDATE_POOL);
    candidates
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
    fn decay_halves_over_half_life() {
        let now = now_secs();
        let half = decayed_freq(8, now.saturating_sub(30 * 86_400), now);
        assert_eq!(half, 4, "8 after one half-life → 4, got {half}");
        let gone = decayed_freq(1, now.saturating_sub(200 * 86_400), now);
        assert_eq!(gone, 0);
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

    #[test]
    fn user_composed_word_hit_by_prefix_and_jianpin() {
        let mut store = UserWordStore::new();
        store.touch_lang("zh", "taoliuchang", "淘流畅");
        let syls = vec!["tao".into(), "liu".into(), "chang".into()];
        let empty = Vec::new();
        let full =
            merge_user_boosts_lang_syls("zh", "taoliuchang", empty.clone(), &store, Some(&syls));
        assert!(
            full.iter().any(|c| c.text == "淘流畅"),
            "full key: {:?}",
            full.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let prefix =
            merge_user_boosts_lang_syls("zh", "taoliu", empty.clone(), &store, Some(&syls));
        assert!(
            prefix.iter().any(|c| c.text == "淘流畅"),
            "prefix taoliu: {:?}",
            prefix.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let jp = merge_user_boosts_lang_syls("zh", "tlc", empty, &store, Some(&syls));
        assert!(
            jp.iter().any(|c| c.text == "淘流畅"),
            "jianpin tlc: {:?}",
            jp.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn user_word_short_key_code_len_under_long_composing() {
        let mut store = UserWordStore::new();
        store.touch_lang("zh", "tao", "套");
        let syls = vec!["tao".into(), "liu".into(), "chang".into()];
        let out = merge_user_boosts_lang_syls(
            "zh",
            "taoliuchang",
            Vec::new(),
            &store,
            Some(&syls),
        );
        let hit = out.iter().find(|c| c.text == "套").expect("套 from user");
        assert_eq!(
            hit.code_len, 3,
            "should consume tao only, got {}",
            hit.code_len
        );
    }
}
