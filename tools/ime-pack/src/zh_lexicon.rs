use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Common-char floor: rank 0 → BASE, lower ranks still ≫ rare.
const FREQ_COMMON_BASE: u32 = 300_000;
/// Unlisted CJK single chars / demoted traditional-only forms.
const FREQ_RARE: u32 = 2_000;
/// Keep traditional below its simplified twin by at least this gap.
const SIMP_DELTA: u32 = 50_000;

#[derive(Debug, Clone)]
struct LexEntry {
    word: String,
    freq: u32,
    key: String,
}

pub struct BuildZhLexiconOptions {
    pub phrase_pinyin: PathBuf,
    pub char_pinyin: PathBuf,
    pub sample_tsv: PathBuf,
    pub thuocl_dir: Option<PathBuf>,
    pub output_tsv: PathBuf,
    pub core_tsv: Option<PathBuf>,
    pub core_limit: usize,
}

pub fn build_zh_lexicon(opts: &BuildZhLexiconOptions) -> Result<usize, String> {
    let mut char_map: HashMap<char, String> = HashMap::new();
    load_char_pinyin(&opts.char_pinyin, &mut char_map)?;

    let mut dedup: HashMap<(String, String), u32> = HashMap::new();

    let phrase_count = load_phrase_pinyin(&opts.phrase_pinyin, &mut dedup)?;
    let char_count = load_single_chars(&opts.char_pinyin, &mut dedup)?;

    if let Some(dir) = &opts.thuocl_dir {
        if dir.is_dir() {
            load_thuocl_dir(dir, &char_map, &mut dedup)?;
        }
    }

    if opts.sample_tsv.is_file() {
        load_sample_tsv(&opts.sample_tsv, &mut dedup)?;
    }

    let mut entries: Vec<LexEntry> = dedup
        .into_iter()
        .map(|((key, word), freq)| LexEntry { key, word, freq })
        .collect();
    apply_simplified_preference(&mut entries);
    entries.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.key.cmp(&b.key)).then(a.word.cmp(&b.word)));

    write_tsv(&opts.output_tsv, &entries)?;

    if let Some(core_path) = &opts.core_tsv {
        let limit = opts.core_limit.max(1);
        let mut core_map: HashMap<(String, String), u32> = HashMap::new();
        for e in entries.iter().take(limit) {
            merge_entry(
                &mut core_map,
                e.key.clone(),
                e.word.clone(),
                e.freq,
            );
        }
        if opts.sample_tsv.is_file() {
            load_sample_tsv(&opts.sample_tsv, &mut core_map)?;
        }
        let mut core: Vec<LexEntry> = core_map
            .into_iter()
            .map(|((key, word), freq)| LexEntry { key, word, freq })
            .collect();
        apply_simplified_preference(&mut core);
        core.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.key.cmp(&b.key)).then(a.word.cmp(&b.word)));
        write_tsv(core_path, &core)?;
    }

    println!(
        "zh lexicon: {} entries (phrases={}, chars={}, from {})",
        entries.len(),
        phrase_count,
        char_count,
        opts.output_tsv.display()
    );
    Ok(entries.len())
}

fn load_char_pinyin(path: &Path, out: &mut HashMap<char, String>) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((_, rest)) = line.split_once(':') else {
            continue;
        };
        let (pinyin_part, word_part) = if let Some((p, w)) = rest.split_once('#') {
            (p.trim(), w.trim())
        } else {
            continue;
        };
        let word = word_part.trim();
        if word.chars().count() != 1 {
            continue;
        }
        let ch = word.chars().next().unwrap();
        if !is_cjk(ch) {
            continue;
        }
        // Prefer first reading for THUOCL word→pinyin composition.
        let py = normalize_pinyin(pinyin_part.split(',').next().unwrap_or(pinyin_part));
        if py.is_empty() {
            continue;
        }
        out.entry(ch).or_insert(py);
    }
    Ok(())
}

fn load_phrase_pinyin(path: &Path, dedup: &mut HashMap<(String, String), u32>) -> Result<usize, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    let total = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((word, pinyin)) = line.split_once(':') else {
            continue;
        };
        let word = word.trim();
        let key = normalize_pinyin(pinyin);
        if word.is_empty() || key.is_empty() {
            continue;
        }
        let freq = (total.saturating_sub(i) as u32).max(100) + 10_000;
        merge_entry(dedup, key, word.to_string(), freq);
        count += 1;
    }
    Ok(count)
}

fn load_single_chars(path: &Path, dedup: &mut HashMap<(String, String), u32>) -> Result<usize, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((_, rest)) = line.split_once(':') else {
            continue;
        };
        let (pinyin_part, word_part) = if let Some((p, w)) = rest.split_once('#') {
            (p.trim(), w.trim())
        } else {
            continue;
        };
        let word = word_part.trim();
        if word.chars().count() != 1 {
            continue;
        }
        let ch = word.chars().next().unwrap();
        if !is_cjk(ch) {
            continue;
        }
        // All comma-separated readings (polyphones), e.g. 行 → xing,hang.
        let readings: Vec<String> = pinyin_part
            .split(',')
            .map(|p| normalize_pinyin(p))
            .filter(|k| !k.is_empty())
            .collect();
        if readings.is_empty() {
            continue;
        }
        let freq = single_char_freq(ch);
        let common = common_rank(ch).is_some();
        for (i, key) in readings.into_iter().enumerate() {
            // Slightly prefer the first (usually primary) reading.
            let mut f = freq.saturating_sub((i as u32).saturating_mul(50));
            if common {
                // Never drop a listed common char into the rare band.
                f = f.max(FREQ_COMMON_BASE.saturating_sub(200_000));
            }
            merge_entry(dedup, key, word.to_string(), f.max(1));
            count += 1;
        }
    }
    Ok(count)
}

fn load_thuocl_dir(dir: &Path, char_map: &HashMap<char, String>, dedup: &mut HashMap<(String, String), u32>) -> Result<(), String> {
    let mut total = 0usize;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("THUOCL_") {
            continue;
        }
        total += load_thuocl_file(&path, char_map, dedup)?;
    }
    println!("THUOCL: merged {total} words from {}", dir.display());
    Ok(())
}

fn load_thuocl_file(path: &Path, char_map: &HashMap<char, String>, dedup: &mut HashMap<(String, String), u32>) -> Result<usize, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let word = parts[0].trim();
        let df: u32 = parts[1].trim().parse().unwrap_or(1);
        if word.is_empty() || !word.chars().all(is_cjk) {
            continue;
        }
        let key = word_to_pinyin(word, char_map);
        if key.is_empty() {
            continue;
        }
        let freq = df.min(500_000) + 1_000;
        merge_entry(dedup, key, word.to_string(), freq);
        count += 1;
    }
    Ok(count)
}

fn load_sample_tsv(path: &Path, dedup: &mut HashMap<(String, String), u32>) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || (i == 0 && line.starts_with("word")) {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let word = parts[0].trim();
        let freq: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(5000);
        let key = parts
            .get(2)
            .map(|s| yc_lexicon::normalize_romanized(s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| yc_lexicon::normalize_romanized(word));
        if word.is_empty() || key.is_empty() {
            continue;
        }
        merge_entry(dedup, key, word.to_string(), freq.max(50_000));
    }
    Ok(())
}

fn word_to_pinyin(word: &str, char_map: &HashMap<char, String>) -> String {
    let mut out = String::new();
    for ch in word.chars() {
        if let Some(py) = char_map.get(&ch) {
            out.push_str(py);
        } else {
            return String::new();
        }
    }
    out
}

fn merge_entry(dedup: &mut HashMap<(String, String), u32>, key: String, word: String, freq: u32) {
    dedup
        .entry((key, word))
        .and_modify(|f| *f = (*f).max(freq))
        .or_insert(freq);
}

fn write_tsv(path: &Path, entries: &[LexEntry]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push("word\tfreq\tpinyin".to_string());
    for e in entries {
        lines.push(format!("{}\t{}\t{}", e.word, e.freq, e.key));
    }
    fs::write(path, lines.join("\n")).map_err(|e| e.to_string())
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32, 0x4E00..=0x9FFF)
}

fn common_rank_table() -> &'static HashMap<char, u32> {
    static TABLE: OnceLock<HashMap<char, u32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        let mut rank = 0u32;
        for line in include_str!("../data/common_chars.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(ch) = line.chars().next() {
                m.entry(ch).or_insert(rank);
                rank = rank.saturating_add(1);
            }
        }
        m
    })
}

fn common_rank(ch: char) -> Option<u32> {
    common_rank_table().get(&ch).copied()
}

fn single_char_freq(ch: char) -> u32 {
    match common_rank(ch) {
        Some(rank) => FREQ_COMMON_BASE.saturating_sub(rank),
        None => FREQ_RARE,
    }
}

fn t2s_table() -> &'static HashMap<char, char> {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        for line in include_str!("../data/t2s_map.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split('\t');
            let Some(trad) = parts.next().and_then(|s| s.chars().next()) else {
                continue;
            };
            let Some(simp) = parts.next().and_then(|s| s.chars().next()) else {
                continue;
            };
            if trad != simp {
                m.insert(trad, simp);
            }
        }
        m
    })
}

fn to_simplified(word: &str) -> String {
    let map = t2s_table();
    word.chars()
        .map(|ch| map.get(&ch).copied().unwrap_or(ch))
        .collect()
}

/// Demote traditional forms so simplified candidates sort first under the same key.
fn apply_simplified_preference(entries: &mut [LexEntry]) {
    let lookup: HashMap<(String, String), u32> = entries
        .iter()
        .map(|e| ((e.key.clone(), e.word.clone()), e.freq))
        .collect();

    for e in entries.iter_mut() {
        let simp = to_simplified(&e.word);
        if simp == e.word {
            continue;
        }
        if let Some(&simp_freq) = lookup.get(&(e.key.clone(), simp)) {
            e.freq = e.freq.min(simp_freq.saturating_sub(SIMP_DELTA));
        } else {
            // No simplified twin in lexicon: park in rare band.
            e.freq = e.freq.min(FREQ_RARE);
        }
    }
}

pub fn normalize_pinyin(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace() && *c != '\'' && *c != ':')
        .map(strip_tone_char)
        .filter(|c| c.is_ascii_alphabetic())
        .collect()
}

fn strip_tone_char(c: char) -> char {
    match c {
        'a' | 'ā' | 'á' | 'ǎ' | 'à' => 'a',
        'e' | 'ē' | 'é' | 'ě' | 'è' => 'e',
        'i' | 'ī' | 'í' | 'ǐ' | 'ì' => 'i',
        'o' | 'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
        'u' | 'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
        'ü' | 'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'v',
        'n' | 'N' => 'n',
        'r' | 'R' => 'r',
        'm' | 'M' => 'm',
        'A'..='Z' => c.to_ascii_lowercase(),
        'a'..='z' => c,
        _ => '\0',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_tones() {
        assert_eq!(normalize_pinyin("nǐ hǎo"), "nihao");
        assert_eq!(normalize_pinyin("zhōng guó"), "zhongguo");
    }

    #[test]
    fn polyphone_readings_all_inserted() {
        let mut dedup = HashMap::new();
        let dir = std::env::temp_dir().join("yc_polyphone_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("char.txt");
        fs::write(&path, "U+884C:xíng,háng,hàng,héng#行\n").unwrap();
        let n = load_single_chars(&path, &mut dedup).unwrap();
        assert!(n >= 2);
        assert!(dedup.contains_key(&("xing".into(), "行".into())));
        assert!(dedup.contains_key(&("hang".into(), "行".into())));
    }

    #[test]
    fn common_chars_outrank_rare() {
        let de = single_char_freq('的');
        let tao = single_char_freq('桃');
        let rare = single_char_freq('鞉');
        assert!(de > tao, "的 ({de}) should outrank 桃 ({tao})");
        assert!(tao > rare, "桃 ({tao}) should outrank rare 鞉 ({rare})");
        assert!(tao > FREQ_RARE + 10_000);
        assert_eq!(rare, FREQ_RARE);
    }

    #[test]
    fn fa_outranks_traditional_fa() {
        let fa = single_char_freq('发');
        let traditional = single_char_freq('發');
        assert!(fa > traditional, "发 should be listed common; 發 rare or lower");
    }

    #[test]
    fn simplified_preference_demotes_traditional() {
        let mut entries = vec![
            LexEntry {
                key: "fa".into(),
                word: "发".into(),
                freq: 290_000,
            },
            LexEntry {
                key: "fa".into(),
                word: "發".into(),
                freq: 290_000,
            },
            LexEntry {
                key: "fa".into(),
                word: "法".into(),
                freq: 295_000,
            },
        ];
        apply_simplified_preference(&mut entries);
        let fa = entries.iter().find(|e| e.word == "发").unwrap().freq;
        let trad = entries.iter().find(|e| e.word == "發").unwrap().freq;
        assert!(
            trad + SIMP_DELTA <= fa,
            "發 ({trad}) must sit below 发 ({fa}) by >= {SIMP_DELTA}"
        );
    }

    #[test]
    fn to_simplified_maps_common_traditional() {
        assert_eq!(to_simplified("發"), "发");
        assert_eq!(to_simplified("體"), "体");
        assert_eq!(to_simplified("发"), "发");
        assert_eq!(to_simplified("討論"), "讨论");
    }

    #[test]
    fn tao_and_fa_common_simplified_sort_first() {
        let mut entries = vec![
            LexEntry {
                key: "tao".into(),
                word: "桃".into(),
                freq: single_char_freq('桃'),
            },
            LexEntry {
                key: "tao".into(),
                word: "陶".into(),
                freq: single_char_freq('陶'),
            },
            LexEntry {
                key: "tao".into(),
                word: "鞉".into(),
                freq: single_char_freq('鞉'),
            },
            LexEntry {
                key: "tao".into(),
                word: "韜".into(),
                freq: single_char_freq('韜'),
            },
            LexEntry {
                key: "fa".into(),
                word: "发".into(),
                freq: single_char_freq('发'),
            },
            LexEntry {
                key: "fa".into(),
                word: "發".into(),
                freq: single_char_freq('發'),
            },
            LexEntry {
                key: "fa".into(),
                word: "法".into(),
                freq: single_char_freq('法'),
            },
        ];
        apply_simplified_preference(&mut entries);
        entries.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then(b.freq.cmp(&a.freq))
                .then(a.word.cmp(&b.word))
        });

        let tao: Vec<_> = entries.iter().filter(|e| e.key == "tao").collect();
        assert!(tao[0].freq > tao.last().unwrap().freq);
        assert!(
            tao.iter().take(2).all(|e| e.word != "鞉" && e.word != "韜"),
            "rare/traditional must not lead tao: {:?}",
            tao.iter().map(|e| &e.word).collect::<Vec<_>>()
        );
        assert!(tao.iter().any(|e| e.word == "桃" || e.word == "陶"));

        let fa: Vec<_> = entries.iter().filter(|e| e.key == "fa").collect();
        let fa_simp = fa.iter().find(|e| e.word == "发").unwrap().freq;
        let fa_trad = fa.iter().find(|e| e.word == "發").unwrap().freq;
        assert!(fa_simp > fa_trad);
        assert_eq!(fa[0].word, "发");
    }
}
