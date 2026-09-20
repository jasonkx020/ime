# ContentPack（行业 / 话术）

第三类可运营内容包，与语言包 `.imepack`、皮肤 `.imeskin` 并列。

## 目录

| 文件 | 说明 |
|------|------|
| `content.toml` | id、version、kind、langs、privacy |
| `lexicon/words.tsv` | 行业词：`word\\tfreq\\tpinyin` |
| `phrases/deck.json` | 话术卡 deck |

## kind

- `industry_lexicon` — 行业词 + 可选话术
- `phrase_deck` — 仅话术
- `sticker` — 贴纸（后续）
- `skin_addon` — 皮肤附加（后续）

## 样例

- `industry-ecommerce-v1` — 电商客服
- `industry-gaming-v1` — 游戏黑话

启用路径：主 App 发现页 → 写入 `yc_content.active_industry` → 键盘「话术」加载对应 `phrases/deck.json`。
