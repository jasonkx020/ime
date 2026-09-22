use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use parking_lot::Mutex;
use yc_types::{Candidate, CandidateSource, EngineError, HotResult, MAX_CANDIDATE_POOL};

use crate::lookup_opts::{LookupCancel, LookupOpts};
use crate::preprocess::preprocess_pinyin;
use crate::rank::{as_correction_score, rank_candidates};
use crate::seg_hypotheses::{build_seg_hypotheses_ex, SegmentMode};
use crate::span_resolve::attach_spans;
use crate::user_words::{merge_user_boosts_lang, merge_user_boosts_lang_syls, UserWordStore};
use crate::correction::{correction_variants, CorrectionTables};
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
    /// Top followers of each character, highest bigram first (at most 16).
    next: HashMap<char, Vec<char>>,
    /// Most frequent characters, for association backoff.
    top_uni: Vec<char>,
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

    /// Next characters after `prev`. Bigram hits first; unigram fills up to 8 when sparse.
    pub fn top_next(&self, prev: char, limit: usize) -> Vec<char> {
        if limit == 0 {
            return Vec::new();
        }
        let mut out: Vec<char> = self
            .next
            .get(&prev)
            .map(|v| v.iter().copied().take(limit).collect())
            .unwrap_or_default();
        let fill = 8.min(limit);
        if out.len() < fill {
            for &ch in &self.top_uni {
                if out.contains(&ch) {
                    continue;
                }
                out.push(ch);
                if out.len() >= fill {
                    break;
                }
            }
        }
        out
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
                    let e = uni.entry(ch).or_default();
                    *e = (*e).saturating_add(freq);
                }
                for w in chars.windows(2) {
                    let e = bi.entry((w[0], w[1])).or_default();
                    *e = (*e).saturating_add(freq);
                }
                buckets.entry(first).or_default().push((word, freq));
            }
        }
        for list in buckets.values_mut() {
            list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            list.dedup_by(|a, b| a.0 == b.0);
        }
        let vocab = uni.len() as u32;
        let mut next_buckets: HashMap<char, Vec<(u32, char)>> = HashMap::new();
        for ((prev, next), freq) in &bi {
            next_buckets
                .entry(*prev)
                .or_default()
                .push((*freq, *next));
        }
        let next = next_buckets
            .into_iter()
            .map(|(prev, mut list)| {
                list.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                list.truncate(16);
                (prev, list.into_iter().map(|(_, ch)| ch).collect())
            })
            .collect();
        let mut uni_rank: Vec<(u32, char)> = uni.iter().map(|(ch, freq)| (*freq, *ch)).collect();
        uni_rank.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        uni_rank.truncate(16);
        let top_uni = uni_rank.into_iter().map(|(_, ch)| ch).collect();
        (
            buckets,
            CharNgramModel {
                uni,
                bi,
                vocab,
                next,
                top_uni,
            },
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
            let e = next_char_freq.entry(next_ch).or_default();
            *e = (*e).saturating_add(*freq);
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

        if collected.len() < limit.min(MAX_CANDIDATE_POOL) {
            if let Some(last) = prefix.chars().last() {
                for ch in self.ngram.top_next(last, 8) {
                    let text = ch.to_string();
                    if !seen.insert(text.clone()) {
                        continue;
                    }
                    collected.push((0, text));
                    if collected.len() >= limit.min(MAX_CANDIDATE_POOL) {
                        break;
                    }
                }
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
                code_len: 0,
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

    /// Exact DAT key → all (freq, word) payloads (empty if key missing).
    pub fn exact_key_words_pub(&self, key: &str) -> Vec<(u32, String)> {
        self.exact_key_words(key)
    }

    /// Exact DAT key → all (freq, word) payloads (empty if key missing).
    pub(crate) fn exact_key_words(&self, key: &str) -> Vec<(u32, String)> {
        if key.is_empty() {
            return Vec::new();
        }
        let idx = lower_bound_key(self, key);
        let Some((kb, payload_off, payload_count)) = self.key_at_raw(idx) else {
            return Vec::new();
        };
        if kb != key.as_bytes() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(payload_count as usize);
        for j in 0..payload_count {
            if let Some(pair) = self.read_payload_word(payload_off, j) {
                out.push(pair);
            }
        }
        out
    }

    /// Best words whose pinyin uses exactly `span_syls` syllables and matches `pattern`
    /// as full syllables or initials. Scans keys under `key_prefix` (a syllable or one initial).
    /// Keeps a freq-aware cap so a later high-frequency hit like 苹果 is not dropped
    /// just because it is not among the first alphabetical matches.
    pub(crate) fn jianpin_span_words(
        &self,
        key_prefix: &str,
        pattern: &str,
        syllables: &[String],
        span_syls: usize,
    ) -> Vec<(u32, String)> {
        const MATCH_CAP: usize = 32;
        const KEY_CAP: usize = 16384;
        if key_prefix.is_empty() || pattern.is_empty() || span_syls == 0 {
            return Vec::new();
        }
        let mut best: Vec<(u32, String)> = Vec::new();
        let start = lower_bound_key(self, key_prefix);
        let key_count = self.key_count();
        let mut seen_keys = 0usize;
        for i in start..key_count {
            seen_keys += 1;
            if seen_keys > KEY_CAP {
                break;
            }
            let Some((kb, payload_off, payload_count)) = self.key_at_raw(i) else {
                break;
            };
            if !kb.starts_with(key_prefix.as_bytes()) {
                break;
            }
            let key = std::str::from_utf8(kb).unwrap_or("");
            let key_syls = crate::pinyin_match::split_syllables(key, syllables);
            if key_syls.len() != span_syls {
                continue;
            }
            if !crate::pinyin_match::key_matches_jianpin(key, pattern, syllables) {
                continue;
            }
            for j in 0..payload_count {
                if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                    consider_span_hit(&mut best, freq, word, span_syls, MATCH_CAP);
                }
            }
        }
        best
    }

    pub fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        self.lookup_with_key_filter(prefix, |_| true)
    }

    pub fn lookup_pinyin(&self, composing: &str, syllables: &[String]) -> Vec<Candidate> {
        self.lookup_pinyin_opts(
            composing,
            syllables,
            None,
            0,
            LookupOpts::lean(),
        )
        .unwrap_or_default()
    }

    /// Cancelable lean/expanded pinyin lookup. Returns `None` if canceled mid-scan.
    pub fn lookup_pinyin_opts(
        &self,
        composing: &str,
        syllables: &[String],
        cancel: Option<&LookupCancel>,
        mine: u64,
        opts: LookupOpts,
    ) -> Option<Vec<Candidate>> {
        self.lookup_pinyin_opts_ex(composing, syllables, cancel, mine, opts, None)
    }

    /// Like [`lookup_pinyin_opts`] with explicit correction tables (tests / pack).
    pub fn lookup_pinyin_opts_ex(
        &self,
        composing_raw: &str,
        syllables: &[String],
        cancel: Option<&LookupCancel>,
        mine: u64,
        opts: LookupOpts,
        tables: Option<&CorrectionTables>,
    ) -> Option<Vec<Candidate>> {
        let pre = preprocess_pinyin(composing_raw, syllables);
        let composing = pre.key;
        if composing.is_empty() {
            return Some(Vec::new());
        }
        if cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }
        let tables_owned;
        let tables: &CorrectionTables = match tables {
            Some(t) => t,
            None => {
                tables_owned = CorrectionTables::builtin_fallback();
                &tables_owned
            }
        };

        // (freq, jianpin_only, class) class: 0 exact, 1 fuzzy, 2 typo, 3 edit1
        let mut best: std::collections::HashMap<String, (u32, bool, u8)> =
            std::collections::HashMap::new();

        let merge_hit = |best: &mut std::collections::HashMap<String, (u32, bool, u8)>,
                         freq: u32,
                         word: String,
                         jianpin_only: bool,
                         class: u8| {
            match best.get(&word) {
                Some(&(ef, ej, ec)) => {
                    let better = class < ec
                        || (class == ec && !jianpin_only && ej)
                        || (class == ec && jianpin_only == ej && freq > ef);
                    if better {
                        best.insert(word, (freq, jianpin_only, class));
                    }
                }
                None => {
                    best.insert(word, (freq, jianpin_only, class));
                }
            }
        };

        let exact = self.lookup_pinyin_one(
            &composing,
            syllables,
            opts.enable_jianpin,
            opts.skip_jianpin_if_exact_ge,
            opts.pool_limit,
            cancel,
            mine,
        )?;
        for (freq, word, jianpin_only) in exact {
            merge_hit(&mut best, freq, word, jianpin_only, 0);
        }
        if cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }

        let want_correct = opts.enable_fuzzy || opts.enable_typo || opts.enable_edit1;
        if want_correct && composing.len() >= opts.typo_min_len {
            let fuzzy_cap = if opts.enable_fuzzy {
                opts.fuzzy_variant_cap
            } else {
                0
            };
            let typo_cap = if opts.enable_typo
                && best.len() < opts.typo_max_hits
            {
                opts.typo_variant_cap
            } else {
                0
            };
            let edit_cap = if opts.enable_edit1
                && composing.len() >= 3
                && (best.is_empty() || best.len() < opts.typo_max_hits)
            {
                opts.edit1_variant_cap
            } else {
                0
            };

            for (variant, class) in correction_variants(
                &composing,
                tables,
                fuzzy_cap,
                typo_cap,
                edit_cap,
            ) {
                if cancel.is_some_and(|c| c.is_canceled(mine)) {
                    return None;
                }
                if best.len() >= opts.pool_limit.saturating_mul(2) {
                    break;
                }
                let hits = self.lookup_pinyin_one(
                    &variant,
                    syllables,
                    opts.enable_jianpin,
                    opts.skip_jianpin_if_exact_ge,
                    opts.pool_limit,
                    cancel,
                    mine,
                )?;
                for (freq, word, jianpin_only) in hits {
                    merge_hit(&mut best, freq, word, jianpin_only, class);
                }
            }
        }

        if cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }

        let mut collected: Vec<(u32, String, bool, u8)> = best
            .into_iter()
            .map(|(word, (freq, jianpin_only, class))| (freq, word, jianpin_only, class))
            .collect();
        collected.sort_by(|a, b| {
            a.3.cmp(&b.3)
                .then(a.2.cmp(&b.2))
                .then(b.0.cmp(&a.0))
                .then(a.1.cmp(&b.1))
        });

        let limit = opts.pool_limit.min(MAX_CANDIDATE_POOL);
        let has_exact_full = !self.exact_key_words(&composing).is_empty();

        let mode = opts.segment_mode;
        let hyps = build_seg_hypotheses_ex(&composing, syllables, mode, &pre.hard_breaks);

        let mut cands: Vec<Candidate> = Vec::new();
        if !has_exact_full {
            gen_structural(self, &composing, syllables, cancel, mine, mode, &hyps, &mut cands)?;
        }

        let noise_budget = if has_exact_full {
            limit
        } else {
            limit.max(16)
        };
        let noise: Vec<Candidate> = collected
            .into_iter()
            .take(noise_budget)
            .enumerate()
            .map(|(i, (_freq, text, jianpin_only, class))| {
                let base = if class == 0 && !jianpin_only {
                    crate::cand_tiers::score_l0(i)
                } else if class > 0 {
                    as_correction_score(
                        (crate::cand_tiers::SCORE_L3 - 0.02 - i as f32 * 0.001).max(0.70),
                    )
                } else if jianpin_only {
                    (crate::cand_tiers::SCORE_L3 - 0.02 - i as f32 * 0.001).max(0.72)
                } else {
                    crate::cand_tiers::score_l0(i)
                };
                Candidate {
                    id: i as u32,
                    text,
                    source: CandidateSource::Lexicon,
                    score: base,
                    code_len: composing.len() as u32,
                }
            })
            .collect();

        if has_exact_full {
            cands = noise;
            if crate::pinyin_match::is_complete_syllable(&composing, syllables) {
                prefer_single_char_candidates(&mut cands);
            }
            gen_structural(self, &composing, syllables, cancel, mine, mode, &hyps, &mut cands)?;
        } else {
            for n in noise {
                if let Some(existing) = cands.iter_mut().find(|c| c.text == n.text) {
                    if n.score > existing.score {
                        existing.score = n.score;
                    }
                    continue;
                }
                cands.push(n);
            }
        }
        attach_spans(self, &composing, syllables, &mut cands);
        rank_candidates(&mut cands);
        Some(cands)
    }

    /// One composing string → raw hits (freq, word, jianpin_only). No typo expansion.
    fn lookup_pinyin_one(
        &self,
        composing: &str,
        syllables: &[String],
        enable_jianpin: bool,
        skip_jianpin_if_exact_ge: usize,
        pool_limit: usize,
        cancel: Option<&LookupCancel>,
        mine: u64,
    ) -> Option<Vec<(u32, String, bool)>> {
        if composing.is_empty() {
            return Some(Vec::new());
        }

        // Soft caps: avoid short-prefix scans of 10k+ keys on the hot path.
        // Collect a bit more than pool_limit so freq-sort still has headroom.
        let collect_cap = pool_limit.saturating_mul(4).clamp(pool_limit.max(16), 256);
        let key_scan_cap = pool_limit.saturating_mul(32).clamp(256, 2048);

        let mut collected: Vec<(u32, String, bool)> = Vec::new();
        let mut seen = std::collections::HashSet::<String>::new();
        let key_count = self.key_count();
        let start = lower_bound_key(self, composing);
        let mut keys_seen = 0usize;
        for i in start..key_count {
            if i & 0x3f == 0 && cancel.is_some_and(|c| c.is_canceled(mine)) {
                return None;
            }
            let Some((key_bytes, payload_off, payload_count)) = self.key_at_raw(i) else {
                break;
            };
            if !key_bytes.starts_with(composing.as_bytes()) {
                break;
            }
            keys_seen += 1;
            if keys_seen > key_scan_cap {
                break;
            }
            let key = std::str::from_utf8(key_bytes).unwrap_or("");
            if !crate::pinyin_match::key_matches_composing(key, composing, syllables) {
                continue;
            }
            for j in 0..payload_count {
                if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                    if seen.insert(word.clone()) {
                        collected.push((freq, word, false));
                        if collected.len() >= collect_cap {
                            break;
                        }
                    }
                }
            }
            if collected.len() >= collect_cap {
                break;
            }
        }

        let exact_count = collected.iter().filter(|(_, _, jp)| !jp).count();
        if enable_jianpin
            && exact_count < skip_jianpin_if_exact_ge
            && crate::pinyin_match::needs_jianpin_scan(composing, syllables)
        {
            const JIANPIN_MATCH_CAP: usize = 200;
            // nihao sits ~1759 into the n* bucket on the full zh pack; h* is ~8k.
            let jp_key_scan_cap = pool_limit.saturating_mul(64).clamp(2048, 8192);
            let first = composing.as_bytes()[0];
            let first_prefix = &composing[..1];
            let jp_start = lower_bound_key(self, first_prefix);
            let mut jp_best: Vec<(u32, String)> = Vec::new();
            let mut jp_keys_seen = 0usize;
            for i in jp_start..key_count {
                if i & 0x3f == 0 && cancel.is_some_and(|c| c.is_canceled(mine)) {
                    return None;
                }
                let Some((key_bytes, payload_off, payload_count)) = self.key_at_raw(i) else {
                    break;
                };
                if key_bytes.first().copied() != Some(first) {
                    break;
                }
                jp_keys_seen += 1;
                if jp_keys_seen > jp_key_scan_cap {
                    break;
                }
                if key_bytes.starts_with(composing.as_bytes()) {
                    continue;
                }
                let key = std::str::from_utf8(key_bytes).unwrap_or("");
                if !crate::pinyin_match::key_matches_jianpin(key, composing, syllables) {
                    continue;
                }
                for j in 0..payload_count {
                    if let Some((freq, word)) = self.read_payload_word(payload_off, j) {
                        if seen.contains(&word) {
                            continue;
                        }
                        let demoted = freq.saturating_sub(freq / 10).max(1);
                        consider_jianpin_hit(&mut jp_best, demoted, word, JIANPIN_MATCH_CAP);
                    }
                }
            }
            for (freq, word) in jp_best {
                if seen.insert(word.clone()) {
                    collected.push((freq, word, true));
                }
            }
        }
        Some(collected)
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
                code_len: 0,
            })
            .collect()
    }
}

/// Gen: structural candidates from Seg hypotheses / lattice (shared Instant+Lean path).
fn gen_structural(
    lex: &DatLexicon,
    composing: &str,
    syllables: &[String],
    cancel: Option<&LookupCancel>,
    mine: u64,
    mode: SegmentMode,
    hyps: &crate::seg_hypotheses::SegHypotheses,
    cands: &mut Vec<Candidate>,
) -> Option<()> {
    match mode {
        SegmentMode::Off => Some(()),
        SegmentMode::FirstSyllable => {
            crate::cand_gen::gen_from_hyps(lex, composing, hyps, cands);
            Some(())
        }
        SegmentMode::Full | SegmentMode::FullFine => {
            crate::segment::merge_composed_ex(
                lex,
                composing,
                syllables,
                cancel,
                mine,
                cands,
                mode == SegmentMode::FullFine,
            )
        }
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

/// Keep the strongest span hits: exact char-count first, then higher freq.
fn consider_jianpin_hit(best: &mut Vec<(u32, String)>, freq: u32, word: String, cap: usize) {
    if let Some(existing) = best.iter_mut().find(|(_, w)| *w == word) {
        if freq > existing.0 {
            existing.0 = freq;
        }
        return;
    }
    let hit = (freq, word);
    if best.len() < cap {
        best.push(hit);
        return;
    }
    let worst = best
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(i, _)| i);
    if let Some(idx) = worst {
        if hit.0 > best[idx].0 || (hit.0 == best[idx].0 && hit.1 < best[idx].1) {
            best[idx] = hit;
        }
    }
}

fn consider_span_hit(
    best: &mut Vec<(u32, String)>,
    freq: u32,
    word: String,
    span: usize,
    cap: usize,
) {
    let hit = (freq, word);
    if best.len() < cap {
        best.push(hit);
        return;
    }
    let worst = best
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| span_hit_rank(a, span).cmp(&span_hit_rank(b, span)))
        .map(|(i, _)| i);
    if let Some(idx) = worst {
        if span_hit_rank(&hit, span) > span_hit_rank(&best[idx], span) {
            best[idx] = hit;
        }
    }
}

fn span_hit_rank(hit: &(u32, String), span: usize) -> (u8, u32, String) {
    let exact = u8::from(hit.1.chars().count() == span);
    (exact, hit.0, hit.1.clone())
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
    packs: HashMap<String, Arc<DatLexicon>>,
    handles: HashMap<String, LangLexiconHandle>,
    next_handle: u64,
    active_pack: Option<String>,
    /// BCP-47-ish tag: zh / en / vi / th …
    active_lang: Option<String>,
    user_words: Option<Arc<Mutex<UserWordStore>>>,
    shared_ngram: Option<SharedCharNgram>,
}

fn lang_from_pack_id(pack_id: &str) -> String {
    let id = pack_id.to_ascii_lowercase();
    if id.starts_with("zh") || id.contains("zh-") {
        return "zh".into();
    }
    if id.starts_with("en") || id.contains("en-") {
        return "en".into();
    }
    if id.starts_with("vi") || id.contains("vi-") {
        return "vi".into();
    }
    if id.starts_with("th") || id.contains("th-") {
        return "th".into();
    }
    id.split('-').next().unwrap_or("").to_string()
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

    pub fn active_lang(&self) -> &str {
        self.active_lang.as_deref().unwrap_or("")
    }

    pub fn set_active_lang(&mut self, lang: impl Into<String>) {
        let lang = lang.into().trim().to_ascii_lowercase();
        self.active_lang = if lang.is_empty() { None } else { Some(lang) };
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
        self.packs.insert(pack_id.to_string(), Arc::new(lex));
        if self.active_pack.is_none() {
            self.active_pack = Some(pack_id.to_string());
            self.active_lang = Some(lang_from_pack_id(pack_id));
        }
        self.publish_active_ngram();
        Ok(())
    }

    pub fn close_lang(&mut self, pack_id: &str) -> HotResult<()> {
        self.packs.remove(pack_id);
        self.handles.remove(pack_id);
        if self.active_pack.as_deref() == Some(pack_id) {
            self.active_pack = None;
            self.active_lang = None;
            if let Some(shared) = &self.shared_ngram {
                shared.clear();
            }
        }
        self.publish_active_ngram();
        Ok(())
    }

    pub fn set_active(&mut self, pack_id: &str) {
        self.active_pack = Some(pack_id.to_string());
        self.active_lang = Some(lang_from_pack_id(pack_id));
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
            cands = merge_user_boosts_lang(self.active_lang(), prefix, cands, &store.lock());
        }
        cands
    }

    pub fn lookup_pinyin(&self, composing: &str, syllables: &[String]) -> Vec<Candidate> {
        self.lookup_pinyin_opts(composing, syllables, None, 0, LookupOpts::lean())
            .unwrap_or_default()
    }

    pub fn lookup_pinyin_opts(
        &self,
        composing: &str,
        syllables: &[String],
        cancel: Option<&LookupCancel>,
        mine: u64,
        opts: LookupOpts,
    ) -> Option<Vec<Candidate>> {
        let mut cands = if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                lex.lookup_pinyin_opts(composing, syllables, cancel, mine, opts)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        if cancel.is_some_and(|c| c.is_canceled(mine)) {
            return None;
        }
        if self.user_words.is_some() {
            cands = self.apply_user_boosts(composing, cands, syllables);
        }
        if let Some(lex) = self.active_lexicon_arc() {
            attach_spans(&lex, composing, syllables, &mut cands);
        }
        Some(cands)
    }

    /// Re-apply learned words after a raw DAT pool replace (async poll / flush).
    pub fn apply_user_boosts(
        &self,
        composing: &str,
        cands: Vec<Candidate>,
        syllables: &[String],
    ) -> Vec<Candidate> {
        let Some(store) = &self.user_words else {
            return cands;
        };
        let mut cands = merge_user_boosts_lang_syls(
            self.active_lang(),
            composing,
            cands,
            &store.lock(),
            Some(syllables),
        );
        if let Some(lex) = self.active_lexicon_arc() {
            attach_spans(&lex, composing, syllables, &mut cands);
        }
        cands
    }

    /// Cheap Arc clone of active DAT for background lookup threads.
    pub fn active_lexicon_arc(&self) -> Option<Arc<DatLexicon>> {
        let id = self.active_pack.as_ref()?;
        self.packs.get(id).cloned()
    }

    pub fn touch_user_word(&self, query_key: &str, word: &str) {
        if let Some(store) = &self.user_words {
            store.lock().touch_lang(self.active_lang(), query_key, word);
            UserWordStore::schedule_flush_shared(store.clone());
        }
    }

    pub fn touch_user_word_lang(&self, lang: &str, query_key: &str, word: &str) {
        if let Some(store) = &self.user_words {
            store.lock().touch_lang(lang, query_key, word);
            UserWordStore::schedule_flush_shared(store.clone());
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
    normalize_lookup_key(raw)
}

/// Lexicon / composing lookup key:
/// - Thai script → keep Thai chars (Kedmanee composing prefix match)
/// - otherwise → Vietnamese/Latin diacritic strip to ASCII
pub fn normalize_lookup_key(raw: &str) -> String {
    let s = raw.trim();
    if s.chars().any(is_thai_char) {
        s.chars().filter(|&c| is_thai_char(c)).collect()
    } else {
        romanize_latin(s)
    }
}

fn is_thai_char(c: char) -> bool {
    matches!(c, '\u{0E00}'..='\u{0E7F}')
}

/// Strip Vietnamese (and generic Latin) diacritics to ASCII letters for lexicon keys.
/// `đ/Đ` → `d`; combining tones removed via base-letter map; non-letters dropped.
pub fn romanize_latin(raw: &str) -> String {
    raw.chars().filter_map(latin_base_char).collect()
}

fn latin_base_char(c: char) -> Option<char> {
    let lower = c.to_lowercase().next().unwrap_or(c);
    let base = match lower {
        'a' | 'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ'
        | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
        'e' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
        'i' | 'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
        'o' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
        | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
        'u' | 'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
        'y' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
        'd' | 'đ' => 'd',
        c if c.is_ascii_alphanumeric() => c,
        _ => return None,
    };
    Some(base)
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
    fn romanize_latin_vi_diacritics() {
        assert_eq!(romanize_latin("xin chào"), "xinchao");
        assert_eq!(romanize_latin("Đà Nẵng"), "danang");
        assert_eq!(romanize_latin("ưở"), "uo");
        assert_eq!(normalize_romanized("Xin Chào"), "xinchao");
    }

    #[test]
    fn normalize_lookup_key_keeps_thai() {
        assert_eq!(normalize_lookup_key("สวัสดี"), "สวัสดี");
        assert_eq!(normalize_lookup_key("  ไทย  "), "ไทย");
        assert_eq!(normalize_lookup_key("xin chào"), "xinchao");
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
    fn associate_without_lexicon_prefix_still_suggests() {
        let tmp = std::env::temp_dir().join("yc_lexicon_assoc_backoff.tsv");
        let tsv = "\
word\tfreq\tpinyin
我们\t1000\twomen
好的\t90000\thaode
是\t80000\tshi
了\t70000\tle
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let cands = lex.associate("我们", 8);
        assert!(
            !cands.is_empty(),
            "backoff next-char should not be empty"
        );
        assert!(cands.iter().any(|c| c.text.chars().count() == 1));
        let _ = std::fs::remove_file(tmp);
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
    fn lookup_opts_cancel_returns_none() {
        let tmp = std::env::temp_dir().join("yc_lexicon_cancel.tsv");
        let tsv = "word\tfreq\tpinyin\n你好\t100\tnihao\n";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let cancel = LookupCancel::new();
        let mine = cancel.bump();
        let _ = cancel.bump(); // invalidate
        let out = lex.lookup_pinyin_opts(
            "nihao",
            &["ni".into(), "hao".into()],
            Some(&cancel),
            mine,
            LookupOpts::lean(),
        );
        assert!(out.is_none());
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn adjacent_typo_wn_yields_women() {
        let tmp = std::env::temp_dir().join("yc_lexicon_wn_typo.tsv");
        let tsv = "\
word\tfreq\tpinyin
我们\t90000\twomen
问题\t80000\twenti
吻\t1000\twen
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec![
            "wo".into(),
            "men".into(),
            "wen".into(),
            "ti".into(),
        ];
        let direct = lex.lookup_pinyin("wm", &syls);
        assert!(
            direct.iter().any(|c| c.text == "我们"),
            "direct wm should hit 我们: {:?}",
            direct.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let variants = crate::typo::adjacent_typo_variants("wn");
        assert!(variants.contains(&"wm".to_string()), "variants={:?}", variants);
        let cands = lex.lookup_pinyin("wn", &syls);
        assert!(
            cands.iter().any(|c| c.text == "我们"),
            "wn (n→m typo) should hit 我们 via wm: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let exact = lex.lookup_pinyin("wm", &syls);
        assert!(exact.iter().any(|c| c.text == "我们"));
        let women = lex.lookup_pinyin("women", &syls);
        assert!(women.iter().any(|c| c.text == "我们"));
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn jianpin_nh_still_yields_nihao() {
        let tmp = std::env::temp_dir().join("yc_lexicon_nh_jianpin.tsv");
        let tsv = "\
word\tfreq\tpinyin
你好\t90000\tnihao
你们\t80000\tnimen
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "men".into()];
        let cands = lex.lookup_pinyin("nh", &syls);
        assert!(
            cands.iter().any(|c| c.text == "你好"),
            "jianpin nh → 你好 still works: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn jianpin_hh_yields_haha() {
        let tmp = std::env::temp_dir().join("yc_lexicon_hh_jianpin.tsv");
        let tsv = "\
word\tfreq\tpinyin
哈哈\t90000\thaha
哈\t80000\tha
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ha".into()];
        let cands = lex
            .lookup_pinyin_opts("hh", &syls, None, 0, crate::LookupOpts::instant())
            .unwrap();
        assert!(
            cands.iter().any(|c| c.text == "哈哈"),
            "jianpin hh → 哈哈: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn jianpin_zh_keeps_full_and_initials() {
        let tmp = std::env::temp_dir().join("yc_lexicon_zh_jianpin.tsv");
        let tsv = "\
word\tfreq\tpinyin
这\t90000\tzhe
中\t80000\tzhong
最好\t70000\tzuihao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["zhe".into(), "zhong".into(), "zui".into(), "hao".into()];
        let cands = lex
            .lookup_pinyin_opts("zh", &syls, None, 0, crate::LookupOpts::instant())
            .unwrap();
        let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
        assert!(texts.contains(&"这"), "full pinyin zh → 这: {texts:?}");
        assert!(texts.contains(&"中"), "full pinyin zh → 中: {texts:?}");
        assert!(texts.contains(&"最好"), "jianpin zh → 最好: {texts:?}");
        assert_eq!(texts[0], "这", "full pinyin should lead jianpin sentence: {texts:?}");
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn fuzzy_zongguo_finds_zhongguo_on_lean() {
        let tmp = std::env::temp_dir().join("yc_lexicon_fuzzy_zongguo.tsv");
        let tsv = "\
word\tfreq\tpinyin
中国\t90000\tzhongguo
总\t80000\tzong
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["zhong".into(), "guo".into(), "zong".into()];
        let cands = lex
            .lookup_pinyin_opts("zongguo", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            cands.iter().any(|c| c.text == "中国"),
            "fuzzy zongguo → 中国: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn instant_hhhhh_returns_without_hanging() {
        let tmp = std::env::temp_dir().join("yc_lexicon_hhhhh.tsv");
        let tsv = "\
word\tfreq\tpinyin
哈\t90000\tha
哈哈\t80000\thaha
好\t70000\thao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["ha".into(), "hao".into()];
        let start = std::time::Instant::now();
        let cands = lex
            .lookup_pinyin_opts("hhhhh", &syls, None, 0, LookupOpts::instant())
            .unwrap();
        assert!(
            start.elapsed().as_millis() < 200,
            "hhhhh instant took {:?}",
            start.elapsed()
        );
        // May be empty (no 5-slot jianpin key); must not hang.
        let _ = cands;
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn user_nihy_reappears_after_dat_overwrite() {
        let dir = std::env::temp_dir().join("yc_lexicon_user_nihy");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat_path = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n你\t90000\tni\n好\t80000\thao\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat_path, &bytes).unwrap();

        let mut mgr = LexiconManager::new();
        mgr.open_lang("zh-test", dat_path.to_str().unwrap()).unwrap();
        mgr.set_active("zh-test");
        let store = std::sync::Arc::new(parking_lot::Mutex::new(UserWordStore::new()));
        store.lock().touch_lang("zh", "nihy", "你好呀");
        mgr.set_user_words(store);

        let syls = vec!["ni".into(), "hao".into(), "ya".into()];
        let cands = mgr
            .lookup_pinyin_opts("nihy", &syls, None, 0, LookupOpts::instant())
            .unwrap();
        let hit = cands.iter().find(|c| c.text == "你好呀").expect(&format!(
            "learned 你好呀 missing: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        ));
        assert_eq!(hit.code_len, 4);
        assert_eq!(cands[0].text, "你好呀");

        let raw = mgr
            .active_lexicon_arc()
            .unwrap()
            .lookup_pinyin_opts("nihy", &syls, None, 0, LookupOpts::instant())
            .unwrap();
        assert!(
            raw.iter().all(|c| c.text != "你好呀"),
            "raw DAT must not invent 你好呀"
        );
        let merged = mgr.apply_user_boosts("nihy", raw, &syls);
        assert_eq!(merged[0].text, "你好呀");
        assert_eq!(merged[0].code_len, 4);
        let _ = std::fs::remove_dir_all(dir);
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
        let ranked = crate::merge_user_boosts("tao", cands, &store);
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

    #[test]
    fn instant_liuchang_liu_char_code_len_is_3() {
        let tmp = std::env::temp_dir().join("yc_lexicon_instant_liuchang.tsv");
        let tsv = "\
word\tfreq\tpinyin
流\t80000\tliu
畅\t70000\tchang
流畅\t95000\tliuchang
淘\t90000\ttao
";
        std::fs::write(&tmp, tsv).unwrap();
        let dat = compile_tsv_to_dat(&tmp).unwrap();
        let lex = DatLexicon::from_bytes(dat).unwrap();
        let syls = vec!["tao".into(), "liu".into(), "chang".into()];
        let cands = lex
            .lookup_pinyin_opts("liuchang", &syls, None, 0, LookupOpts::instant())
            .unwrap();
        let liu = cands.iter().find(|c| c.text == "流").expect("流 on instant");
        assert_eq!(
            liu.code_len, 3,
            "instant first-syl 流 code_len={}",
            liu.code_len
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn lean_nuhao_typo_finds_nihao() {
        let tmp = std::env::temp_dir().join("yc_lexicon_nuhao.tsv");
        std::fs::write(
            &tmp,
            "word\tfreq\tpinyin\n你好\t90000\tnihao\n怒\t1000\tnu\n",
        )
        .unwrap();
        let lex = DatLexicon::from_bytes(compile_tsv_to_dat(&tmp).unwrap()).unwrap();
        let syls = vec!["ni".into(), "hao".into(), "nu".into()];
        let cands = lex
            .lookup_pinyin_opts("nuhao", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            cands.iter().any(|c| c.text == "你好"),
            "nuhao typo → 你好: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn lean_nhao_edit1_finds_nihao() {
        let tmp = std::env::temp_dir().join("yc_lexicon_nhao.tsv");
        std::fs::write(&tmp, "word\tfreq\tpinyin\n你好\t90000\tnihao\n").unwrap();
        let lex = DatLexicon::from_bytes(compile_tsv_to_dat(&tmp).unwrap()).unwrap();
        let syls = vec!["ni".into(), "hao".into()];
        let cands = lex
            .lookup_pinyin_opts("nhao", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            cands.iter().any(|c| c.text == "你好"),
            "nhao edit1 → 你好: {:?}",
            cands.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_file(tmp);
    }

    #[test]
    fn exact_nihao_correction_not_first() {
        let tmp = std::env::temp_dir().join("yc_lexicon_nihao_first.tsv");
        std::fs::write(
            &tmp,
            "word\tfreq\tpinyin\n你好\t90000\tnihao\n泥嚎\t80000\tnihao\n",
        )
        .unwrap();
        let lex = DatLexicon::from_bytes(compile_tsv_to_dat(&tmp).unwrap()).unwrap();
        let syls = vec!["ni".into(), "hao".into()];
        let cands = lex
            .lookup_pinyin_opts("nihao", &syls, None, 0, LookupOpts::lean())
            .unwrap();
        assert!(
            !cands.is_empty() && !crate::rank::is_correction_cand(&cands[0]),
            "exact nihao #0 must not be correction band: score={}",
            cands[0].score
        );
        let _ = std::fs::remove_file(tmp);
    }
}
