# zh-pack-v1 词库

## 文件

| 文件 | 说明 |
|------|------|
| `zh_words.sample.tsv` | 最小测试集（~200 条），`cargo test` 离线构建 |
| `zh_words.core.tsv` | 常用词子集（~8000 条），提交进 Git，无网络可构建 imepack |
| `zh_words.tsv` | 完整词库（10 万+ 条，gitignore） |
| `zh_words.dat` | YCLX v2 编译产物（gitignore，构建时生成） |

## 生成完整词库

```powershell
.\scripts\build-zh-lexicon.ps1
```

数据来源（MIT 许可）：

- [mozillazg/phrase-pinyin-data](https://github.com/mozillazg/phrase-pinyin-data) — 词语拼音
- [mozillazg/pinyin-data](https://github.com/mozillazg/pinyin-data) — 单字拼音
- [thunlp/THUOCL](https://github.com/thunlp/THUOCL) — 词频补充

原始缓存：`fixtures/cache/pinyin/`、`fixtures/cache/thuocl/`（gitignore）。

## TSV 格式

```text
word	freq	pinyin
你好	9000	nihao
```

- `pinyin`：无声调全拼，与 `pinyin_syllables.yaml` 对齐
- 同 `(pinyin, word)` 保留最高 freq
