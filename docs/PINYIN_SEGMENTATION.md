# 拼音智能分词方法论（字母 → 词跨度）

本文约定中文拼音输入中：**每个候选必须绑定自己消费的输入字母区间（span）**。区间由拼音结构推出；词库只回答「该跨度上有没有这个词」，不决定「跨度有多长」。缺字、错字只扩展查词假设，**不改写**「消费哪些用户已敲字母」。

实现入口：`yc-lexicon` 的 `span_resolve`（`resolve_code_len` / `attach_spans`），引擎 `select` 按 `Candidate.code_len` 切残码。

---

## 流水线

```text
用户 composing 原串 S
  → 音节可达图 jumps（多假设）
  → 缺字/错字变体层（仅查词）
  → 结构假设 → 边 (text, span_on_S)
  → 候选池 code_len = span
  → select 消费 S[0..code_len)，残码再递归
```

---

## 规则

### R0 — 跨度是一等公民

- `code_len` = 从当前 composing（**用户原串**）起点起消费的字节数。
- 禁止：单字默认写成 `composing.len()` 再事后碰运气收紧。
- 允许：整句组词 `code_len == composing.len()`（明确「覆盖全串」）。

### R1 — 先结构，后词库

1. 音节表建 `syllable_jumps`（每位多终点并存）。
2. 枚举结构假设，得到字节区间。
3. 在该区间键（或其 R6 变体）上查词；命中则 `code_len` 写在原串上。

### R2 — 四类结构假设

| 类型 | 何时 | 键 | code_len（原串） |
|------|------|----|------------------|
| MonoSyl | 一段完整音节 | `S[i..j]` | `j-i` |
| MultiSyl | 连续 2..K 音节 | 拼接 | 覆盖字节数 |
| Initial | 简拼 / 细切声母 | 单字母等 | 通常 1 |
| IntraSyl | 当前音节内前缀（`tao` 下 `t`/`ta`） | 前缀 | 前缀长 |

展示优先级：L0 整键 > L2 组句 > L3 Mono/Multi > L4 Initial/Intra（见 `cand_tiers`）。

歧义（`xian` vs `xi+an`）：保留多条 jumps，靠词频/上下文打分，结构层不定死。

### R3 — 选词与残码

`select` 消费 `[0, code_len)`，残码 `S[code_len..]` 再套用同一方法论（残码上再选「流」只吃 `liu`）。

### R4 — 声母与孤儿韵母

- 声母走 Initial / mixed；不得用裸声母误吞后续韵母（除非候选绑定更长 span）。
- 孤儿韵母 `i/u/v`：无词库命中则不单独成边。

### R5 — 注入 / 用户词 / 噪声同源

AI、用户词、DAT noise 必须经同一 `attach_spans`。推不出：单字→首音节长，多字→全串。禁止无依据的全串单字。

### R6 — 缺字与错字（纠错假设，span 仍在原串）

| 类别 | 含义 | 能力 |
|------|------|------|
| Typo | 邻键误触（`wn`→`wm`） | `typo::adjacent_typo_variants` |
| Fuzzy | 音系混淆（`z/zh`、`an/ang`） | `fuzzy::fuzzy_variants` |
| Incomplete | 少敲（`beijng`、`tal`→`tao`） | 前缀 / IntraSyl / 简拼 |

回映射原则：

1. 可在 `variant(S)` 上查词；展示汉字。
2. `code_len` **永远相对原串 S**。
3. 对齐启发式：
   - **替换型**（同长）：`code_len_original = code_len_on_variant`。
   - **扩初型**（`z`→`zh`）：原串消费到该音节终点（通常比变体短 1）。
   - **缩初型**（`zh`→`z`）：原串含被删 `h` 的终点（通常比变体长 1）。
   - **缺尾/前缀**：`code_len = 用户已输入的该假设前缀长`。
4. 分数：exact > fuzzy > typo；纠错不得压过同 span 精确边。
5. Instant 可关重纠错；Lean/Expanded 开 fuzzy+typo，但收口必须在原串上 `attach_spans`。

---

## 与候选分层

| 层 | 结构来源 | 典型 span |
|----|----------|-----------|
| L0 | 整键 / 精确前缀热词 | 常为全串或最长精确前缀 |
| L1 | AI 词语 | `resolve_code_len` |
| L2 | 组句 / 路径前缀 | 覆盖字节 `end` |
| L3 | MonoSyl / MultiSyl | 音节跨度 |
| L4 | Initial / IntraSyl | 1 或音节内前缀 |

---

## 经典流水线契约（Pre → Seg → Gen → Rank）

对照「预处理 → 音节切分 → 候选生成 → 候选排序 → 输出」：

| 阶段 | 输入 | 输出 | 本仓库入口 |
|------|------|------|------------|
| **Pre** | 原始 composing | key / shape / hard_breaks | `preprocess::preprocess_pinyin` |
| **Seg** | key + 音节表 | 结构假设（jumps / paths / spans） | `seg_hypotheses::build_seg_hypotheses_ex` |
| **Gen** | 假设 + 词库 + 规则纠错 | Candidate（纠错带封顶） | DAT + `cand_gen` / lattice；`correction` |
| **Rank** | 候选 + 上下文 | 最终排序（纠错不抢首位） | `rank::rank_candidates` / `rank_fuse` |
| **Out** | 选中候选 | 上屏 + 残码 | `select` 按 `code_len` |

硬性出口：

1. Gen 结束必须 `attach_spans`（禁止裸全串单字）。
2. Rank 可叠加 ngram/user/prefer，但最终排序走 `rank_candidates`；纠错带永不抢正常首位。
3. Instant / Lean / Expanded 只是 **同一 Gen 的预算开关**（`SegmentMode` + 纠错开关）。
4. **传统热路径无 LLM 填池**；纠错细则见 [PINYIN_CORRECTION.md](PINYIN_CORRECTION.md)。

不动根基：不改 YCLX、`code_len`/`select` 协议、LangPack/FFI。

---

## 反例（禁止）

- 输入 `liuchang`，候选「流」的 `code_len == 8`（整串）。
- Fuzzy 命中「中国」却用理想键 `zhongguo` 长度（8）而用户只敲了 `zongguo`（7）时写错映射（整词应消费**原串全长** 7）。
- 把多音节词注册成音节表里的「假音节」来定死切分。

---

## 相关代码

- `yc-lexicon/src/preprocess.rs` — Pre
- `yc-lexicon/src/seg_hypotheses.rs` — Seg
- `yc-lexicon/src/correction.rs` — 漏字/错字/发音模糊
- `yc-lexicon/src/span_resolve.rs` — `attach_spans`
- `yc-lexicon/src/rank.rs` — Rank
- `yc-lexicon/src/user_words.rs` — 词频衰减 + 纠错词典
- [PINYIN_CORRECTION.md](PINYIN_CORRECTION.md) — 纠错专文
