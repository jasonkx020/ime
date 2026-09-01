# 语言包与资产创作规范（开发者指南）

> 版本：1.0  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 3.5 语言包 OTA、[KEYBOARD_UI_DESIGN.md](KEYBOARD_UI_DESIGN.md) 布局分区  
> 读者：语言包作者、键盘布局设计师、词库维护者、平台工程师

---

## 1. 设计原则

| 原则 | 说明 |
|------|------|
| **人读源格式、机读产物格式** | 开发者日常编辑 YAML / TOML / TSV / JSON；运行时只加载编译后的二进制 |
| **数据驱动优先** | 绝大多数语言（拼音、Telex、泰语、拉丁预测）**无需写代码**，仅配置 + 词库 |
| **热路径零解析** | 词库为 MMAP 只读二进制；布局/方案在 enable 时解析一次，热路径不碰 FlatBuffers |
| **与皮肤解耦** | 语言包定义 **逻辑布局**（键位、输出、动作）；视觉样式来自 **皮肤包** `.imeskin` |
| **可验证、可 diff** | 源文件文本化，CI 跑 `ime-pack validate` + 签名校验 |

---

## 2. 角色与编辑内容

| 角色 | 主要编辑 | 需要掌握 |
|------|----------|----------|
| **语言包维护者** | `pack.toml`、版本、依赖、权限声明 | TOML、semver |
| **输入方案工程师** | `schemes/*.yaml`、转换规则 | YAML、各语言拼写规则 |
| **键盘布局设计师** | `layouts/*.yaml` | YAML、网格与键宽概念 |
| **词库编辑** | `lexicon/*.tsv`、热词 `hotwords.csv` | 表格编辑、UTF-8 |
| **本地化** | `strings/*.json` | JSON、BCP-47 |
| **皮肤设计师** | 独立 `.imeskin`（见 7 节） | 切图 + `skin.toml` |
| **高级引擎扩展（可选）** | Rust `engine` crate → `.so` | Rust、**Android / 鸿蒙** |

**不需要** Kotlin / Swift / JNI 即可制作并发布一个完整语言包。

---

## 3. 工作流总览

```text
  开发者工作区（Git 仓库）                    ime-tools CLI（Rust）
┌─────────────────────────────────┐         ┌──────────────────────────┐
│ pack.toml                       │         │ ime-pack validate        │
│ schemes/vi_telex.yaml           │  build  │ ime-pack build           │
│ layouts/layout_telex.yaml       │ ──────► │ ime-lexicon compile      │
│ lexicon/vi_words.tsv            │         │ ime-layout compile       │
│ strings/vi.json                 │         │ flatc（manifest.fb）     │
│ hotwords.csv（可选）            │         └───────────┬──────────────┘
└─────────────────────────────────┘                     │
                                                        ▼
                                            vi-v1.imepack（ZIP 容器）
                                            ├── manifest.fb
                                            ├── scheme/vi_telex.bin
                                            ├── layouts/layout_telex.bin
                                            ├── lexicon/vi.dat
                                            ├── strings/vi.fb
                                            └── signature

  分发：Catalog CDN → PluginHost.install → enable → mmap / 注册引擎
```

**本地调试**：`ime-pack serve` 将 `.imepack` 挂到本机 Catalog，或 `adb push`（Android）/ 拖入 App Group（iOS）/ 复制到桌面 `data_dir/langpacks/`（Windows `%LOCALAPPDATA%`、macOS `Application Support`、Linux `$XDG_DATA_HOME/ime`）后 enable。

---

## 4. 源格式一览（开发者编辑）

| 资产 | 源格式 | 编码 | 编译产物 | 运行时加载 |
|------|--------|------|----------|------------|
| 包清单 | `pack.toml` | UTF-8 | `manifest.fb`（FlatBuffers） | PluginHost enable |
| 输入方案 | `schemes/<id>.yaml` | UTF-8 | `scheme/<id>.bin` | EngineFactory 注册 |
| 键盘布局 | `layouts/<id>.yaml` | UTF-8 | `layouts/<id>.bin` | UI ReloadKeyboard |
| 词库 | `lexicon/*.tsv` | UTF-8 | `lexicon/*.dat` 或 `*.fst` | Lexicon mmap |
| UI 文案 | `strings/<lang>.json` | UTF-8 | `strings/<lang>.fb` | 冷路径一次解析 |
| 热词 | `hotwords.csv` | UTF-8 | `hotword.delta`（FlatBuffers） | IO 线程 merge |
| 皮肤（独立包） | `skin.toml` + PNG/WebP | UTF-8 / 二进制 | `*.imeskin` | ThemeRuntime |

**明确禁止在源仓库中使用**：Protobuf、运行时 JSON 配置、Excel 二进制 `.xlsx`（可导出 TSV）。

---

## 5. 包清单 `pack.toml`

语言包的 **唯一入口**，描述元数据与文件引用。

```toml
[package]
id = "vi-v1"
version = 1
min_host_version = "1.3.0"
lang = "vi"                    # BCP-47 主标签
display_name = "Tiếng Việt"
engine = "data_driven"         # data_driven | native_so（Android / 鸿蒙）

[[schemes]]
id = "telex"
name = "Telex"
file = "schemes/vi_telex.yaml"
default_layout = "layout_telex"

[[schemes]]
id = "vni"
name = "VNI"
file = "schemes/vi_vni.yaml"
default_layout = "layout_telex"

[lexicon]
file = "lexicon/vi_words.tsv"
format = "dat"                 # dat | fst
key_column = "romanized"       # 查询键列名，见 8 节

[assets]
strings = "strings/vi.json"
hotwords = "hotwords.csv"      # 可选

[permissions]
allow = ["lexicon", "layout"]  # 不含 network、asr
```

构建时 `ime-pack build` 读取 `pack.toml`，生成 `manifest.fb`（字段与附录 C `LangPackManifest` 一致）。

---

## 6. 输入方案 `schemes/*.yaml`

描述 **按键序列 → 文本/音节** 的规则，供 `ime-engine` 数据驱动管线消费。

### 6.1 通用结构

```yaml
scheme_id: telex
lang: vi
normalization: nfc          # nfc | nfd | none

# 音节/词元字母表（用于验证与消歧）
alphabet: "aăâbcdđeêghiklmnopqrstuưvxy"

transform:
  type: rule_chain          # rule_chain | fst_ref | table
  rules: rules/telex.yaml   # 相对路径

segmentation:
  type: lexicon_backed      # 无空格语言（泰语、缅甸语等）
  lexicon_ref: vi_words

candidate:
  sources: [lexicon, user, hot]
  max_syllables: 8
```

### 6.2 规则文件 `rules/telex.yaml`（示例）

```yaml
# Telex 转越南文：序列替换，顺序敏感
- match: "aw"
  output: "ă"
- match: "aa"
  output: "â"
- match: "dd"
  output: "đ"
- match: "s"
  output_tone: acute      # 声调修饰上一音节
  consume: 1
```

### 6.3 中文拼音方案示例

```yaml
scheme_id: pinyin_full
lang: zh
transform:
  type: table
  syllable_separator: "'"
  rules: rules/pinyin_syllables.yaml

candidate:
  sources: [lexicon, user, hot]
  pinyin_mode: full       # full | shuangpin | wubi_ref
```

`rules/pinyin_syllables.yaml` 维护 **合法音节表**（可从开源拼音表生成），词库 TSV 的 `pinyin` 列与之对齐。

### 6.4 编译产物 `scheme/<id>.bin`

定长头 + 规则表二进制（**非** FlatBuffers），enable 时 mmap，热路径直接查表。格式版本由 `ime-scheme` crate 定义，随 `min_host_version` 门禁。

### 6.5 何时需要写代码

| 场景 | 方式 |
|------|------|
| 规则可表化（Telex、全拼、简单替换） | 仅 YAML |
| 大规则集 / 形态分析 | `lexicon/*.fst` 源 + `ime-lexicon compile --fst` |
| 复杂算法且无法表化 | Rust 实现 `InputEnginePlugin`，`engine = native_so`，**Android / 鸿蒙**；iOS 必须退回 `data_driven` |

---

## 7. 键盘布局 `layouts/*.yaml`

定义 **逻辑键位**：标签、输出、宽度、动作；**不含**颜色与切图（由皮肤 `ThemeTokens` 渲染）。

### 7.1 坐标系

- 10 列等分网格（与 [KEYBOARD_UI_DESIGN.md](KEYBOARD_UI_DESIGN.md) 一致）
- `width` 为相对标准键倍数（1.0 = 1 列）
- `row` 自上而下；`KeyView` 负责 dp 换算与边距

### 7.2 示例：`layouts/layout_telex.yaml`

```yaml
layout_id: layout_telex
name: "Telex 越南语"
rows:
  - keys:
      - { label: "Q", output: "q", width: 1.0 }
      - { label: "W", output: "w", width: 1.0 }
      # ... 省略
      - { label: "⌫", action: backspace, width: 1.35, style: utility }
  - keys:
      - { label: "分词", action: separator, width: 1.35, style: utility }
      # ...
  - keys:
      - { label: "123", action: switch_layout, arg: "layout_numeric", width: 1.2, style: utility }
      - { label: "🌐", action: switch_lang, width: 1.2, style: utility }
      - { label: "空格", output: " ", width: 4.2 }
      - { label: "换行", action: enter, width: 1.6, style: accent }

# 可选：长按弹出
long_press:
  - key: "a"
    popup: ["ă", "â", "á", "à", "ả", "ã", "ạ"]
```

### 7.3 `action` 枚举（平台无关）

| action | 说明 |
|--------|------|
| `backspace` | 退格 |
| `enter` | 换行/搜索（由 EditorInfo 决定） |
| `separator` | 分词/音节分隔（中文、泰语等） |
| `switch_layout` | `arg` 为目标 `layout_id` |
| `switch_scheme` | `arg` 为 `scheme_id` |
| `switch_lang` | 打开语言列表 |
| `toggle_ascii` | 中英/大小写等 |
| `space` | 空格（也可用 `output: " "`） |

### 7.4 编译产物 `layouts/<id>.bin`

定长 `LayoutHeader` + `KeySlot[]`（`#[repr(C)]`，与 UI 侧共享头文件）。热路径通过 `layout_id` 索引，不解析 YAML。

### 7.5 与皮肤的关系

| 语言包布局 | 皮肤包 |
|------------|--------|
| 键位、label、output、width | 背景色、按键圆角、字体、按下态、候选栏样式 |
| `style: utility \| accent` 语义标签 | 映射到 `ThemeTokens` 具体颜色 |

同一 `layout_telex` 可搭配多套 `.imeskin` 换肤，无需重打语言包。

### 7.6 手写板布局：`layouts/layout_handwriting_pad.yaml`

手写布局 **无实体字母键**，仅定义顶栏/底栏动作与 InkCanvas 占位（逻辑布局，由 UI 原生实现画布）：

```yaml
layout_id: layout_handwriting_pad
name: "手写输入"
layout_type: handwriting_pad    # 非 key_grid

toolbar:
  - { action: mode_single_char, label: "单字" }
  - { action: mode_continuous, label: "连写" }
  - { action: undo, label: "撤销" }
  - { action: clear, label: "清空" }

canvas:
  id: ink_main
  show_grid: true
  grid_spacing_dp: 24

footer:
  - { action: dismiss_handwriting, label: "返回键盘", width: 1.0 }

# 无 rows[]；编译为 layout_handwriting_pad.bin（HandwritingLayoutHeader）
```

内置中文 LangPack 或 App 默认包应包含此布局；`SwitchScheme(handwriting)` 时加载。

---

## 8. 词库 `lexicon/*.tsv`

### 8.1 源格式（UTF-8 TSV，首行表头）

**中文拼音**

```tsv
word	freq	pinyin
你好	50000	ni'hao
输入法	12000	shu'ru'fa
```

**越南语 / 拉丁语**

```tsv
word	freq	romanized
xin chào	8000	xin chao
```

**泰语（无空格）**

```tsv
word	freq	romanized
สวัสดี	9000	sawatdee
```

| 列 | 必填 | 说明 |
|----|------|------|
| `word` | ✓ | 上屏正文（NFC 规范化） |
| `freq` | ✓ | 词频权重，整数，越大越靠前 |
| `pinyin` / `romanized` / 自定义 | ✓ | 与 `pack.toml [lexicon].key_column` 对应 |

可选列：`pos`（词性）、`user_only`（仅用户词典模板不用）。

### 8.2 编辑工具建议

- 小型词库：VS Code / LibreOffice Calc（**另存为 TSV**）
- 大型词库：Python/pandas 脚本清洗后输出 TSV
- **禁止**直接编辑 `.dat` / `.fst` 二进制

### 8.3 编译：`ime-lexicon compile`

```bash
ime-lexicon compile \
  --input lexicon/vi_words.tsv \
  --key-column romanized \
  --format dat \
  --output build/lexicon/vi.dat
```

### 8.4 产物格式

#### `*.dat`（默认，推荐）

自定义只读词典，针对 IME 查询优化：

```text
Header (64B)
  magic: "IMELEX\0"
  version: u32
  entry_count: u64
  key_index_offset: u64    # 有序 key 索引（romanized / 拼音音节序列）
  payload_offset: u64      # word + freq 紧凑区
  flags: u32                 # case_insensitive 等

KeyIndex[]
  key_hash: u64
  key_bytes_offset / len
  payload_offset / len

Payload
  freq: u32
  word_utf8: [u8]
```

- 构建时排序、去重、规范化
- 运行时 `Lexicon.open_lang` → `memmap2`，热路径二分/哈希查索引

#### `*.fst`（可选，大词库 / 形态复杂）

- 源：OpenFST 文本格式或内部 `*.fst.txt`
- 编译：`ime-lexicon compile --format fst`
- 适用：超大词库、前缀搜索极强的语言；包体积可能更大，构建更慢

### 8.5 热词 `hotwords.csv`

```csv
word,weight,tag
双十一,1000,event
```

编译为 `hotword.delta`（FlatBuffers），由 SyncWorker / IO 线程 merge 进用户词库，**不进热路径 mmap 主库**。

---

## 9. UI 文案 `strings/<lang>.json`

```json
{
  "toolbar.voice": "语音",
  "toolbar.clipboard": "剪贴板",
  "lang.switch_hint": "轻触切换语言",
  "cand.empty": "继续输入以查看候选"
}
```

- 键名：`snake` 或 `dot` 命名，稳定不随翻译变化
- 构建：`ime-pack build` 转为 `strings/<lang>.fb`（FlatBuffers `StringTable`）
- 运行时：enable 时加载；**UI 热路径只读内存 HashMap，不解析 JSON**

---

## 10. 皮肤包（独立资产，非语言包必需）

皮肤与语言包 **分开发布**，避免换肤重下词库。

| 源 | 格式 |
|----|------|
| 元数据 | `skin.toml` |
| 色板 / 字号 | TOML 内联或引用 token |
| 切图 | `assets/keys/*.webp` |
| 编译产物 | `manifest.fb` + 资源文件 → `.imeskin` |

`skin.toml` 可声明 `supported_layouts = ["layout_26_pinyin", "layout_qwerty"]`，与语言包 `layout_id` 对齐。

详见架构文档 **3.2 皮肤更换** 与附录 B。

---

## 11. `.imepack` 容器布局（构建输出）

```text
vi-v1.imepack/                 # ZIP，扩展名 .imepack
  manifest.fb
  scheme/
    telex.bin
    vni.bin
  layouts/
    layout_telex.bin
    layout_numeric.bin
  lexicon/
    vi.dat
  strings/
    vi.fb
  hotword.delta                # 可选
  signature                    # Ed25519，覆盖 manifest 至 assets 的 hash 树
```

Android / 鸿蒙 可选：`engine/libvi_engine.so`（`engine = native_so` 且与宿主同签）。

---

## 12. 工具链 `ime-tools`（规划）

Rust 实现，与宿主 `HOST_VERSION` 对齐，建议独立仓库或 monorepo `tools/`。

| 命令 | 作用 |
|------|------|
| `ime-pack validate` | 校验 TOML/YAML/TSV 语义、layout_id 引用、音节表一致性 |
| `ime-pack build -o vi-v1.imepack` | 全量编译 + 打 ZIP + 签名 |
| `ime-lexicon compile` | TSV → DAT/FST |
| `ime-layout compile` | YAML → layout bin |
| `ime-scheme compile` | YAML → scheme bin |
| `ime-pack inspect <file>` | 查看已打包包内容与版本 |
| `ime-pack sign --key ed25519.pem` | 发布签名 |

**依赖**：`flatc`（FlatBuffers 编译 manifest）、宿主提供的 `ime_scheme_v1.fbs` 等 IDL。

---

## 13. 示例仓库结构

```text
langpack-vi/
  pack.toml
  schemes/
    vi_telex.yaml
    vi_vni.yaml
    rules/
      telex.yaml
      vni.yaml
  layouts/
    layout_telex.yaml
    layout_numeric.yaml
  lexicon/
    vi_words.tsv
  strings/
    vi.json
  hotwords.csv
  scripts/
    normalize_tsv.py          # 可选清洗脚本
  .github/
    workflows/
      build-pack.yml          # CI: validate + build + 上传 artifact
```

---

## 14. 版本、兼容与校验

| 项 | 规则 |
|----|------|
| `pack.version` | 单调递增整数；Catalog 按 `(pack_id, version)` 唯一 |
| `min_host_version` | 与 `ime-tools`、运行时 `HOST_VERSION` semver 比较 |
| `lexicon.dat` `version` | 旧版 runtime 拒绝未知 version |
| `layout.bin` / `scheme.bin` | 头文件含 `magic + version`，与 cbindgen 头一致 |
| CI 必跑 | `validate`、词库条目数下限、签名、包大小预算（建议单包 &lt; 30MB） |

---

## 15. 与运行时模块映射

```text
pack.toml           → LangPackLoader → manifest.fb
schemes/*.yaml      → EngineFactory.register → InputEnginePlugin（数据驱动）
layouts/*.yaml      → UiCommand.ReloadKeyboard → KeyView 网格
lexicon/*.tsv       → Lexicon.open_lang → mmap 热查询
strings/*.json      → 冷路径 StringTable
hotwords.csv        → importHotDelta（IO 线程）
```

**热路径禁止**：作者源 YAML/TSV/JSON、Protobuf、FlatBuffers 解析。仅允许 mmap 词库 + 已编译 layout/scheme bin + 内存中的候选结构。

---

## 16. 快速对照：我该用什么格式？

| 我想… | 编辑 | 语言/工具 |
|--------|------|-----------|
| 新增越南 Telex 语言包 | `pack.toml` + scheme/layout YAML + TSV | 文本编辑器 |
| 改键盘多一行符号 | `layouts/layout_*.yaml` | YAML |
| 加 1 万条词 | `lexicon/*.tsv` | Calc / 脚本导出 TSV |
| 改工具栏文案 | `strings/*.json` | 任意 JSON 编辑器 |
| 换键盘颜色 | 皮肤 `skin.toml` + 切图 | 设计工具 + TOML |
| 复杂泰语消歧算法 | `scheme` + `fst` 或 Rust engine | YAML + 可选 Rust |
| 发布到 CDN | `ime-pack build` + 签名 | CLI |

---

*文档结束*
