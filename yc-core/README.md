# yc-core

跨平台输入法 Rust 核心（M0–M3.5）。

## Crates

| Crate | 职责 |
|-------|------|
| `yc-types` | 领域类型 + `YcHotAction` 等 C ABI 结构 |
| `yc-lexicon` | YCLX v2 mmap 词库 + TSV 编译 |
| `yc-pack` | LangPack / SkinPack ZIP 构建与验签 |
| `yc-theme` | `ThemeRuntime` + `ThemeTokens`（M3） |
| `yc-engine` | `DataDrivenEngine` + `LatinPredictEngine` + `pinyin_seg` |
| `yc-handwriting` | 手写 + 连写云确认 stub（M3） |
| `yc-session` | SessionManager + Scheduler |
| `yc-plugin` | `PluginHost` 本地 install/enable（M3.5） |
| `yc-data` | 冷路径队列 + Repository（M3/M3.5） |
| `yc-ffi` | C ABI、`HotArena`、冷路径回调 |
| `yc-cli` | 桌面 REPL（M3/M3.5 验收） |
| `yc-intel` / `yc-ai` / `yc-ext` | 后续里程碑 stub |

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

```bash
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- compile-lexicon \
  ../fixtures/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv \
  -o /tmp/zh_words.dat
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- build \
  -o ../fixtures/dist/vi-v1.imepack ../fixtures/langpacks/vi-v1
```
cargo run --manifest-path ../tools/ime-pack/Cargo.toml -- build-skin \
  -o ../fixtures/dist/samsung-light.imeskin ../fixtures/skins/samsung-light
```

可选：安装 [flatc](https://github.com/google/flatbuffers) 用于 IDL 代码生成（当前 MVP 不强制）。

## yc-cli（M3/M3.5）

见 [docs/M3_SMOKE.md](../docs/M3_SMOKE.md)。主要新增命令：

- `/skin apply <path>` — 冷路径换肤
- `/install_lang` / `/enable_lang` / `/switch_lang` / `/list_langs`
- `/pinyin` / `/zh` — 安装并 enable `zh-pack-v1` 后切拼音布局
- `/hw continuous` + `/confirm_cloud` / `/dismiss_cloud`

## FFI

头文件：[`include/yc_hot.h`](include/yc_hot.h)

冷路径（需 `--features data` 或 `full`）：

- `yc_cold_submit` / `yc_cold_cancel` / `yc_cold_set_callback`
- `yc_core_sync_lang_packs` — enable 语言包后同步至 Scheduler
- `YC_CMD_APPLY_THEME` — Arena 换肤命令
- `YC_ACTION_CONFIRM_CLOUD_HW`(12) / `DISMISS_CLOUD_HW`(13) / `SWITCH_LANG`(14)

命名规范见 [docs/SOURCE_NAMING_CONVENTIONS.md](../docs/SOURCE_NAMING_CONVENTIONS.md)。
