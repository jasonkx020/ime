# M3 + M3.5 + LangPack P0–P3 冒烟验收

M3（皮肤冷路径 + 手写连写云确认）、M3.5（语言包 install/enable + switch_lang）、**P0–P3**（manifest 驱动 Slot、scheme/layout 编译、六端布局热加载）以 **yc-cli** 与 **cargo test** 为主验收面。

## 前置

```powershell
$env:CARGO_HTTP_CHECK_REVOKE = 'false'
.\scripts\build-all.ps1
```

或手动：

```powershell
cd yc-core
cargo test --workspace
cargo build -p yc-ffi --features full --release
```

构建 fixture：`vi-v1`、`th-v1`、`zh-pack-v1` 语言包 + `samsung-light` 皮肤（见 `fixtures/dist/`）。

## M3：皮肤换肤

```text
cargo run -p yc-cli
/skin list
/skin apply samsung-light
```

## 中文拼音（zh-pack-v1，必选）

```text
/install_lang <path/to/zh-pack-v1.imepack>
/enable_lang zh-pack-v1
/pinyin
nihao
/1
```

期望：`/1` 上屏「你好」。无 enable 时 `/layout pinyin26` 返回 Unsupported。

大词库：`scripts/build-zh-lexicon.ps1`（10 万 TSV + YCLX v2 dat，已集成 `build-all.ps1`）。

## M3.5 / P0：语言包（manifest 路径 + Slot）

```text
/install_lang <path/to/th-v1.imepack>
/enable_lang th-v1
/switch_lang th-v1
hello
```

`enable` 后 `yc_core_sync_lang_packs()` 为 **幂等 reconcile**（yc-cli 仍自动调用；非必须手动步骤）。

vi ↔ th 来回 switch 无需改 Rust；词库路径来自 manifest `lexicon.dat_path`。

## P2：Scheme 编译

```powershell
cargo test -p yc-scheme
cargo test -p yc-engine --test p2_schemes
```

- `vi-v1`：`latin` + `telex`（`aw→ă` 规则链）
- `zh-pack-v1`：`pinyin_full`（~410 音节表 + YCLX v2 mmap 词库）

## P3：布局热加载

`switch_lang` / `switch_layout` 产出 `ReloadKeyboard { layout, layout_id }`；Arena `text` 字段携带 `layout_id`（≤64B）。

| 端 | LayoutLoader | ReloadKeyboard |
|----|--------------|----------------|
| Android | `yc-ui-android/.../LayoutLoader.kt` | `YcImeService.refreshUi` |
| iOS | `YcKeyboard/LayoutLoader.swift` | `YcBridge.refreshIfNeeded` |
| 鸿蒙 | `YcArena.ets` + Extension 日志 | `InputMethodExtensionAbility` |
| macOS | `YcArena.swift` | `YcInputServer` |
| Windows/Linux | `yc_layout_loader.hpp` | Arena `Command.text` |

共享头：`yc-core/include/yc_layout.h`（`sync-headers.ps1` 同步六端）。

## Android

`MainActivity` 通用 install：`assets/langpacks/{pack}.imepack`（vi-v1、th-v1、**zh-pack-v1 默认 cold enable**）。

## 相关 crate

| 组件 | 路径 |
|------|------|
| LangPackSlot / Registry | `yc-core/crates/yc-plugin` |
| Scheme 编译/运行时 | `yc-core/crates/yc-scheme` |
| Layout 编译/运行时 | `yc-core/crates/yc-layout` |
| DataDrivenEngine | `yc-core/crates/yc-engine` |
| ime-pack CLI | `tools/ime-pack` |
