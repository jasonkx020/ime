# vi-v1 词库

## 文件

| 文件 | 说明 |
|------|------|
| `vi_words.tsv` | 标准越语正字 + 词频 + 去调 romanized 查词键（提交进 Git） |

列格式：`word \t freq \t romanized`

## 生成

```powershell
python .\scripts\build-vi-lexicon.py
```

然后重建语言包：

```powershell
cargo run --manifest-path .\tools\ime-pack\Cargo.toml -- build `
  -o .\assets\dist\vi-v1.imepack .\assets\langpacks\vi-v1
Copy-Item .\assets\dist\vi-v1.imepack* .\assets\langpacks\dist\ -Force
Copy-Item .\assets\dist\vi-v1.imepack* .\platforms\yc-shell-android\app\src\main\assets\langpacks\ -Force
```

## 数据来源

- **[tabidots/vn-freqs](https://github.com/tabidots/vn-freqs)** — 约 1.9 万词级频率表（含多音节词）
  - Leipzig Corpora Collection（CC BY）
  - OpenSubtitles2016（CC）
  - 分词：`pyvi`
- 原始缓存：`assets/cache/vi/vn_word_frequencies.tsv`（gitignore）
- `scripts/build-vi-lexicon.py` 内对问候/输入法常用短语做了频次抬升（如 `xin chào`）

未采用 GPL 词表（如 Hồ Ngọc Đức / duyet vietnamese-wordlist），以免语言包分发受限。
