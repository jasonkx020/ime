# yc-core

跨平台输入法 Rust 核心（M0–M2）。

## Crates

| Crate | 职责 |
|-------|------|
| `yc-types` | 领域类型 + `YcHotAction` 等 C ABI 结构 |
| `yc-lexicon` | 内存词表（M1 占位） |
| `yc-engine` | 最小全拼引擎（M2：ASCII / 数字直出） |
| `yc-session` | SessionManager + Scheduler（M2：布局/方案切换、EditorInfo 强制） |
| `yc-ffi` | C ABI 导出、`HotArena`、`arena_read` |
| `yc-cli` | 桌面 REPL Demo 壳（经 C ABI 端到端演示） |

## 构建

```bash
cargo test --workspace
cargo build -p yc-ffi --release
cargo run -p yc-cli
```

## yc-cli（M2 Demo）

交互式 REPL，经真实 `yc-ffi` C ABI 演示热路径：

```text
yc-cli> nihao
组字: nihao
候选: 1.你好 2.你好吗 ...
yc-cli> /1
yc-cli> /layout qwerty
已切换布局: qwerty
yc-cli> /field password
已切换输入框: password (input_type=0x80)
yc-cli> /quit
```

命令：`/<n>` 选词、`/layout`、`/scheme`、`/ascii`、`/field`、`/help`、`/quit`。

## FFI

头文件：[`include/yc_hot.h`](include/yc_hot.h)

主要入口：

- `yc_core_init` / `yc_core_shutdown`
- `yc_session_begin` / `yc_session_begin_with_input` / `yc_session_validate` / `yc_session_stop`
- `yc_hot_submit` / `yc_hot_arena_ptr`
- M2 action：`SwitchLayout`(4)、`SwitchScheme`(5)、`ToggleAscii`(6)

命名规范见 [docs/SOURCE_NAMING_CONVENTIONS.md](../docs/SOURCE_NAMING_CONVENTIONS.md)。
