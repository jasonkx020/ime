# yc-core

跨平台输入法 Rust 核心（M0–M3.5）。Crates 按依赖层物理分目录；**crate 包名不变**。

## 分层布局

```text
crates/
  foundation/   yc-types
  hot/          yc-session, yc-engine, yc-lexicon, yc-scheme, yc-layout, yc-intel
  cold/         yc-data, yc-plugin, yc-pack, yc-theme
  features/     yc-ai, yc-handwriting, yc-ext
  boundary/     yc-ffi          # 唯一 C ABI 边界
  apps/         yc-cli
```

| 层 | Crates | 职责 |
|----|--------|------|
| **foundation** | `yc-types` | 领域类型 + `YcHotAction` 等 C ABI 结构 |
| **hot** | `yc-session` | SessionManager + Scheduler |
| | `yc-engine` | `DataDrivenEngine` + `LatinPredictEngine` + `pinyin_seg` |
| | `yc-lexicon` | YCLX v2 mmap 词库 + TSV 编译 |
| | `yc-scheme` / `yc-layout` | 方案与布局编译/运行时 |
| | `yc-intel` | LightIntel 用户词重排 |
| **cold** | `yc-data` | 冷路径队列 + Repository；AI 经 `ColdAiHandler` 注入 |
| | `yc-plugin` | `PluginHost` install/enable / Catalog OTA |
| | `yc-pack` / `yc-theme` | LangPack·SkinPack 构建验签；ThemeRuntime |
| **features** | `yc-ai` | AiAssistService（隐私门禁 + 本地模板 + Stub 云） |
| | `yc-handwriting` | 手写 + 连写云确认 stub |
| | `yc-ext` | ExtensionHost stub |
| **boundary** | `yc-ffi` | C ABI、`HotArena`、冷路径回调与 handler 注入 |
| **apps** | `yc-cli` | 桌面 REPL（验收） |

### 依赖禁令

- **hot 禁止直接 HTTP**（网络只走 cold / plugin）。
- **`yc-ai` 仅冷路径**：`yc-data` 不硬依赖 `yc-ai`；由 `yc-ffi`（`feature = "ai"`）/ `yc-cli` 注入 `ColdAiHandler`。
- **`yc-ffi` 是唯一跨语言边界**；内部 crate 仅 Rust 调用。

## 构建

```bash
# Windows 若遇 crates.io SSL 问题：
# set CARGO_HTTP_CHECK_REVOKE=false

cargo test --workspace
cargo build -p yc-ffi --features full --release
cargo run -p yc-cli
```

### ime-pack 工具链（M3.5）

FlatBuffers IDL 见仓库根 [`schemas/`](../schemas/)（运行时 manifest 暂用 JSON 字节，名为 `manifest.fb`）。

测试与示例资源在仓库根 [`assets/`](../assets/)（`langpacks/`、`skins/`、`dist/`）。

```bash
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- compile-lexicon \
  ../assets/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv \
  -o /tmp/zh_words.dat
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- build \
  -o ../assets/dist/vi-v1.imepack ../assets/langpacks/vi-v1
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- build-skin \
  -o ../assets/dist/samsung-light.imeskin ../assets/skins/samsung-light
```

可选：安装 [flatc](https://github.com/google/flatbuffers) 用于 IDL 代码生成（当前 MVP 不强制）。

## yc-cli（M3/M3.5）

见 [docs/M3_SMOKE.md](../docs/M3_SMOKE.md)。主要新增命令：

- `/skin apply <path>` — 冷路径换肤
- `/install_lang` / `/enable_lang` / `/switch_lang` / `/list_langs`
- `/pinyin` / `/zh` — 安装并 enable `zh-pack-v1` 后切拼音布局
- `/hw continuous` + `/confirm_cloud` / `/dismiss_cloud`
- `/ai` — 冷路径 AI 润色/助手（需 ffi `ai` feature）
- `/catalog` — Catalog OTA 拉取

## FFI

头文件：[`include/yc_hot.h`](include/yc_hot.h)

冷路径（需 `--features data` 或 `full`）：

- `yc_cold_submit` / `yc_cold_cancel` / `yc_cold_set_callback`
- `yc_core_sync_lang_packs` — enable 语言包后同步至 Scheduler
- `YC_CMD_APPLY_THEME` — Arena 换肤命令
- `YC_ACTION_CONFIRM_CLOUD_HW`(12) / `DISMISS_CLOUD_HW`(13) / `SWITCH_LANG`(14)

命名规范见 [docs/SOURCE_NAMING_CONVENTIONS.md](../docs/SOURCE_NAMING_CONVENTIONS.md)。
