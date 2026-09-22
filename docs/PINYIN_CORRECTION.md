# 拼音纠错设计（传统热路径）

关联：[PINYIN_SEGMENTATION.md](PINYIN_SEGMENTATION.md)、[LANGPACK_AUTHORING.md](LANGPACK_AUTHORING.md)、[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md)。

## 边界

- **热路径无 LLM 填池**：`needs_llm_fallback` 对 Table composing 恒为 false。
- 纠错候选 **永不抢占** 正常（精确/简拼/分段）候选首位（`CORRECTION_SCORE_CAP` + `rank_candidates`）。
- Instant：无纠错。Lean/Expanded：发音模糊 + 邻键错字 + 漏字 edit1。

## 分级

规则 → 个人纠错词典 → n-gram/prefer →（可选）异步 LLM 冷分析。实时只用规则 + 个人词典。

## 三类实时纠错

| 类型 | 例 | 数据 | 代码 |
|------|-----|------|------|
| 发音模糊 | `zongguo`→中国 | LangPack `rules/pinyin_fuzzy.yaml` | `fuzzy_variants_with` |
| 错字邻键 | `nuhao`→你好 | **每 layout** `keyboard_neighbors` | `adjacent_typo_variants_with` |
| 漏字/多字/交换 | `nhao`→你好 | 算法 | `edit1_variants` |

## 语言包

- 发音对：方案级 `[correction] fuzzy = ...`（`pack.toml`）。
- 临近码：布局级 `layouts/*.yaml` → `keyboard_neighbors`；`switch_layout` 换表。
- 缺表：`CorrectionTables::builtin_fallback()`。
- 热路径禁止解析 YAML（enable 时载入内存）。

## 学习

- `UserWordStore`：词频 + 衰减。
- `UserCorrectionStore`：typed → corrected_key。
- 云端聚合 / 质检 / 版本 / 灰度：冷闭环契约，不进热路径。

## 指标（目标）

常见类型召回 >95%；Top-3 准确 >90%；Lean 规则路径预算 <15ms（变体 cap）。
