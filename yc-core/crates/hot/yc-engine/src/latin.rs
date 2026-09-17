//! Latin / script predict engine for OTA langpacks (vi/th/id/ms).
//! - Vietnamese: Unicode composing + tone-stripped romanized lexicon lookup
//! - Thai: Unicode composing + Thai-script prefix lexicon lookup

use std::sync::Arc;

use parking_lot::Mutex;
use yc_lexicon::{normalize_lookup_key, LexiconManager, UserWordStore};
use yc_types::{
    Candidate, ComposingText, EditorId, EngineError, EngineStep, HotResult, InputMode, UiCommand,
    MAX_CANDIDATES,
};

use crate::data_driven::{page_count, page_slice};
use crate::{invalid_session, session_invalid, InputEngine};

#[derive(Debug)]
pub struct LatinPredictEngine {
    active: EditorId,
    composing: String,
    lexicon: LexiconManager,
    pack_id: String,
    cand_pool: Vec<Candidate>,
    cand_page: u32,
    last_query_key: String,
}

impl LatinPredictEngine {
    pub fn new(pack_id: String) -> Self {
        Self {
            active: EditorId::NONE,
            composing: String::new(),
            lexicon: LexiconManager::new(),
            pack_id,
            cand_pool: Vec::new(),
            cand_page: 0,
            last_query_key: String::new(),
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
    }

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

    fn refresh_candidates(&mut self) {
        let key = normalize_lookup_key(&self.composing);
        self.last_query_key = key.clone();
        self.cand_pool = if key.is_empty() {
            Vec::new()
        } else {
            self.lexicon.lookup(&key)
        };
        self.cand_page = 0;
    }

    fn step_from_pool(&mut self, composing: String) -> EngineStep {
        EngineStep {
            composing: ComposingText {
                text: composing.clone(),
                cursor: composing.len() as u32,
            },
            candidates: page_slice(&self.cand_pool, self.cand_page),
            commands: Vec::new(),
        }
    }

    /// Replace composing (e.g. after shell tone apply) and re-query lexicon.
    pub fn set_composing(&mut self, editor_id: EditorId, text: String) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        self.composing = text;
        self.refresh_candidates();
        Ok(self.step_from_pool(self.composing.clone()))
    }

    pub fn page_next(&mut self, editor_id: EditorId) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let pages = page_count(&self.cand_pool);
        if pages == 0 {
            return Err(EngineError::Unsupported);
        }
        if self.cand_page + 1 < pages {
            self.cand_page += 1;
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
            candidates: page_slice(&self.cand_pool, self.cand_page),
            commands: Vec::new(),
        }
    }
}

fn feed_char(key_code: u32) -> Option<char> {
    if key_code == b' ' as u32 {
        return None;
    }
    // ASCII letters (legacy)
    if (b'a'..=b'z').contains(&(key_code as u8)) {
        return Some(key_code as u8 as char);
    }
    if (b'A'..=b'Z').contains(&(key_code as u8)) {
        return Some((key_code as u8).to_ascii_lowercase() as char);
    }
    // Full Unicode scalar (Vietnamese ăâêôơưđ and digits/punct if needed)
    char::from_u32(key_code).filter(|c| !c.is_control() && !c.is_whitespace())
}

impl InputEngine for LatinPredictEngine {
    fn reset(&mut self, editor_id: EditorId) {
        self.active = editor_id;
        self.composing.clear();
        self.cand_pool.clear();
        self.cand_page = 0;
        self.last_query_key.clear();
    }

    fn feed(
        &mut self,
        editor_id: EditorId,
        key_code: u32,
        _input_mode: &InputMode,
    ) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        if key_code == b' ' as u32 {
            // Prefer first candidate; else commit composing; empty → commit space
            if let Some(first) = self.cand_pool.first().cloned() {
                let text = first.text;
                self.composing.clear();
                self.cand_pool.clear();
                self.cand_page = 0;
                self.last_query_key.clear();
                return Ok(EngineStep {
                    composing: ComposingText::empty(),
                    candidates: Vec::new(),
                    commands: vec![UiCommand::Commit { text }],
                });
            }
            if !self.composing.is_empty() {
                let text = std::mem::take(&mut self.composing);
                self.cand_pool.clear();
                self.cand_page = 0;
                self.last_query_key.clear();
                return Ok(EngineStep {
                    composing: ComposingText::empty(),
                    candidates: Vec::new(),
                    commands: vec![UiCommand::Commit { text }],
                });
            }
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::Commit {
                    text: " ".into(),
                }],
            });
        }
        let ch = feed_char(key_code).ok_or(EngineError::Unsupported)?;
        self.composing.push(ch);
        self.refresh_candidates();
        Ok(self.step_from_pool(self.composing.clone()))
    }

    fn select(&mut self, editor_id: EditorId, candidate_id: u32) -> HotResult<EngineStep> {
        if invalid_session(editor_id, self.active) {
            return session_invalid();
        }
        let global = self.cand_page * MAX_CANDIDATES as u32 + candidate_id;
        let text = self
            .cand_pool
            .get(global as usize)
            .map(|c| c.text.clone())
            .or_else(|| {
                page_slice(&self.cand_pool, self.cand_page)
                    .into_iter()
                    .find(|c| c.id == candidate_id)
                    .map(|c| c.text)
            })
            .ok_or(EngineError::Unsupported)?;
        self.composing.clear();
        self.cand_pool.clear();
        self.cand_page = 0;
        self.last_query_key.clear();
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
            return Ok(EngineStep {
                composing: ComposingText::empty(),
                candidates: Vec::new(),
                commands: vec![UiCommand::DeleteSurrounding {
                    before: 1,
                    after: 0,
                }],
            });
        }
        self.composing.pop();
        self.refresh_candidates();
        Ok(self.step_from_pool(self.composing.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use yc_lexicon::{compile_tsv_to_dat, romanize_latin};
    use yc_types::InputMode;

    #[test]
    fn romanize_strips_vi_tones() {
        assert_eq!(romanize_latin("xin chào"), "xinchao");
        assert_eq!(romanize_latin("Việt"), "viet");
        assert_eq!(romanize_latin("đường"), "duong");
        assert_eq!(romanize_latin("ăâêôơư"), "aaeoou");
    }

    fn engine_with_vi_fixture() -> LatinPredictEngine {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "yc-vi-latin-{}-{}",
            std::process::id(),
            stamp
        ));
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("vi.tsv");
        let dat = dir.join("vi.dat");
        {
            let mut f = std::fs::File::create(&tsv).expect("tsv");
            writeln!(f, "word\tfreq\tromanized").unwrap();
            writeln!(f, "xin chào\t9000\txinchao").unwrap();
            writeln!(f, "xin\t8000\txin").unwrap();
            writeln!(f, "đường\t7000\tduong").unwrap();
            writeln!(f, "học\t6000\thoc").unwrap();
        }
        let bytes = compile_tsv_to_dat(&tsv).expect("compile");
        std::fs::write(&dat, bytes).expect("write dat");
        let mut eng = LatinPredictEngine::new("vi-test".into());
        eng.load_lexicon("vi-test", dat.to_str().unwrap())
            .expect("load");
        eng.reset(EditorId(1));
        eng
    }

    #[test]
    fn feed_ascii_prefix_finds_xin_chao() {
        let mut eng = engine_with_vi_fixture();
        let mode = InputMode::default();
        for ch in b"xin" {
            eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let step = eng.current_paged_step();
        assert_eq!(step.composing.text, "xin");
        assert!(
            step.candidates.iter().any(|c| c.text == "xin chào"),
            "expected xin chào in {:?}",
            step.candidates
        );
    }

    #[test]
    fn feed_unicode_special_letters_and_lookup() {
        let mut eng = engine_with_vi_fixture();
        let mode = InputMode::default();
        eng.feed(EditorId(1), 'đ' as u32, &mode).unwrap();
        for ch in "ường".chars() {
            eng.feed(EditorId(1), ch as u32, &mode).unwrap();
        }
        let step = eng.current_paged_step();
        assert_eq!(step.composing.text, "đường");
        assert_eq!(eng.last_query_key(), "duong");
        assert!(step.candidates.iter().any(|c| c.text == "đường"));
    }

    #[test]
    fn space_commits_first_candidate() {
        let mut eng = engine_with_vi_fixture();
        let mode = InputMode::default();
        for ch in b"xin" {
            eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let step = eng.feed(EditorId(1), b' ' as u32, &mode).unwrap();
        assert!(step.composing.text.is_empty());
        assert!(matches!(
            step.commands.first(),
            Some(UiCommand::Commit { text }) if text == "xin chào" || text == "xin"
        ));
    }

    #[test]
    fn backspace_shortens_and_requeries() {
        let mut eng = engine_with_vi_fixture();
        let mode = InputMode::default();
        for ch in b"xin" {
            eng.feed(EditorId(1), *ch as u32, &mode).unwrap();
        }
        let step = eng.backspace(EditorId(1)).unwrap();
        assert_eq!(step.composing.text, "xi");
        eng.backspace(EditorId(1)).unwrap();
        let empty = eng.backspace(EditorId(1)).unwrap();
        assert!(empty.composing.text.is_empty());
        assert!(empty.candidates.is_empty());
    }

    #[test]
    fn set_composing_after_tone_resync() {
        let mut eng = engine_with_vi_fixture();
        let step = eng.set_composing(EditorId(1), "học".into()).unwrap();
        assert_eq!(step.composing.text, "học");
        assert_eq!(eng.last_query_key(), "hoc");
        assert!(step.candidates.iter().any(|c| c.text == "học"));
    }

    fn engine_with_th_fixture() -> LatinPredictEngine {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "yc-th-latin-{}-{}",
            std::process::id(),
            stamp
        ));
        let _ = std::fs::create_dir_all(&dir);
        let tsv = dir.join("th.tsv");
        let dat = dir.join("th.dat");
        {
            let mut f = std::fs::File::create(&tsv).expect("tsv");
            writeln!(f, "word\tfreq\tromanized").unwrap();
            // key column = Thai script (same as word) for Kedmanee prefix lookup
            writeln!(f, "สวัสดี\t9000\tสวัสดี").unwrap();
            writeln!(f, "ขอบคุณ\t8000\tขอบคุณ").unwrap();
            writeln!(f, "ไทย\t7000\tไทย").unwrap();
        }
        let bytes = compile_tsv_to_dat(&tsv).expect("compile");
        std::fs::write(&dat, bytes).expect("write dat");
        let mut eng = LatinPredictEngine::new("th-test".into());
        eng.load_lexicon("th-test", dat.to_str().unwrap())
            .expect("load");
        eng.reset(EditorId(1));
        eng
    }

    #[test]
    fn feed_thai_prefix_finds_sawatdee() {
        let mut eng = engine_with_th_fixture();
        let mode = InputMode::default();
        for ch in "สวัส".chars() {
            eng.feed(EditorId(1), ch as u32, &mode).unwrap();
        }
        let step = eng.current_paged_step();
        assert_eq!(step.composing.text, "สวัส");
        assert_eq!(eng.last_query_key(), "สวัส");
        assert!(
            step.candidates.iter().any(|c| c.text == "สวัสดี"),
            "expected สวัสดี in {:?}",
            step.candidates
        );
    }

    #[test]
    fn thai_space_commits_first_candidate() {
        let mut eng = engine_with_th_fixture();
        let mode = InputMode::default();
        for ch in "สวัส".chars() {
            eng.feed(EditorId(1), ch as u32, &mode).unwrap();
        }
        let step = eng.feed(EditorId(1), b' ' as u32, &mode).unwrap();
        assert!(step.composing.text.is_empty());
        assert!(matches!(
            step.commands.first(),
            Some(UiCommand::Commit { text }) if text == "สวัสดี"
        ));
    }
}
