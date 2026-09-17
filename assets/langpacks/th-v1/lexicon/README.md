# th-v1 词库

## 文件

| 文件 | 说明 |
|------|------|
| `th_words.tsv` | 泰语正字 + 词频 + 查词键（键=泰文本身，提交进 Git） |

列格式：`word \t freq \t romanized`（第三列对泰语为 **泰文前缀键**，非拉丁转写）

## 生成

```powershell
python .\scripts\build-th-lexicon.py
```

然后重建语言包：

```powershell
cargo run --manifest-path .\tools\ime-pack\Cargo.toml -- build `
  -o .\assets\dist\th-v1.imepack .\assets\langpacks\th-v1
Copy-Item .\assets\dist\th-v1.imepack* .\assets\langpacks\dist\ -Force
Copy-Item .\assets\dist\th-v1.imepack* .\platforms\yc-shell-android\app\src\main\assets\langpacks\ -Force
```

## 数据来源

- **[PyThaiNLP TTC](https://github.com/PyThaiNLP/pythainlp)** — `ttc_freq.txt`（Thai Textbook Corpus 词频，**CC0**）
- 原始缓存：`assets/cache/th/ttc_freq.txt`（gitignore）
- `scripts/build-th-lexicon.py` 对问候/输入法常用词做了频次抬升（如 `สวัสดี`）

输入模型：Kedmanee 键面 → 泰文 composing → 泰文前缀查词 → 空格确认首选。
