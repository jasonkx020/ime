use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use memmap2::Mmap;
use yc_types::{Candidate, CandidateSource, EngineError, HotResult, MAX_CANDIDATES};

use crate::LangLexiconHandle;

pub const LEXICON_MAGIC: &[u8; 4] = b"YCLX";
pub const LEXICON_VERSION: u32 = 2;

const HEADER_SIZE: usize = 16;

/// Memory-mapped lexicon view (YCLX v2).
#[derive(Debug)]
pub struct DatLexicon {
    data: Arc<[u8]>,
    index_offsets: Vec<u32>,
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
        Ok(Self {
            data,
            index_offsets,
        })
    }

    fn key_count(&self) -> u32 {
        u32::from_le_bytes(self.data[8..12].try_into().unwrap())
    }

    fn payload_offset(&self) -> u32 {
        u32::from_le_bytes(self.data[12..16].try_into().unwrap())
    }

    fn key_at(&self, idx: u32) -> Option<(String, u32, u32)> {
        let off = *self.index_offsets.get(idx as usize)? as usize;
        if off + 2 > self.data.len() {
            return None;
        }
        let key_len = u16::from_le_bytes(self.data[off..off + 2].try_into().unwrap()) as usize;
        let mut off = off + 2;
        if off + key_len + 8 > self.data.len() {
            return None;
        }
        let key = String::from_utf8_lossy(&self.data[off..off + key_len]).into_owned();
        off += key_len;
        let payload_off = u32::from_le_bytes(self.data[off..off + 4].try_into().unwrap());
        let payload_count = u32::from_le_bytes(self.data[off + 4..off + 8].try_into().unwrap());
        Some((key, payload_off, payload_count))
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
        self.lookup_with_key_filter(&composing, |key| {
            crate::pinyin_match::key_matches_composing(key, &composing, syllables)
        })
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
            .take(MAX_CANDIDATES)
            .enumerate()
            .map(|(i, (_freq, text))| Candidate {
                id: i as u32,
                text,
                source: CandidateSource::Lexicon,
                score: 1.0 - (i as f32 * 0.05),
            })
            .collect()
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
        let key = lex.key_at(mid).map(|(k, _, _)| k).unwrap_or_default();
        if key.as_str() < prefix {
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
}

impl LexiconManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_lang(&mut self, pack_id: &str, path: &str) -> HotResult<()> {
        let lex = DatLexicon::open_mmap(Path::new(path)).map_err(|_| EngineError::Internal)?;
        self.next_handle += 1;
        let handle = LangLexiconHandle(self.next_handle);
        self.handles.insert(pack_id.to_string(), handle);
        self.packs.insert(pack_id.to_string(), lex);
        Ok(())
    }

    pub fn close_lang(&mut self, pack_id: &str) -> HotResult<()> {
        self.packs.remove(pack_id);
        self.handles.remove(pack_id);
        if self.active_pack.as_deref() == Some(pack_id) {
            self.active_pack = None;
        }
        Ok(())
    }

    pub fn set_active(&mut self, pack_id: &str) {
        self.active_pack = Some(pack_id.to_string());
    }

    pub fn lookup(&self, prefix: &str) -> Vec<Candidate> {
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                return lex.lookup(prefix);
            }
        }
        Vec::new()
    }

    pub fn lookup_pinyin(&self, composing: &str, syllables: &[String]) -> Vec<Candidate> {
        if let Some(id) = &self.active_pack {
            if let Some(lex) = self.packs.get(id) {
                return lex.lookup_pinyin(composing, syllables);
            }
        }
        Vec::new()
    }
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
            .join("../../../fixtures/langpacks/vi-v1/lexicon/vi_words.tsv");
        if p.exists() {
            return p;
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv")
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
            .join("../../../fixtures/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv");
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
}
