# ContentPack（行业 / 话术）

第三类可运营内容包，与语言包 `.imepack`、皮肤 `.imeskin` 并列。

话术 LLM / 自定义场景 / 知识库见 [PHRASE_LLM_DESIGN.md](../docs/PHRASE_LLM_DESIGN.md)。

## 目录

| 文件 | 说明 |
|------|------|
| `content.toml` | id、version、kind、langs、privacy、industry_id |
| `lexicon/words.tsv` | 行业词 |
| `phrases/deck.json` | 话术卡 deck |
| `prompts/system.txt` | 行业 LLM 场景说明（可选） |

## 样例

- `industry-ecommerce-v1` — 电商客服
- `industry-gaming-v1` — 游戏黑话

启用：发现「行业包」或「设置 → 行业话术与知识库」。
