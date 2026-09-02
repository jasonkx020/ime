use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    entries.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.key.cmp(&b.key)));

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
        core.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.key.cmp(&b.key)));
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
        let key = normalize_pinyin(pinyin_part.split(',').next().unwrap_or(pinyin_part));
        if key.is_empty() {
            continue;
        }
        merge_entry(dedup, key, word.to_string(), single_char_freq(ch));
        count += 1;
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

fn single_char_freq(ch: char) -> u32 {
    if COMMON_SINGLE_CHARS.contains(&ch) {
        150_000
    } else {
        8_000
    }
}

const COMMON_SINGLE_CHARS: &[char] = &[
    '的', '一', '是', '了', '我', '不', '在', '人', '有', '他', '这', '中', '大', '来', '上', '国',
    '个', '到', '说', '们', '为', '子', '和', '你', '地', '出', '也', '时', '道', '就', '下', '得',
    '可', '以', '生', '会', '自', '着', '去', '之', '过', '家', '学', '对', '能', '多', '然', '于',
    '她', '它', '好', '要', '看', '没', '还', '那', '么', '什', '吗', '呢', '吧', '啊',
];

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
}
