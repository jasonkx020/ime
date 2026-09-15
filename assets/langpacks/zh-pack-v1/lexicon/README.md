# zh-pack-v1 词库

## 文件

| 文件 | 说明 |
|------|------|
| `zh_words.sample.tsv` | 最小测试集（~200 条），`cargo test` 离线构建 |
| `zh_words.core.tsv` | 常用词子集（~8000 条），提交进 Git，无网络可构建 imepack |
| `zh_words.tsv` | 完整词库（10 万+ 条，gitignore） |
| `zh_words.dat` | YCLX v2 编译产物（gitignore，构建时生成） |

## 频次策略（常用字 + 简体优先）

编译期写在 `freq` 列里，引擎仍按 freq 降序，无需热路径改动：

1. **常用字分层**：`tools/ime-pack/data/common_chars.txt`（Jun Da 现代汉语字频，约 1 万字）
   - 表内：`freq = 300000 - rank`（越常用越高）
   - 表外单字：`freq = 2000`
2. **简体优先**：`tools/ime-pack/data/t2s_map.txt`（OpenCC TSCharacters 子集）
   - 同拼音下若存在简体形，繁体 `freq = min(原freq, 简体freq - 50000)`
   - 仅有繁体、无简体对照：压到生僻档 `2000`

用户选词习惯（`user_words.tsv`）仍可在运行时压过默认排序。

## 生成完整词库

```powershell
.\scripts\build-zh-lexicon.ps1
```

数据来源（MIT / Apache-2.0）：

- [mozillazg/phrase-pinyin-data](https://github.com/mozillazg/phrase-pinyin-data) — 词语拼音
- [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) — 单字拼音
- [thunlp/THUOCL](https://github.com/thunlp/THUOCL) — 词频补充
- Jun Da 字频（via [hanziDB.csv](https://github.com/ruddfawcett/hanziDB.csv)）— 常用字排序
- [OpenCC](https://github.com/BYVoid/OpenCC) TSCharacters — 繁→简

原始缓存：`assets/cache/pinyin/`、`assets/cache/thuocl/`（gitignore）。

## TSV 格式

```text
word	freq	pinyin
你好	9000	nihao
```

- `pinyin`：无声调全拼，与 `pinyin_syllables.yaml` 对齐
- 同 `(pinyin, word)` 保留最高 freq
