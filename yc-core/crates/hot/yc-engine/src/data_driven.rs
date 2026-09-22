//! Data-driven engine: scheme transform + lexicon lookup + candidate paging.

use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{
    clears_assoc_context, infer_code_len, score_l1, score_l2, score_l3, score_l4, tighten_code_lens,
    LexiconManager, LookupOpts, UserWordStore, AI_WAVE_CAP,
};
use yc_scheme::SchemeDesc;
use yc_scheme::TransformKind;
use yc_types::{
    Candidate, CandidateSource, ComposingText, EditorId, EngineError, EngineStep, HotResult,
    InputMode, UiCommand, MAX_CANDIDATES,
};

use crate::pinyin_seg::{is_valid_pinyin_input, normalize_query};
use crate::{invalid_session, key_code_to_char, session_invalid, InputEngine};

/// Max association candidates after select (CandBar pages of 9).
pub const ASSOC_CANDIDATE_LIMIT: usize = 200;
/// Max AI phrases per inject wave (L1 / L2 / span).
pub const LLM_FALLBACK_MIN_POOL: usize = 5;
/// status_flags bit2: composing needs LLM candidate fill.
pub const STATUS_NEEDS_LLM_FALLBACK: u32 = 0x4;

/// AI inject wave: 0 = need L1, 1 = L1 done need L2, 2 = L2 done (span empty only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LlmWave {
    NeedL1 = 0,
    NeedL2 = 1,
    NeedSpan = 2,
    Done = 3,
}

#[derive(Debug)]
pub struct DataDrivenEngine {
    active: EditorId,
    composing: String,
    lexicon: LexiconManager,
    pack_id: String,
    scheme: SchemeDesc,
    /// Full ranked candidate pool (before paging).
    cand_pool: Vec<Candidate>,
    cand_page: u32,
    last_query_key: String,
    /// Accumulated committed text used for lexicon association.
    assoc_context: String,
    /// True when `cand_pool` currently holds association suffixes.
    assoc_active: bool,
    /// True after an expanded (scroll/more) lookup filled beyond lean first-screen.
    pool_expanded: bool,
    /// True after lean (async/sync) pool applied for `last_query_key`.
    /// Instant first-syl fill alone leaves this false so flush can still upgrade.
    lean_ready: bool,
    /// True when shell should call LLM to top up candidates for current query.
    needs_llm_fallback: bool,
    /// Query key for which Ai candidates were already injected (tracks wave progress).
    llm_injected_query: String,
    llm_wave: LlmWave,
    /// L3/L4 spans with no local lexicon hit (AI only when empty).
    llm_empty_spans: Vec<String>,
    /// Multi-select compose: consumed pinyin spans + chosen text for new-word learning.
    compose_py: String,
    compose_buf: String,
    /// Original full composing when the compose chain started (for touch key).
    compose_origin: String,
}

impl DataDrivenEngine {
    pub fn new(pack_id: String, scheme: SchemeDesc) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
            scheme,
            cand_pool: Vec::new(),
            cand_page: 0,
            last_query_key: String::new(),
            assoc_context: String::new(),
            assoc_active: false,
            pool_expanded: false,
            lean_ready: false,
            needs_llm_fallback: false,
            llm_injected_query: String::new(),
            llm_wave: LlmWave::NeedL1,
            llm_empty_spans: Vec::new(),
            compose_py: String::new(),
            compose_buf: String::new(),
            compose_origin: String::new(),
        }
    }

    pub fn load_lexicon(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        self.lexicon.open_lang(pack_id, path)?;
        self.lexicon.set_active(pack_id);
        self.pack_id = pack_id.to_string();
        Ok(())
    }

    pub fn set_user_words(&mut self, store: Arc<Mutex<UserWordStore>>) {
        self.lexicon.set_user_words(store);
    }

    pub fn set_shared_ngram(&mut self, shared: yc_lexicon::SharedCharNgram) {
        self.lexicon.set_shared_ngram(shared);
    }

    pub fn set_cand_pool(&mut self, cands: Vec<Candidate>) {
        self.cand_pool = cands;
        self.cand_page = 0;
        self.pool_expanded = false;
        self.lean_ready = true;
        self.clear_llm_state_for_new_pool();
    }

    fn clear_llm_state_for_new_pool(&mut self) {
        let q = self.transformed(&self.composing);
        if self.llm_injected_query != q {
            self.llm_injected_query.clear();
            self.llm_wave = LlmWave::NeedL1;
            self.llm_empty_spans.clear();
        }
        self.needs_llm_fallback = false;
    }

    fn clear_compose_learn(&mut self) {
        self.compose_py.clear();
        self.compose_buf.clear();
        self.compose_origin.clear();
    }

    fn note_compose_span(&mut self, py_span: &str, text: &str) {
        if py_span.is_empty() || text.is_empty() {
            return;
        }
        self.compose_py.push_str(py_span);
        self.compose_buf.push_str(text);
    }

    fn finish_compose_learn_if_done(&mut self) {
        if self.composing.is_empty()
            && !self.compose_py.is_empty()
            && !self.compose_buf.is_empty()
        {
            let learn = if !self.compose_origin.is_empty() {
                self.compose_origin.clone()
            } else {
                self.compose_py.clone()
            };
            self.lexicon
                .touch_user_word(&learn, &self.compose_buf.clone());
            self.clear_compose_learn();
        }
    }

    pub fn needs_llm_fallback(&self) -> bool {
        self.needs_llm_fallback
    }

    /// Refresh empty L3/L4 span hints from current composing (local miss → AI later).
    #[allow(dead_code)]
    fn refresh_empty_spans(&mut self) {
        self.llm_empty_spans.clear();
        if self.scheme.transform != TransformKind::Table {
            return;
        }
        let q = self.transformed(&self.composing);
        if q.len() < 2 {
            return;
        }
        let Some(lex) = self.lexicon.active_lexicon_arc() else {
            return;
        };
        // First syllable + letter splits under it.
        let syls = &self.scheme.syllables;
        let mut best_syl = None::<usize>;
        for s in syls {
            if !s.is_empty() && q.starts_with(s.as_str()) {
                let len = s.len();
                if best_syl.map_or(true, |b| len > b) {
                    best_syl = Some(len);
                }
            }
        }
        if let Some(end) = best_syl {
            let key = &q[..end];
            if lex.exact_key_words_pub(key).is_empty() && !yc_lexicon::is_orphan_final(key) {
                self.llm_empty_spans.push(key.to_string());
            }
            for i in 0..end {
                let ch = &q[i..i + 1];
                if yc_lexicon::is_orphan_final(ch) {
                    continue;
                }
                if lex.exact_key_words_pub(ch).is_empty() {
                    self.llm_empty_spans.push(ch.to_string());
                }
            }
        }
        self.llm_empty_spans.sort();
        self.llm_empty_spans.dedup();
    }

    fn refresh_llm_fallback_flag(&mut self) {
        // Traditional Table composing never requests LLM fill (hot path 无 LLM 填池).
        self.needs_llm_fallback = false;
        let _ = (
            self.composing.len(),
            self.assoc_active,
            self.scheme.transform,
            self.llm_wave,
        );
    }

    fn request_llm_fallback_at_end(&mut self) {
        // No LLM fill on traditional composing CandBar.
        self.needs_llm_fallback = false;
    }

    /// Append texts for an **independent AI panel** only — does not set needs_llm_fallback.
    /// Traditional composing path should not call this to top-up CandBar.
    pub fn inject_ai_candidates(&mut self, query: &str, texts: &[String]) -> bool {
        let current = self.transformed(&self.composing);
        if current != query || query.len() < 2 {
            return false;
        }
        let mut seen: std::collections::HashSet<String> =
            self.cand_pool.iter().map(|c| c.text.clone()).collect();
        let full_len = query.len() as u32;
        let wave = self.llm_wave;
        let lex = self.lexicon.active_lexicon_arc();
        let syls = &self.scheme.syllables;
        let mut added = 0usize;
        for (i, raw) in texts.iter().enumerate().take(AI_WAVE_CAP) {
            let text = raw.trim();
            if text.is_empty() || !seen.insert(text.to_string()) {
                continue;
            }
            // L2 full composition may keep query.len(); still tighten via infer for short texts.
            let default_full = matches!(wave, LlmWave::NeedL2);
            let span_len = infer_code_len(lex.as_deref(), query, syls, text, default_full);
            let (score, span_len) = match wave {
                LlmWave::NeedL1 => (score_l1(i), span_len),
                LlmWave::NeedL2 => (score_l2(0.90 - i as f32 * 0.002), span_len),
                LlmWave::NeedSpan | LlmWave::Done => {
                    let chars = text.chars().count();
                    if chars <= 1 {
                        (score_l4(i), span_len)
                    } else {
                        (score_l3(i, chars.min(8)), span_len.min(full_len))
                    }
                }
            };
            self.cand_pool.push(Candidate {
                id: 0,
                text: text.to_string(),
                source: CandidateSource::Ai,
                score,
                code_len: span_len,
            });
            // Learn under the consumed span, not the full query (avoid taoliuchang→套).
            let touch_key = if span_len > 0 && (span_len as usize) <= query.len() {
                &query[..span_len as usize]
            } else {
                query
            };
            self.lexicon.touch_user_word(touch_key, text);
            added += 1;
        }
        if let Some(ref lex) = lex {
            tighten_code_lens(lex, query, syls, &mut self.cand_pool);
        }
        self.cand_pool.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.text.cmp(&b.text))
        });
        for (i, c) in self.cand_pool.iter_mut().enumerate() {
            c.id = i as u32;
        }
        self.llm_injected_query = query.to_string();
        self.llm_wave = match wave {
            LlmWave::NeedL1 => LlmWave::NeedL2,
            LlmWave::NeedL2 => {
                if self.llm_empty_spans.is_empty() {
                    LlmWave::Done
                } else {
                    LlmWave::NeedSpan
                }
            }
            LlmWave::NeedSpan => LlmWave::Done,
            LlmWave::Done => LlmWave::Done,
        };
        self.refresh_llm_fallback_flag();
        self.last_query_key = query.to_string();
        let _ = added;
        true
    }

    /// Replace pool without resetting page (used after intel rerank of same query).
    pub fn replace_cand_pool_keep_page(&mut self, cands: Vec<Candidate>) {
        let max_page = page_count(&cands).saturating_sub(1);
        self.cand_pool = cands;
        if self.cand_page > max_page {
            self.cand_page = max_page;
        }
    }

    pub fn set_last_candidates(&mut self, cands: Vec<Candidate>) {
        self.set_cand_pool(cands);
    }

    pub fn last_query_key(&self) -> &str {
        &self.last_query_key
    }

    pub fn composing_text(&self) -> &str {
        &self.composing
    }

    pub fn cand_page(&self) -> u32 {
        self.cand_page
    }

    pub fn cand_total(&self) -> u32 {
        self.cand_pool.len() as u32
    }

    pub fn touch_user_word(&self, pinyin: &str, word: &str) {
        self.lexicon.touch_user_word(pinyin, word);
    }

    pub fn associate(&self, prefix: &str, limit: usize) -> Vec<Candidate> {
        self.lexicon.associate(prefix, limit)
    }

    pub fn assoc_context(&self) -> &str {
        &self.assoc_context
    }

    pub fn clear_assoc(&mut self) {
        self.assoc_context.clear();
        self.assoc_active = false;
    }

    pub fn set_assoc_active(&mut self, active: bool) {
        self.assoc_active = active;
    }

    /// Update association context after Commit. Candidate pool is filled only by
    /// `Scheduler::fill_association` (shared with handwriting).
    fn apply_commit_association(&mut self, committed: &str) {
        if clears_assoc_context(committed) {
            self.clear_assoc();
            self.cand_pool.clear();
            self.cand_page = 0;
            return;
        }
        if self.assoc_active {
            self.assoc_context.push_str(committed);
        } else {
            self.assoc_context = committed.to_string();
        }
        self.cand_pool.clear();
        self.cand_page = 0;
        // Stay in assoc mode until Scheduler fills; empty fill clears via set_assoc_active.
        self.assoc_active = true;
    }

    fn transformed(&self, raw: &str) -> String {
        match self.scheme.transform {
            TransformKind::RuleChain => self.scheme.apply_rule_chain(raw),
            TransformKind::Table | TransformKind::LatinPredict => normalize_query(raw),
        }
    }

    fn paged_candidates(&self) -> Vec<Candidate> {
        page_slice(&self.cand_pool, self.cand_page)
    }

    fn step_from_pool(&mut self, composing: String) -> EngineStep {
        self.last_query_key = self.transformed(&composing);
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            // Full pool for scheduler rerank; ImmSnapshot will page.
            candidates: self.cand_pool.clone(),
            commands: Vec::new(),
        }
    }

    /// Same-frame fill, then context boost from the last committed character.
    fn lookup_instant(&self) -> Vec<Candidate> {
        let query = self.transformed(&self.composing);
        let cands = self
            .lexicon
            .lookup_pinyin_opts(
                &query,
                &self.scheme.syllables,
                None,
                0,
                LookupOpts::instant(),
            )
            .unwrap_or_default();
        self.apply_context_boost(cands)
    }

    fn lookup(&self) -> Vec<Candidate> {
        let query = self.transformed(&self.composing);
        let cands = if self.scheme.transform == TransformKind::Table {
            self.lexicon.lookup_pinyin(&query, &self.scheme.syllables)
        } else {
            self.lexicon.lookup(&query)
        };
        self.apply_context_boost(cands)
    }

    /// Nudge scores using the last committed character. Empty context is unchanged.
    fn apply_context_boost(&self, mut cands: Vec<Candidate>) -> Vec<Candidate> {
        let ctx = self.assoc_context.as_str();
        if ctx.is_empty() || clears_assoc_context(ctx) || cands.is_empty() {
            return cands;
        }
        yc_lexicon::apply_ngram_nudge(&mut cands, ctx, |context, text| {
            self.lexicon.continuation_score(context, text)
        });
        cands
    }

    /// Instant fill + background lean upgrade for Table composing updates.
    fn fill_table_composing_cands(&mut self) {
        self.cand_pool = self.lookup_instant();
        self.cand_page = 0;
        self.last_query_key = self.transformed(&self.composing);
        self.pool_expanded = false;
        // Instant-only until poll/flush applies lean.
        self.lean_ready = false;
        // New composing key → restart AI waves at L1.
        self.llm_injected_query.clear();
        self.llm_wave = LlmWave::NeedL1;
        self.llm_empty_spans.clear();
        self.refresh_llm_fallback_flag();
        self.schedule_async_lookup();
    }

    /// Schedule cancelable background lookup for current composing (pinyin table only).
    fn schedule_async_lookup(&self) {
        if self.scheme.transform != TransformKind::Table || self.composing.is_empty() {
            return;
        }
        let Some(lex) = self.lexicon.active_lexicon_arc() else {
            return;
        };
        let query = self.transformed(&self.composing);
        let hub = crate::AsyncLookupHub::shared();
        hub.submit(
            query,
            self.scheme.syllables.clone(),
            lex,
        );
    }

    /// Apply background result if it matches the latest generation. Returns true if pool updated.
    pub fn poll_async_lookup(&mut self) -> bool {
        let query = self.transformed(&self.composing);
        let hub = crate::AsyncLookupHub::shared();
        let Some((_gen, cands)) = hub.take_if_matches(&query) else {
            return false;
        };
        self.cand_pool = self.apply_context_boost(self.lexicon.apply_user_boosts(
            &query,
            cands,
            &self.scheme.syllables,
        ));
        self.cand_page = 0;
        self.last_query_key = query;
        self.pool_expanded = false;
        self.lean_ready = true;
        self.llm_injected_query.clear();
        self.refresh_llm_fallback_flag();
        true
    }

    pub fn cand_pool_clone(&self) -> Vec<Candidate> {
        self.cand_pool.clone()
    }

    /// Wait for current lookup (space/select barrier). Returns true if pool was filled.
    pub fn flush_async_lookup(&mut self, timeout_ms: u64) -> bool {
        let query = self.transformed(&self.composing);
        // Lean already applied for this composing — keep cand_page (paging + select).
        if self.lean_ready && !self.cand_pool.is_empty() && self.last_query_key == query {
            return true;
        }
        let hub = crate::AsyncLookupHub::shared();
        if let Some(cands) =
            hub.wait_for(&query, std::time::Duration::from_millis(timeout_ms))
        {
            self.cand_pool = self.apply_context_boost(self.lexicon.apply_user_boosts(
            &query,
            cands,
            &self.scheme.syllables,
        ));
            self.cand_page = 0;
            self.last_query_key = query;
            self.pool_expanded = false;
            self.lean_ready = true;
            self.llm_injected_query.clear();
            self.refresh_llm_fallback_flag();
            return true;
        }
        // Fallback: sync lean lookup so space never commits empty wrongly.
        if (!self.lean_ready || self.cand_pool.is_empty()) && !self.composing.is_empty() {
            self.cand_pool = self.lookup();
            self.cand_page = 0;
            self.last_query_key = query;
            self.pool_expanded = false;
            self.lean_ready = true;
            self.llm_injected_query.clear();
            self.refresh_llm_fallback_flag();
            return !self.cand_pool.is_empty();
        }
        !self.cand_pool.is_empty()
    }

    /// When user pages past the lean first-screen pool, run expanded lookup once.
    fn expand_cand_pool_if_needed(&mut self) {
        if self.pool_expanded || self.composing.is_empty() || self.assoc_active {
            return;
        }
        if self.scheme.transform != TransformKind::Table {
            return;
        }
        let query = self.transformed(&self.composing);
        let cands = self
            .lexicon
            .lookup_pinyin_opts(
                &query,
                &self.scheme.syllables,
                None,
                0,
                LookupOpts::expanded(),
            )
            .unwrap_or_default();
        // Always adopt expanded pool (includes P2 fine splits) so end-of-page
        // listing continues by segmentation even when lean already had ≥5 hits.
        self.cand_pool = self.apply_context_boost(self.lexicon.apply_user_boosts(
            &query,
            cands,
            &self.scheme.syllables,
        ));
        self.last_query_key = query;
        self.llm_injected_query.clear();
        self.refresh_llm_fallback_flag();
        self.pool_expanded = true;
        self.lean_ready = true;
    }

    pub fn page_next(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let pages = page_count(&self.cand_pool);
        if pages == 0 {
            self.request_llm_fallback_at_end();
            return Err(EngineError::Unsupported);
        }
        // At last lean page → expand pool, then advance if more pages appear.
        if self.cand_page + 1 >= pages {
            self.expand_cand_pool_if_needed();
        }
        let pages = page_count(&self.cand_pool);
        if self.cand_page + 1 < pages {
            self.cand_page += 1;
        } else {
            // Still on last page after expand → ask shell for LLM top-up.
            self.request_llm_fallback_at_end();
        }
        Ok(self.step_from_pool(self.composing.clone()))
    }

    pub fn page_prev(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        if self.cand_pool.is_empty() {
            return Err(EngineError::Unsupported);
        }
        if self.cand_page > 0 {
            self.cand_page -= 1;
        }
        Ok(self.step_from_pool(self.composing.clone()))
    }

    pub fn current_paged_step(&self) -> EngineStep {
        EngineStep {
            composing: ComposingText {
                text: self.composing.clone(),
                cursor: self.composing.len() as u32,
            },
            candidates: self.paged_candidates(),
            commands: Vec::new(),
        }
    }
}

impl InputEngine for DataDrivenEngine {
    fn reset(&mut self, editor_id: EditorId) {
        self.active = editor_id;
        self.composing.clear();
        self.cand_pool.clear();
        self.cand_page = 0;
        self.last_query_key.clear();
        self.pool_expanded = false;
        self.lean_ready = false;
        self.clear_assoc();
        self.clear_compose_learn();
    }

    fn feed(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
        input_mode: &InputMode,
    ) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        // New composing input ends association mode.
        if key_code != b' ' as u32 {
            self.clear_assoc();
        }
        if key_code == b' ' as u32 {
            if input_mode.ascii_mode {
                let text = self.composing.clone();
                self.composing.clear();
                self.cand_pool.clear();
                self.cand_page = 0;
                self.apply_commit_association(&text);
                return Ok(EngineStep {
                    composing: ComposingText::empty(),
                    candidates: Vec::new(),
                    commands: vec![UiCommand::Commit { text }],
                });
            }
            // Barrier: wait for latest-gen lookup before committing first candidate.
            let _ = self.flush_async_lookup(32);
            let text = page_slice(&self.cand_pool, self.cand_page)
                .first()
                .map(|c| c.text.clone())
                .or_else(|| self.cand_pool.first().map(|c| c.text.clone()))
                .or_else(|| self.lookup().first().map(|c| c.text.clone()))
                .unwrap_or_else(|| self.composing.clone());
            self.composing.clear();
            self.apply_commit_association(&text);
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::Commit { text }],
            });
        }
        let ch = key_code_to_char(key_code).ok_or(EngineError::Unsupported)?;
        self.composing.push(ch);
        if input_mode.ascii_mode {
            // English: no syllable check, no Chinese candidates
            self.cand_pool.clear();
            self.cand_page = 0;
            return Ok(self.step_from_pool(self.composing.clone()));
        }
        if self.scheme.transform == TransformKind::Table
            && !is_valid_pinyin_input(&self.composing, &self.scheme.syllables)
        {
            self.composing.pop();
            return Err(EngineError::Unsupported);
        }
        if self.scheme.transform == TransformKind::Table {
            // Same-frame instant cands; async lean upgrades via poll.
            self.fill_table_composing_cands();
            return Ok(self.step_from_pool(self.composing.clone()));
        }
        self.cand_pool = self.lookup();
        self.cand_page = 0;
        self.last_query_key = self.transformed(&self.composing);
        self.pool_expanded = false;
        self.lean_ready = true;
        Ok(self.step_from_pool(self.composing.clone()))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let _ = self.flush_async_lookup(32);
        let global = self.cand_page * MAX_CANDIDATES as u32 + candidate_id;
        let cand = self
            .cand_pool
            .get(global as usize)
            .cloned()
            .or_else(|| {
                page_slice(&self.cand_pool, self.cand_page)
                    .into_iter()
                    .find(|c| c.id == candidate_id)
            })
            .ok_or(EngineError::Unsupported)?;
        let text = cand.text.clone();
        let full_len = self.composing.len() as u32;
        let consume = if cand.code_len == 0 {
            full_len
        } else {
            cand.code_len.min(full_len)
        };
        let consume_usize = consume as usize;
        let pre_composing = self.composing.clone();

        if self.compose_origin.is_empty() {
            if self.compose_py.is_empty() {
                self.compose_origin = pre_composing.clone();
            } else {
                self.compose_origin = format!("{}{}", self.compose_py, pre_composing);
            }
        }

        if consume_usize > 0 && consume_usize <= pre_composing.len() {
            let py_span = &pre_composing[..consume_usize];
            self.note_compose_span(py_span, &text);
        }

        if consume_usize > 0 && consume_usize < self.composing.len() {
            // Partial select: commit span, keep residual pinyin; refill via async (Table) or sync.
            self.composing = self.composing[consume_usize..].to_string();
            self.clear_assoc();
            self.cand_pool.clear();
            self.cand_page = 0;
            self.last_query_key.clear();
            self.pool_expanded = false;
            self.lean_ready = false;
            if self.scheme.transform == TransformKind::Table {
                if !self.composing.is_empty() {
                    self.fill_table_composing_cands();
                }
            } else if !self.composing.is_empty() {
                self.cand_pool = self.lookup();
                self.last_query_key = self.transformed(&self.composing);
                self.lean_ready = true;
            }
            return Ok(EngineStep {
                composing: ComposingText {
                    text: self.composing.clone(),
                    cursor: self.composing.len() as u32,
                },
                candidates: self.paged_candidates(),
                commands: vec![UiCommand::Commit { text }],
            });
        }

        self.composing.clear();
        self.finish_compose_learn_if_done();
        self.apply_commit_association(&text);
        Ok(EngineStep {
            composing: ComposingText::empty(),
            candidates: Vec::new(),
            commands: vec![UiCommand::Commit { text }],
        })
    }

    fn backspace(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        if self.composing.is_empty() {
            self.clear_assoc();
            self.cand_pool.clear();
            self.cand_page = 0;
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::DeleteSurrounding {
                    before: 1,
                    after: 0,
                }],
            });
        }
        self.clear_assoc();
        self.composing.pop();
        if self.composing.is_empty() {
            self.cand_pool.clear();
            self.cand_page = 0;
            self.last_query_key.clear();
            self.pool_expanded = false;
            self.lean_ready = false;
            self.needs_llm_fallback = false;
            self.llm_injected_query.clear();
            self.clear_compose_learn();
            // Cancel any in-flight lookup.
            let _ = crate::AsyncLookupHub::shared().cancel_token().bump();
            return Ok(self.step_from_pool(String::new()));
        }
        if self.scheme.transform == TransformKind::Table {
            self.fill_table_composing_cands();
            return Ok(self.step_from_pool(self.composing.clone()));
        }
        self.cand_pool = self.lookup();
        self.cand_page = 0;
        self.last_query_key = self.transformed(&self.composing);
        self.pool_expanded = false;
        self.lean_ready = true;
        Ok(self.step_from_pool(self.composing.clone()))
    }
}

pub(crate) fn page_count(pool: &[Candidate]) -> u32 {
    if pool.is_empty() {
        0
    } else {
        ((pool.len() + MAX_CANDIDATES - 1) / MAX_CANDIDATES) as u32
    }
}

pub(crate) fn page_slice(pool: &[Candidate], page: u32) -> Vec<Candidate> {
    let start = page as usize * MAX_CANDIDATES;
    if start >= pool.len() {
        return Vec::new();
    }
    pool[start..]
        .iter()
        .take(MAX_CANDIDATES)
        .enumerate()
        .map(|(i, c)| Candidate {
            id: i as u32,
            text: c.text.clone(),
            source: c.source,
            score: c.score,
            code_len: c.code_len,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_lexicon::compile_tsv_to_dat;
    use yc_scheme::{SchemeDesc, TransformKind};
    use yc_types::InputMode;

    fn table_scheme(syls: &[&str]) -> SchemeDesc {
        SchemeDesc {
            scheme_id: "pinyin_full".into(),
            lang: "zh".into(),
            transform: TransformKind::Table,
            rules: Vec::new(),
            syllables: syls.iter().map(|s| (*s).to_string()).collect(),
            alphabet: String::new(),
        }
    }

    #[test]
    fn pinyin_feed_async_space_commits_latest() {
        let dir = std::env::temp_dir().join("yc_engine_async_pinyin_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n你好\t100\tnihao\n你们\t80\tnimen\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng = DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao", "men"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));

        let mode = InputMode::default();
        for ch in b"nihao" {
            let step = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
            assert!(
                !step.candidates.is_empty(),
                "letter keys should instant-fill candidates"
            );
        }
        assert_eq!(eng.composing_text(), "nihao");
        let _ = eng.flush_async_lookup(200);
        assert!(
            eng.cand_pool_clone().iter().any(|c| c.text == "你好"),
            "pool should contain 你好 after flush"
        );
        let step = eng.feed(EditorId(1), b' ' as u32, &mode).unwrap();
        assert!(
            step.commands
                .iter()
                .any(|c| matches!(c, UiCommand::Commit { text } if text == "你好")),
            "space barrier should commit 你好: {:?}",
            step.commands
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pinyin_select_prefix_keeps_residual() {
        let dir = std::env::temp_dir().join("yc_engine_residual_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n你\t90000\tni\n好\t80000\thao\n吗\t70000\tma\n你好\t95000\tnihao\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng =
            DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao", "ma"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"nihaoma" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let page = page_slice(&eng.cand_pool_clone(), 0);
        let nihao = page
            .iter()
            .find(|c| c.text == "你好")
            .expect("你好 on page 0");
        assert_eq!(nihao.code_len, 5);
        let step = eng.select(EditorId(1), nihao.id).unwrap();
        assert!(
            step.commands
                .iter()
                .any(|c| matches!(c, UiCommand::Commit { text } if text == "你好"))
        );
        assert_eq!(step.composing.text, "ma");
        assert_eq!(eng.composing_text(), "ma");
        let _ = eng.flush_async_lookup(200);
        assert!(
            !eng.cand_pool_clone().is_empty(),
            "residual ma should fill candidates after flush"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pinyin_space_commits_composed_sentence() {
        let dir = std::env::temp_dir().join("yc_engine_compose_space_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n你\t90000\tni\n好\t80000\thao\n吗\t70000\tma\n你好\t95000\tnihao\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng =
            DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao", "ma"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"nihaoma" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let step = eng.feed(EditorId(1), b' ' as u32, &mode).unwrap();
        assert!(
            step.commands
                .iter()
                .any(|c| matches!(c, UiCommand::Commit { text } if text == "你好吗")),
            "space should commit composed 你好吗: {:?}",
            step.commands
        );
        assert!(step.composing.text.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn llm_fallback_flag_disabled_on_traditional_composing() {
        let mut eng = DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao"]));
        eng.reset(EditorId(1));
        eng.composing = "zz".into();
        eng.cand_pool.clear();
        eng.llm_wave = LlmWave::NeedL1;
        eng.refresh_llm_fallback_flag();
        assert!(
            !eng.needs_llm_fallback(),
            "traditional Table composing must not request LLM fill"
        );
        eng.llm_wave = LlmWave::NeedL2;
        eng.refresh_llm_fallback_flag();
        assert!(!eng.needs_llm_fallback());
        eng.request_llm_fallback_at_end();
        assert!(!eng.needs_llm_fallback());
    }

    #[test]
    fn inject_ai_candidates_appends_and_rejects_stale_query() {
        let dir = std::env::temp_dir().join("yc_engine_llm_inject_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n你\t90\tni\n好\t80\thao\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng = DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"ni" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let before = eng.cand_pool_clone().len();
        eng.llm_wave = LlmWave::NeedL1;
        assert!(
            eng.inject_ai_candidates("ni", &["你好呀".into(), "你".into()]),
            "matching query should inject"
        );
        let pool = eng.cand_pool_clone();
        let ai = pool.iter().find(|c| c.text == "你好呀").expect("AI cand");
        assert_eq!(ai.source, CandidateSource::Ai);
        assert!(
            (0.92..0.95).contains(&ai.score),
            "L1 score band: {}",
            ai.score
        );
        assert!(pool.len() >= before);
        // After L1 → NeedL2, flag may still be true.
        assert_eq!(eng.llm_wave, LlmWave::NeedL2);
        assert!(!eng.inject_ai_candidates("hao", &["好的".into()]));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn inject_ai_learns_user_word_for_next_lookup() {
        let dir = std::env::temp_dir().join("yc_engine_llm_learn_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(&tsv, "word\tfreq\tpinyin\n你\t90\tni\n").unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let store = std::sync::Arc::new(parking_lot::Mutex::new(
            yc_lexicon::UserWordStore::new(),
        ));
        let mut eng = DataDrivenEngine::new("zh-test".into(), table_scheme(&["ni", "hao"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.set_user_words(store.clone());
        eng.reset(EditorId(1));
        eng.composing = "zz".into();
        assert!(eng.inject_ai_candidates("zz", &["自定义词".into()]));
        let boosted = yc_lexicon::merge_user_boosts_lang(
            "zh",
            "zz",
            Vec::new(),
            &store.lock(),
        );
        assert!(
            boosted.iter().any(|c| c.text == "自定义词"),
            "learned AI word should merge: {:?}",
            boosted.iter().map(|c| &c.text).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compose_partials_learn_full_pinyin_key() {
        let dir = std::env::temp_dir().join("yc_engine_compose_learn_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n淘\t90000\ttao\n流\t80000\tliu\n畅\t70000\tchang\n流畅\t95000\tliuchang\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let store = std::sync::Arc::new(parking_lot::Mutex::new(
            yc_lexicon::UserWordStore::new(),
        ));
        let mut eng = DataDrivenEngine::new(
            "zh-test".into(),
            table_scheme(&["tao", "liu", "chang", "liuchang"]),
        );
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.set_user_words(store.clone());
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"taoliuchang" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let pool = eng.cand_pool_clone();
        let tao = pool
            .iter()
            .find(|c| c.text == "淘" && c.code_len == 3)
            .expect("淘 first span");
        let id = tao.id;
        let _ = eng.select(EditorId(1), id).unwrap();
        assert_eq!(eng.composing_text(), "liuchang");
        let _ = eng.flush_async_lookup(200);
        let pool2 = eng.cand_pool_clone();
        let liuchang = pool2
            .iter()
            .find(|c| c.text == "流畅")
            .expect("流畅");
        let _ = eng.select(EditorId(1), liuchang.id).unwrap();
        assert!(eng.composing_text().is_empty());
        assert!(
            store.lock().freq_lang("zh", "taoliuchang", "淘流畅") > 0,
            "should learn taoliuchang→淘流畅"
        );
        // Next lookup via jianpin/prefix
        eng.reset(EditorId(1));
        for ch in b"tlc" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let hit = eng
            .cand_pool_clone()
            .iter()
            .any(|c| c.text == "淘流畅");
        assert!(hit, "jianpin tlc should find learned word");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn select_tao_char_keeps_liuchang_residual() {
        let dir = std::env::temp_dir().join("yc_engine_tao_residual_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n套\t7000\ttao\n淘\t90000\ttao\n流\t80000\tliu\n畅\t70000\tchang\n流畅\t95000\tliuchang\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng = DataDrivenEngine::new(
            "zh-test".into(),
            table_scheme(&["tao", "liu", "chang", "liuchang"]),
        );
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"taoliuchang" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let pool = eng.cand_pool_clone();
        let tao = pool
            .iter()
            .find(|c| c.text == "套")
            .expect("套 in pool");
        assert_eq!(tao.code_len, 3, "套 should consume tao only");
        let step = eng.select(EditorId(1), tao.id).unwrap();
        assert!(
            step.commands
                .iter()
                .any(|c| matches!(c, UiCommand::Commit { text } if text == "套"))
        );
        assert_eq!(step.composing.text, "liuchang");
        assert_eq!(eng.composing_text(), "liuchang");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ai_inject_single_char_code_len_is_first_syllable() {
        let dir = std::env::temp_dir().join("yc_engine_ai_tao_span_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        // Deliberately omit 套 from lexicon so AI is the only source.
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n淘\t90000\ttao\n流\t80000\tliu\n畅\t70000\tchang\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let store = std::sync::Arc::new(parking_lot::Mutex::new(
            yc_lexicon::UserWordStore::new(),
        ));
        let mut eng = DataDrivenEngine::new(
            "zh-test".into(),
            table_scheme(&["tao", "liu", "chang"]),
        );
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.set_user_words(store.clone());
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"taoliuchang" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        eng.cand_pool.retain(|c| c.text != "套");
        eng.llm_wave = LlmWave::NeedL1;
        assert!(eng.inject_ai_candidates("taoliuchang", &["套".into()]));
        let pool = eng.cand_pool_clone();
        let tao = pool.iter().find(|c| c.text == "套").expect("AI 套");
        assert_eq!(tao.code_len, 3, "AI 套 code_len={}", tao.code_len);
        assert_eq!(tao.source, CandidateSource::Ai);
        assert!(store.lock().freq_lang("zh", "tao", "套") > 0);
        assert_eq!(store.lock().freq_lang("zh", "taoliuchang", "套"), 0);

        let global = pool.iter().position(|c| c.text == "套").unwrap() as u32;
        eng.cand_page = global / MAX_CANDIDATES as u32;
        let local = global % MAX_CANDIDATES as u32;
        let step = eng.select(EditorId(1), local).unwrap();
        assert_eq!(step.composing.text, "liuchang");
        assert_eq!(eng.composing_text(), "liuchang");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn select_liu_keeps_chang_residual() {
        let dir = std::env::temp_dir().join("yc_engine_liu_chang_residual_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n淘\t90000\ttao\n流\t80000\tliu\n畅\t70000\tchang\n流畅\t95000\tliuchang\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        // Production-like syllables: do NOT register liuchang as one syllable.
        let mut eng = DataDrivenEngine::new(
            "zh-test".into(),
            table_scheme(&["tao", "liu", "chang"]),
        );
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"taoliuchang" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let pool = eng.cand_pool_clone();
        let tao = pool
            .iter()
            .find(|c| c.text == "淘" && c.code_len == 3)
            .expect("淘 first span");
        let global = pool.iter().position(|c| c.text == "淘" && c.code_len == 3).unwrap() as u32;
        eng.cand_page = global / MAX_CANDIDATES as u32;
        let _ = eng
            .select(EditorId(1), global % MAX_CANDIDATES as u32)
            .unwrap();
        assert_eq!(eng.composing_text(), "liuchang");
        let _ = eng.flush_async_lookup(200);

        let pool2 = eng.cand_pool_clone();
        let liu = pool2
            .iter()
            .find(|c| c.text == "流")
            .expect("流 in residual pool");
        assert_eq!(liu.code_len, 3, "流 should consume liu only, got {}", liu.code_len);
        let global2 = pool2.iter().position(|c| c.text == "流").unwrap() as u32;
        eng.cand_page = global2 / MAX_CANDIDATES as u32;
        let step = eng
            .select(EditorId(1), global2 % MAX_CANDIDATES as u32)
            .unwrap();
        assert_eq!(step.composing.text, "chang");
        assert_eq!(eng.composing_text(), "chang");
        let _ = tao;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn select_bei_keeps_jing_residual() {
        let dir = std::env::temp_dir().join("yc_engine_bei_jing_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(
            &tsv,
            "word\tfreq\tpinyin\n北\t90000\tbei\n京\t80000\tjing\n北京\t95000\tbeijing\n",
        )
        .unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng =
            DataDrivenEngine::new("zh-test".into(), table_scheme(&["bei", "jing"]));
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"beijing" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let pool = eng.cand_pool_clone();
        let bei = pool
            .iter()
            .find(|c| c.text == "北" && c.code_len == 3)
            .expect("北");
        let global = pool.iter().position(|c| c.text == "北" && c.code_len == 3).unwrap() as u32;
        eng.cand_page = global / MAX_CANDIDATES as u32;
        let step = eng
            .select(EditorId(1), global % MAX_CANDIDATES as u32)
            .unwrap();
        assert_eq!(step.composing.text, "jing");
        assert_eq!(eng.composing_text(), "jing");
        let _ = bei;
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fuzzy_zongguo_select_clears_composing() {
        let dir = std::env::temp_dir().join("yc_engine_zongguo_dir");
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("words.tsv");
        let dat = dir.join("lexicon.dat");
        std::fs::write(&tsv, "word\tfreq\tpinyin\n中国\t90000\tzhongguo\n").unwrap();
        let bytes = compile_tsv_to_dat(&tsv).unwrap();
        std::fs::write(&dat, &bytes).unwrap();

        let mut eng = DataDrivenEngine::new(
            "zh-test".into(),
            table_scheme(&["zhong", "guo", "zong"]),
        );
        eng.load_lexicon("zh-test", dat.to_str().unwrap()).unwrap();
        eng.reset(EditorId(1));
        let mode = InputMode::default();
        for ch in b"zongguo" {
            let _ = eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let _ = eng.flush_async_lookup(200);
        let pool = eng.cand_pool_clone();
        let hit = pool.iter().find(|c| c.text == "中国").expect("中国 via fuzzy");
        assert_eq!(hit.code_len, 7);
        let global = pool.iter().position(|c| c.text == "中国").unwrap() as u32;
        eng.cand_page = global / MAX_CANDIDATES as u32;
        let step = eng
            .select(EditorId(1), global % MAX_CANDIDATES as u32)
            .unwrap();
        assert!(step.composing.text.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
