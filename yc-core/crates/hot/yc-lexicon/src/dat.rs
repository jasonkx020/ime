use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use parking_lot::Mutex;
use yc_types::{Candidate, CandidateSource, EngineError, HotResult, MAX_CANDIDATE_POOL};

use crate::user_words::{merge_user_boosts, UserWordStore};
use crate::LangLexiconHandle;

pub const LEXICON_MAGIC: &[u8; 4] = b"YCLX";
pub const LEXICON_VERSION: u32 = 2;

const HEADER_SIZE: usize = 16;
/// Max association suffix length (chars); longer matches are dropped as noise.
pub const ASSOC_MAX_SUFFIX_CHARS: usize = 2;
const NGRAM_ALPHA: f32 = 0.1;
const NGRAM_SCORE_CHARS: usize = 2;

/// Character unigram/bigram stats derived from lexicon words (freq-weighted).
#[derive(Debug, Clone, Default)]
pub struct CharNgramModel {
    uni: HashMap<char, u32>,
    bi: HashMap<(char, char), u32>,
    /// Approximate vocabulary size for additive smoothing.
    vocab: u32,
}

impl CharNgramModel {
    /// Log continuation score of `suffix` given `context` (uses context last char).
    /// Scores at most the first [`NGRAM_SCORE_CHARS`] of the suffix.
    pub fn continuation_score(&self, context: &str, suffix: &str) -> f32 {
        let Some(mut prev) = context.chars().last() else {
            return 0.0;
        };
        if suffix.is_empty() {
            return 0.0;
        }
        let v = self.vocab.max(1) as f32;
        let mut score = 0.0f32;
        for ch in suffix.chars().take(NGRAM_SCORE_CHARS) {
            let uni = *self.uni.get(&prev).unwrap_or(&0) as f32;
            let bi = *self.bi.get(&(prev, ch)).unwrap_or(&0) as f32;
            score += (bi + NGRAM_ALPHA).ln() - (uni + NGRAM_ALPHA * v).ln();
            prev = ch;
        }
        score
    }
}

/// Process-wide slot so LightIntel can read the active pack's n-gram after load.
#[derive(Debug, Clone, Default)]
pub struct SharedCharNgram {
    inner: Arc<Mutex<Option<Arc<CharNgramModel>>>>,
}

impl SharedCharNgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, model: Arc<CharNgramModel>) {
        *self.inner.lock() = Some(model);
    }

    pub fn clear(&self) {
        *self.inner.lock() = None;
    }

    pub fn model(&self) -> Option<Arc<CharNgramModel>> {
        self.inner.lock().clone()
    }

    pub fn continuation_score(&self, context: &str, suffix: &str) -> f32 {
        self.inner
            .lock()
            .as_ref()
            .map(|m| m.continuation_score(context, suffix))
            .unwrap_or(0.0)
    }

    pub fn has_model(&self) -> bool {
        self.inner.lock().is_some()
    }
}

/// Memory-mapped lexicon view (YCLX v2).
#[derive(Debug)]
pub struct DatLexicon {
    data: Arc<[u8]>,
    index_offsets: Vec<u32>,
    /// First character of word → (full word, freq) for post-select association.
    assoc_buckets: HashMap<char, Vec<(String, u32)>>,
    ngram: Arc<CharNgramModel>,
}

impl DatLexicon {
    pub fn open_mmap(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
        Self::from_arc(Arc::from(mmap.as_ref()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        Self::from_arc(Arc::from(bytes.into_boxed_slice()))
    }

    fn from_arc(data: Arc<[u8]>) -> Result<Self, String> {
        validate_header(&data)?;
        let index_offsets = build_index_offsets(&data)?;
        let mut lex = Self {
            data,
            index_offsets,
            assoc_buckets: HashMap::new(),
            ngram: Arc::new(CharNgramModel::default()),
        };
        let (buckets, ngram) = lex.build_assoc_and_ngram();
        lex.assoc_buckets = buckets;
        lex.ngram = Arc::new(ngram);
        Ok(lex)
    }

    pub fn ngram(&self) -> Arc<CharNgramModel> {
        Arc::clone(&self.ngram)
    }

    pub fn continuation_score(&self, context: &str, suffix: &str) -> f32 {
        self.ngram.continuation_score(context, suffix)
    }

    fn build_assoc_and_ngram(&self) -> (HashMap<char, Vec<(String, u32)>>, CharNgramModel) {
        let mut buckets: HashMap<char, Vec<(String, u32)>> = HashMap::new();
        let mut uni: HashMap<char, u32> = HashMap::new();
        let mut bi: HashMap<(char, char), u32> = HashMap::new();
        let key_count = self.key_count();
        for i in 0..key_count {
            let Some((_key, payload_off, payload_count)) = self.key_at_raw(i) else {
                continue;
            };
            for j in 0..payload_count {
                let Some((freq, word)) = self.read_payload_word(payload_off, j) else {
                    continue;
                };
                let Some(first) = word.chars().next() else {
                    continue;
                };
                let chars: Vec<char> = word.chars().collect();
                for &ch in &chars {
                    *uni.entry(ch).or_default() += freq;
                }
                for w in chars.windows(2) {
                    *bi.entry((w[0], w[1])).or_default() += freq;
                }
                buckets.entry(first).or_default().push((word, freq));
            }
        }
        for list in buckets.values_mut() {
            list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            list.dedup_by(|a, b| a.0 == b.0);
        }
        let vocab = uni.len() as u32;
        (
            buckets,
            CharNgramModel { uni, bi, vocab },
        )
    }

    /// Words that continue after `prefix` (Chinese/text prefix, not pinyin).
    /// Candidate text is the **suffix** only; `source = Hot`.
    ///
    /// Ranking bias (next-token style):
    /// 1. Prefer **single next characters**, scored by **summed** freqs of all
    ///    matching words that continue with that character (你们/你好 → 们/好).
    /// 2. Then short multi-char suffixes (len 2..ASSOC_MAX), by word freq.
    ///    This avoids surfacing idiom junk like 来我往 from 你来我往 ahead of 好.
    pub fn associate(&self, prefix: &str, limit: usize) -> Vec<Candidate> {
        let prefix = prefix.trim();
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        let Some(first) = prefix.chars().next() else {
            return Vec::new();
        };
        let Some(bucket) = self.assoc_buckets.get(&first) else {
            return Vec::new();
        };
        let prefix_chars = prefix.chars().count();
        let mut next_char_freq: HashMap<char, u32> = HashMap::new();
        let mut multi: Vec<(u32, String)> = Vec::new();
        let mut multi_seen = std::collections::HashSet::<String>::new();

        for (word, freq) in bucket {
            if !word.starts_with(prefix) {
                continue;
            }
            let word_chars = word.chars().count();
            if word_chars <= prefix_chars {
                continue;
            }
            let suffix_len = word_chars - prefix_chars;
            if suffix_len > ASSOC_MAX_SUFFIX_CHARS {
                continue;
            }
            let suffix: String = word.chars().skip(prefix_chars).collect();
            if suffix.is_empty() {
                continue;
            }
            let Some(next_ch) = suffix.chars().next() else {
                continue;
            };
            *next_char_freq.entry(next_ch).or_default() += *freq;
            if suffix_len >= 2 && multi_seen.insert(suffix.clone()) {
                multi.push((*freq, suffix));
            }
        }

        let mut singles: Vec<(u32, String)> = next_char_freq
            .into_iter()
            .map(|(ch, freq)| (freq, ch.to_string()))
            .collect();
        singles.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        multi.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut collected: Vec<(u32, String)> = Vec::with_capacity(singles.len() + multi.len());
        let mut seen = std::collections::HashSet::<String>::new();
        for (freq, text) in singles.into_iter().chain(multi.into_iter()) {
            if !seen.insert(text.clone()) {
                continue;
            }
            collected.push((freq, text));
            if collected.len() >= limit.min(MAX_CANDIDATE_POOL) {
                break;
            }
        }

        collected
            .into_iter()
            .enumerate()
            .map(|(i, (_freq, text))| Candidate {
                id: i as u32,
                text,
                source: CandidateSource::Hot,
                score: 1.0 - (i as f32 * 0.001),
            })
            .collect()
    }

    fn key_count(&self) -> u32 {
        u32::from_le_bytes(self.data[8..12].try_into().unwrap())
    }

    fn payload_offset(&self) -> u32 {
        u32::from_le_bytes(self.data[12..16].try_into().unwrap())
    }

    /// Zero-copy key slice + payload meta (avoids String alloc on every index probe).
    fn key_at_raw(&self, idx: u32) -> Option<(&[u8], u32, u32)> {
        let off = *self.index_offsets.get(idx as usize)? as usize;
        if off + 2 > self.data.len() {
            return None;
        }
        let key_len = u16::from_le_bytes(self.data[off..off + 2].try_into().unwrap()) as usize;
        let mut off = off + 2;
        if off + key_len + 8 > self.data.len() {
            return None;
        }
        let key = &self.data[off..off + key_len];
        off += key_len;
        let payload_off = u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap());
        let payload_count = u32::from_le_bytes(self.data[off + 4..off + 8].try_into().unwrap());
        Some((key, payload_off, payload_count))
    }

    fn key_at(&self, idx: u32) -> Option<(String, u32, u32)> {
        let (key, payload_off, payload_count) = self.key_at_raw(idx)?;
        Some((
            String::from_utf8_lossy(key).into_owned(),
            payload_off,
            payload_count,
        ))
    }

    fn read_payload_word(&self, base: u32, idx: u32) -> Option<(u32, String)> {
        let payload_base = self.payload_offset() as usize;
        let mut off = payload_base + base as usize;
        for _ in 0..idx {
            if off + 6 > self.data.len() {
                return None;
            }
            let word_len = u16::from_le_bytes(self.data[off + 4..off + 6].try_into().unwrap()) as usize;
            off += 6 + word_len;
        }
        if off + 6 > self.data.len() {
            return None;
        }
        let freq = u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap());
        let word_len = u16::from_le_bytes(self.data[off + 4..off + 6].try_into().unwrap()) as usize;
        off += 6;
        if off + word_len > self.data.len() {
            return None;
        }
        let word = String::from_utf8_lossy(&self.data[off..off + word_len]).into_owned();
        Some((freq, word))
    }

    pub fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        self.lookup_with_key_filter(prefix, |_| true)
    }

    pub fn lookup_pinyin(&self, composing: &str, syllables: &[String]) -> Vec<Candidate> {
        let composing = composing.trim().to_ascii_lowercase();
        if composing.is_empty() {
            return Vec::new();
        }

        // Full-pinyin path: DAT keys are sorted; prefix range scan.
        let mut collected: Vec<(u32, String, bool)> = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();
        let key_count = self.key_count();
        let start = lower_bound_key(self, &composing);
        for i in start..key_count {
            let Some((key_bytes, payload_off, payload_count)) = self.key_at_raw(i) else {
                break;
            };
            if !key_bytes.starts_with(composing.as_bytes()) {
                break;
            }
            let key = std::str::from_utf8(key_bytes).unwrap_or("");
            if !crate::pinyin_match::key_matches_composing(key, &composing, syllables) {
                continue;
            }
            for j in 0..payload_count {
                if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                    if seen.insert(word.clone()) {
                        collected.push((freq, word, false));
                    }
                }
            }
        }

        // Jianpin path: e.g. nh → nihao (key does not start with "nh").
        // Only scan the first-letter contiguous range (sorted index), and cap hits
        // so we never walk the whole lexicon on the UI thread.
        if crate::pinyin_match::needs_jianpin_scan(&composing, syllables) {
            const JIANPIN_MATCH_CAP: usize = 400;
            let first = composing.as_bytes()[0];
            let first_prefix = &composing[..1];
            let jp_start = lower_bound_key(self, first_prefix);
            let mut jp_hits = 0usize;
            for i in jp_start..key_count {
                let Some((key_bytes, payload_off, payload_count)) = self.key_at_raw(i) else {
                    break;
                };
                if key_bytes.first().copied() != Some(first) {
                    break;
                }
                if key_bytes.starts_with(composing.as_bytes()) {
                    continue; // already considered in prefix path
                }
                let key = std::str::from_utf8(key_bytes).unwrap_or("");
                if !crate::pinyin_match::key_matches_jianpin(key, &composing, syllables) {
                    continue;
                }
                for j in 0..payload_count {
                    if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                        if seen.insert(word.clone()) {
                            let demoted = freq.saturating_sub(freq / 10).max(1);
                            collected.push((demoted, word, true));
                            jp_hits += 1;
                            if jp_hits >= JIANPIN_MATCH_CAP {
                                break;
                            }
                        }
                    }
                }
                if jp_hits >= JIANPIN_MATCH_CAP {
                    break;
                }
            }
        }

        collected.sort_by(|a, b| {
            // Prefer non-jianpin-only, then freq
            a.2.cmp(&b.2).then(b.0.cmp(&a.0)).then(a.1.cmp(&b.1))
        });
        let mut cands: Vec<Candidate> = collected
            .into_iter()
            .take(MAX_CANDIDATE_POOL)
            .enumerate()
            .map(|(i, (_freq, text, jianpin_only))| {
                let base = 1.0 - (i as f32 * 0.001);
                Candidate {
                    id: i as u32,
                    text,
                    source: CandidateSource::Lexicon,
                    score: if jianpin_only { base - 0.05 } else { base },
                }
            })
            .collect();

        // 完整单音节（如 tao）：默认单字优先
        if crate::pinyin_match::is_complete_syllable(&composing, syllables) {
            prefer_single_char_candidates(&mut cands);
        }
        cands
    }

    fn lookup_with_key_filter(
        &self,
        prefix: &str,
        key_ok: impl Fn(&str) -> bool,
    ) -> Vec<Candidate> {
        let prefix = prefix.trim().to_ascii_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        let key_count = self.key_count();
        let start = lower_bound_key(self, &prefix);
        let mut collected: Vec<(u32, String)> = Vec::new();
        for i in start..key_count {
            let Some((key, payload_off, payload_count)) = self.key_at(i) else {
                break;
            };
            if !key.starts_with(&prefix) {
                break;
            }
            if !key_ok(&key) {
                continue;
            }
            for j in 0..payload_count {
                if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                    collected.push((freq, word));
                }
            }
        }
        collected.sort_by(|a, b| b.0.cmp(&a.0));
        collected
            .into_iter()
            .take(MAX_CANDIDATE_POOL)
            .enumerate()
            .map(|(i, (_freq, text))| Candidate {
                id: i as u32,
                text,
                source: CandidateSource::Lexicon,
                score: 1.0 - (i as f32 * 0.001),
            })
            .collect()
    }
}

/// Prefer single-character Han candidates, then original score; reassign ids/scores.
fn prefer_single_char_candidates(cands: &mut Vec<Candidate>) {
    cands.sort_by(|a, b| {
        let a1 = a.text.chars().count() == 1;
        let b1 = b.text.chars().count() == 1;
        b1.cmp(&a1)
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.text.cmp(&b.text))
    });
    for (i, c) in cands.iter_mut().enumerate() {
        c.id = i as u32;
        c.score = 1.0 - (i as f32 * 0.001);
    }
}

fn validate_header(data: &[u8]) -> Result<(), String> {
    if data.len() < HEADER_SIZE {
        return Err("dat too short".into());
    }
    if &data[0..4] != LEXICON_MAGIC {
        return Err("bad magic".into());
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != LEXICON_VERSION {
        return Err(format!("unsupported version {version}"));
    }
    Ok(())
}

fn build_index_offsets(data: &[u8]) -> Result<Vec<u32>, String> {
    validate_header(data)?;
    let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let payload_start = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let mut offsets = Vec::with_capacity(count);
    let mut off = HEADER_SIZE;
    for _ in 0..count {
        if off + 2 > data.len() || off + 2 > payload_start {
            return Err("truncated index".into());
        }
        offsets.push(off as u32);
        let key_len = u16::from_le_bytes(data[off..off + 2].try_into().unwrap()) as usize;
        off += 2;
        if off + key_len + 8 > data.len() || off + key_len + 8 > payload_start {
            return Err("truncated key entry".into());
        }
        off += key_len + 8;
    }
    if off != payload_start {
        return Err("index size mismatch".into());
    }
    Ok(offsets)
}

fn lower_bound_key(lex: &DatLexicon, prefix: &str) -> u32 {
    let mut lo = 0u32;
    let mut hi = lex.key_count();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let key = lex
            .key_at_raw(mid)
            .map(|(k, _, _)| k)
            .unwrap_or(b"");
        if key < prefix.as_bytes() {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

#[derive(Debug)]
struct LexiconEntry {
    key: String,
    word: String,
    freq: u32,
}

#[derive(Debug, Default)]
pub struct LexiconManager {
    packs: HashMap<String, DatLexicon>,
    handles: HashMap<String, LangLexiconHandle>,
    next_handle: u64,
    active_pack: Option<String>,
    user_words: Option<Arc<Mutex<UserWordStore>>>,
    shared_ngram: Option<SharedCharNgram>,
}

impl LexiconManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_user_words(&mut self, store: Arc<Mutex<UserWordStore>>) {
        self.user_words = Some(store);
    }

    pub fn user_words(&self) -> Option<Arc<Mutex<UserWordStore>>> {
        self.user_words.clone()
    }

    pub fn set_shared_ngram(&mut self, shared: SharedCharNgram) {
        self.shared_ngram = Some(shared);
        self.publish_active_ngram();
    }

    pub fn shared_ngram(&self) -> Option<SharedCharNgram> {
        self.shared_ngram.clone()
    }

    fn publish_active_ngram(&self) {
        let Some(shared) = &self.shared_ngram else {
            return;
        };
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                shared.publish(lex.ngram());
                return;
            }
        }
        // Fall back to any loaded pack so intel works before set_active.
        if let Some(lex) = self.packs.values().next() {
            shared.publish(lex.ngram());
        }
    }

    pub fn open_lang(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        let lex = DatLexicon::open_mmap(Path::new(path)).map_err(|_| EngineError::Internal)?;
        self.next_handle += 1;
        let handle = LangLexiconHandle(self.next_handle);
        self.handles.insert(pack_id.to_string(), handle);
        self.packs.insert(pack_id.to_string(), lex);
        if self.active_pack.is_none() {
            self.active_pack = Some(pack_id.to_string());
        }
        self.publish_active_ngram();
        Ok(())
    }

    pub fn close_lang(&mut self, pack_id: &str) -> HotResult<()> {
        self.packs.remove(pack_id);
        self.handles.remove(pack_id);
        if self.active_pack.as_deref() == Some(pack_id) {
            self.active_pack = None;
            if let Some(shared) = &self.shared_ngram {
                shared.clear();
            }
        }
        self.publish_active_ngram();
        Ok(())
    }

    pub fn set_active(&mut self, pack_id: &str) {
        self.active_pack = Some(pack_id.to_string());
        self.publish_active_ngram();
    }

    pub fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        let mut cands = if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                lex.lookup(prefix)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        if let Some(store) = &self.user_words {
            cands = merge_user_boosts(prefix, cands, &store.lock());
        }
        cands
    }

    pub fn lookup_pinyin(&self, composing: &str, syllables: &[String]) -> Vec<Candidate> {
        let mut cands = if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                lex.lookup_pinyin(composing, syllables)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        if let Some(store) = &self.user_words {
            cands = merge_user_boosts(composing, cands, &store.lock());
        }
        cands
    }

    pub fn touch_user_word(&self, pinyin: &str, word: &str) {
        if let Some(store) = &self.user_words {
            store.lock().touch(pinyin, word);
        }
    }

    /// Post-select association: suffixes of lexicon words starting with `prefix`.
    pub fn associate(&self, prefix: &str, limit: usize) -> Vec<Candidate> {
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                return lex.associate(prefix, limit);
            }
        }
        Vec::new()
    }

    pub fn continuation_score(&self, context: &str, suffix: &str) -> f32 {
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                return lex.continuation_score(context, suffix);
            }
        }
        0.0
    }

    pub fn ngram_model(&self) -> Option<Arc<CharNgramModel>> {
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                return Some(lex.ngram());
            }
        }
        None
    }
}

/// True when committed text should reset association context (sentence end).
pub fn clears_assoc_context(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '。' | '！' | '？' | '；' | '，' | '.' | '!' | '?' | ';' | ',' | '\n' | '\r'
        )
    })
}

pub fn normalize_romanized(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn compile_tsv_to_dat(path: &Path) -> Result<Vec<u8>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    compile_tsv_text_to_dat(&text)
}

pub fn compile_merged_tsv(paths: &[&Path]) -> Result<Vec<u8>, String> {
    let mut merged = String::from("word\tfreq\tpinyin\n");
    for path in paths {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || (i == 0 && line.starts_with("word")) {
                continue;
            }
            merged.push_str(line);
            merged.push('\n');
        }
    }
    compile_tsv_text_to_dat(&merged)
}

fn compile_tsv_text_to_dat(text: &str) -> Result<Vec<u8>, String> {
    let mut entries = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || (i == 0 && line.starts_with("word")) {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let word = parts[0].trim().to_string();
        let freq: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
        let romanized = parts.get(2).map(|s| normalize_romanized(s)).unwrap_or_default();
        let key = if romanized.is_empty() {
            normalize_romanized(&word)
        } else {
            romanized
        };
        if key.is_empty() || word.is_empty() {
            continue;
        }
        entries.push(LexiconEntry { key, word, freq });
    }
    compile_entries_to_dat(entries)
}

fn compile_entries_to_dat(entries: Vec<LexiconEntry>) -> Result<Vec<u8>, String> {
    let mut dedup: HashMap<(String, String), u32> = HashMap::new();
    for e in entries {
        dedup
            .entry((e.key, e.word))
            .and_modify(|f| *f = (*f).max(e.freq))
            .or_insert(e.freq);
    }
    let mut flat: Vec<LexiconEntry> = dedup
        .into_iter()
        .map(|((key, word), freq)| LexiconEntry { key, word, freq })
        .collect();
    flat.sort_by(|a, b| a.key.cmp(&b.key).then(b.freq.cmp(&a.freq)));

    let mut grouped: BTreeMap<String, Vec<(String, u32)>> = BTreeMap::new();
    for e in flat {
        grouped
            .entry(e.key)
            .or_default()
            .push((e.word, e.freq));
    }

    let mut index_bytes = Vec::new();
    let mut payload_bytes = Vec::new();
    for (key, group) in &grouped {
        let payload_off = payload_bytes.len() as u32;
        let payload_count = group.len() as u32;
        for (word, freq) in group {
            let wb = word.as_bytes();
            payload_bytes.extend_from_slice(&freq.to_le_bytes());
            payload_bytes.extend_from_slice(&(wb.len() as u16).to_le_bytes());
            payload_bytes.extend_from_slice(wb);
        }
        let kb = key.as_bytes();
        index_bytes.extend_from_slice(&(kb.len() as u16).to_le_bytes());
        index_bytes.extend_from_slice(kb);
        index_bytes.extend_from_slice(&payload_off.to_le_bytes());
        index_bytes.extend_from_slice(&payload_count.to_le_bytes());
    }

    let payload_offset = (HEADER_SIZE + index_bytes.len()) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(LEXICON_MAGIC);
    out.extend_from_slice(&LEXICON_VERSION.to_le_bytes());
    out.extend_from_slice(&(grouped.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload_offset.to_le_bytes());
    out.extend_from_slice(&index_bytes);
    out.extend_from_slice(&payload_bytes);
    Ok(out)
}

use std::fs;

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_tsv() -> std::path::PathBuf {
        use std::path::PathBuf;
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/vi-v1/lexicon/vi_words.tsv");
        if p.exists() {
            return p;
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv")
    }

    #[test]
    fn roundtrip_v2_dat() {
        let path = fixture_tsv();
        if !path.exists() {
            return;
        }
        let dat = compile_tsv_to_dat(&path).expect("compile");
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let cands = lex.lookup("xin");
        if path.to_string_lossy().contains("vi") {
            assert!(!cands.is_empty());
        }
    }

    #[test]
    fn prefix_lookup_nihao() {
        let sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv");
        if !sample.exists() {
            return;
        }
        let dat = compile_tsv_to_dat(&sample).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let cands = lex.lookup("nihao");
        assert!(cands.iter().any(|c| c.text == "你好"));
        let partial = lex.lookup("ni");
        assert!(!partial.is_empty());
    }

    #[test]
    fn associate_ni_yields_hao_suffix() {
        let sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv");
        if !sample.exists() {
            return;
        }
        let dat = compile_tsv_to_dat(&sample).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let cands = lex.associate("你", 50);
        assert!(
            cands.iter().any(|c| c.text == "好"),
            "expected suffix 好 from 你好, got {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        assert!(cands.iter().all(|c| c.source == CandidateSource::Hot));
        assert!(cands.iter().all(|c| c.text.chars().count() <= ASSOC_MAX_SUFFIX_CHARS));
        let next = lex.associate("你好", 50);
        assert!(
            next.iter().any(|c| c.text == "吗" || c.text == "啊"),
            "expected 吗/啊 after 你好"
        );
    }

    #[test]
    fn ngram_prefers_hao_after_ni() {
        let sample = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv");
        if !sample.exists() {
            return;
        }
        let dat = compile_tsv_to_dat(&sample).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let score_hao = lex.continuation_score("你", "好");
        let score_men = lex.continuation_score("你", "们");
        assert!(
            score_hao > score_men,
            "好 should score higher than 们 after 你: {score_hao} vs {score_men}"
        );
    }

    #[test]
    fn pinyin_lookup_ta_excludes_tai() {
        let tmp = std::env::temp_dir().join("yc_lexicon_ta.tsv");
        let tsv = "\
word\tfreq\tpinyin
他\t95000\tta
他们\t53001\ttamen
太阳\t52000\ttaiyang
台\t51000\ttai
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec![
            "ta".into(),
            "tai".into(),
            "taiyang".into(),
            "men".into(),
            "yang".into(),
        ];
        let plain = lex.lookup("ta");
        assert!(
            plain.iter().any(|c| c.text == "太阳"),
            "plain prefix wrongly includes tai* keys"
        );
        let pinyin = lex.lookup_pinyin("ta", &syls);
        assert!(pinyin.iter().any(|c| c.text == "他"));
        assert!(pinyin.iter().any(|c| c.text == "他们"));
        assert!(!pinyin.iter().any(|c| c.text == "太阳"));
        assert!(!pinyin.iter().any(|c| c.text == "台"));
        // 完整单音节 ta：单字「他」应排在词组「他们」之前
        let idx_ta = pinyin.iter().position(|c| c.text == "他").unwrap();
        let idx_tamen = pinyin.iter().position(|c| c.text == "他们").unwrap();
        assert!(idx_ta < idx_tamen, "single char should rank before phrase");
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn full_syllable_prefers_single_char_over_high_freq_phrase() {
        let tmp = std::env::temp_dir().join("yc_lexicon_tao_prefer.tsv");
        let tsv = "\
word\tfreq\tpinyin
桃\t8000\ttao
套\t7000\ttao
叨光\t90000\ttaoguang
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["tao".into(), "guang".into()];

        let plain = lex.lookup("tao");
        assert_eq!(
            plain[0].text, "叨光",
            "plain freq sort keeps high-freq phrase first"
        );

        let pinyin = lex.lookup_pinyin("tao", &syls);
        assert!(pinyin[0].text.chars().count() == 1);
        assert!(pinyin.iter().any(|c| c.text == "叨光"));
        let first_phrase = pinyin.iter().position(|c| c.text == "叨光").unwrap();
        assert!(first_phrase >= 2 || pinyin[..first_phrase].iter().all(|c| c.text.chars().count() == 1));

        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn user_habit_phrase_can_outrank_single_char() {
        let tmp = std::env::temp_dir().join("yc_lexicon_tao_user.tsv");
        let tsv = "\
word\tfreq\tpinyin
桃\t8000\ttao
叨光\t90000\ttaoguang
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["tao".into(), "guang".into()];
        let cands = lex.lookup_pinyin("tao", &syls);
        assert_eq!(cands[0].text, "桃");

        let mut store = UserWordStore::new();
        store.touch("tao", "叨光");
        store.touch("tao", "叨光");
        let ranked = merge_user_boosts("tao", cands, &store);
        assert_eq!(ranked[0].text, "叨光");

        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn bench_100k_compile_and_lookup() {
        let mut entries = Vec::new();
        for i in 0..100_000u32 {
            let key = format!("word{i}");
            entries.push(format!("词{i}\t{}\t{key}", 1000 + (i % 500)));
        }
        let tmp = std::env::temp_dir().join("yc_lexicon_bench.tsv");
        std::fs::write(&tmp, format!("word\tfreq\tpinyin\n{}", entries.join("\n"))).unwrap();
        let dat = compile_tsv_to_dat(&tmp).expect("compile 100k");
        let lex = DatLexicon::from_bytes(dat).unwrap();
        assert_eq!(lex.key_count(), 100_000);
        let start = std::time::Instant::now();
        let cands = lex.lookup("word99999");
        let elapsed = start.elapsed();
        assert!(!cands.is_empty());
        assert!(
            elapsed.as_millis() < 2000,
            "lookup too slow: {:?}",
            elapsed
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn dump_packaged_assoc_ni() {
        let path = std::env::temp_dir().join("yc_zh_words.dat");
        if !path.exists() {
            return;
        }
        let lex = DatLexicon::open_mmap(&path).unwrap();
        let cands = lex.associate("你", 40);
        let mut out = format!("n={}\n", cands.len());
        for c in &cands {
            out.push_str(&format!(
                "{}\ts={:.4}\n",
                c.text,
                lex.continuation_score("你", &c.text)
            ));
        }
        let dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../_assoc_ni.txt");
        std::fs::write(&dest, out).unwrap();
        assert!(
            cands.iter().take(5).any(|c| c.text == "好" || c.text == "们"),
            "expected 好/们 in top-5, got {:?}",
            cands.iter().take(8).map(|c| &c.text).collect::<Vec<_>>()
        );
        assert!(
            !cands.iter().any(|c| c.text.contains('我')),
            "idiom junk with 我 should be gone: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
    }
}
