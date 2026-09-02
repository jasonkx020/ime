# 源代码命名规范

> 版本：1.0  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 第 9 章、[RUST_PLATFORM_INTEGRATION.md](RUST_PLATFORM_INTEGRATION.md)  
> 读者：Rust 核心工程师、六端壳工程开发者、语言包与工具链维护者

---

## 1. 目的与适用范围

### 1.1 目的

统一跨平台输入法（IME）实现代码的命名体系，以 **`yc` 作为厂商前缀** 替换遗留 **`ime` 前缀**，确保：

- 六端（Android / iOS / 鸿蒙 / Windows / macOS / Linux）仓库、产物、FFI 符号 **可一眼识别归属**；
- `yc-ffi` 保持 **唯一跨语言边界**，避免多套 FFI 命名并存；
- 设计文档与实现代码在落地阶段可 **按附录对照表** 有序迁移。

### 1.2 适用范围

| 适用 | 不适用 |
|------|--------|
| 新建 `yc-core` workspace 及全部 crate | 操作系统 / 框架 API（`InputMethodService`、`ITfContext` 等） |
| 六端壳工程（`yc-shell-*`）与 UI 模块（`yc-ui-*`） | 纯领域概念且不带产品前缀的类型（`EditorId`、`Scheduler`） |
| C ABI 头文件、导出函数、错误码宏 | 第三方库及其生成代码命名空间 |
| CLI 工具（`yc-pack` 等）与产物扩展名（`.ycpack`） | 本规范发布前已冻结的遗留文档示例（见第 6 节） |

### 1.3 生效时间

**自设计文档 v1.10 起**，一切 **新编写** 的实现代码须遵循本规范。现有架构文档中的 `ime-*` 示例视为 **遗留命名**，实现时按 [附录 A](#附录-a遗留-ime--yc-迁移对照表) 迁移。

---

## 2. 总则

### 2.1 前缀分层

```text
yc 前缀层（产品 / 厂商边界）
  ├── 仓库与目录：yc-core、yc-shell-android
  ├── Rust crate：yc-ffi、yc-session
  ├── C FFI：yc_hot.h、yc_hot_submit
  └── 产物：libyc_ffi.so、yc_ffi.dll

领域层（无前缀或通用名）
  ├── SessionManager、EditorId、UserAction
  ├── KeyView、CandBar、HandwritingService
  └── PluginHost、EngineFactory
```

**规则**：仅在 **跨仓库边界、跨语言 FFI、安装产物、包名** 等「产品标识」处强制 `yc`；Rust crate **内部** 模块、函数使用惯用 `snake_case`，**不重复** 叠加 `yc_` 前缀。

### 2.2 命名风格速查

| 层级 | 风格 | 示例（新） | 替换（旧） |
|------|------|-----------|-----------|
| Git 仓库 / 工作区根 | `yc-{域}` kebab-case | `yc-core`, `yc-design` | `ime-core`, `ime-design` |
| Rust crate 名 | `yc-{模块}` kebab-case | `yc-ffi`, `yc-session` | `ime-ffi`, `ime-session` |
| Rust 库 target 名 | `yc_ffi` snake_case | `libyc_ffi.so` | `libime_ffi.so` |
| C ABI 头文件 | `yc_{域}.h` | `yc_hot.h` | `ime_hot.h` |
| C 导出函数 | `yc_` snake_case | `yc_hot_submit` | `ime_hot_submit` |
| C 结构体 / 枚举 | `Yc` + PascalCase | `YcHotAction` | `ImeHotAction` |
| C 错误码宏 | `YC_` UPPER_SNAKE | `YC_OK`, `YC_ERR_SESSION` | `IME_OK` |
| Rust 对外 `#[repr(C)]` 类型 | `Yc` + PascalCase | `YcHotHeader` | `ImeHotHeader` |
| Rust 内部模块 / 函数 | snake_case | `session::manager` | 同左 |
| CLI 工具 | `yc-{tool}` | `yc-pack`, `yc-lexicon` | `ime-pack` |
| 语言包 / 皮肤扩展名 | `.ycpack`, `.ycskin` | 产物与工具一致 | `.imepack`, `.imeskin` |
| 用户数据子目录 | `ycpacks/` | 语言包安装目录 | `langpacks/`（遗留，迁移期可并存） |

### 2.3 不加 `yc` 前缀的范围

- **系统 IMF API**：`InputConnection`、`textDocumentProxy`、`IBusEngine` 等。
- **领域模型**（架构已定义且不含 `ime` 前缀）：`EditorFingerprint`、`ImmSnapshot`、`PrivacyLevel`、`StrokeBatch`。
- **UI 组件通用名**：`KeyView`、`CandBar`、`Toolbar`、`HandwritingPad`（壳工程内命名空间由平台包名约束，见第 3 节）。
- **Cargo feature 名**：语义化小写即可，如 `handwriting`、`lang-pack-ota`，不强制 `yc-` 前缀。

---

## 3. 分语言约束

### 3.1 Rust（`yc-core`）

**仓库布局**

```text
yc-core/
  Cargo.toml
  cbindgen.toml
  include/
    yc_hot.h              # cbindgen 生成，提交 Git
  crates/
    yc-ffi/
    yc-session/
    yc-engine/
    yc-lexicon/
    yc-plugin/
    yc-intel/
    yc-ai/
    yc-handwriting/
    yc-ext/
    yc-data/
```

**规则**

| 项 | 规范 |
|----|------|
| crate 名 | `yc-{模块}`，与目录名一致 |
| `lib.name` | `yc_ffi`（下划线，供 `libyc_ffi.so`） |
| `#[no_mangle] extern "C"` | 函数名 **必须** `yc_` 前缀，如 `yc_core_init` |
| 对外 C 类型 | `YcHotAction`、`YcHotHeader` |
| 内部类型 | `SessionManager`、`Scheduler`（无前缀） |
| 错误码常量 | `pub const YC_OK: i32 = 0;` |
| 测试 crate | `yc-ffi` 内 `#[cfg(test)]` 或 `yc-ffi/tests/`，不另建 `yc-ffi-test` |

**`yc-ffi` Cargo.toml 示例**

```toml
[package]
name = "yc-ffi"
version = "0.1.0"

[lib]
name = "yc_ffi"
crate-type = ["cdylib", "staticlib", "rlib"]

[dependencies]
yc-session = { path = "../yc-session" }
yc-engine  = { path = "../yc-engine" }
```

### 3.2 C / C++（FFI 消费方）

| 项 | 规范 |
|----|------|
| 头文件 | 仅 `#include "yc_hot.h"`（冷路径 `yc_cold.h`）；**禁止** 第二套无前缀 FFI |
| 函数调用 | `yc_hot_submit(&action)` |
| C++ 类 | `YcPlatformAdapter`、`YcTsfp` |
| C++ 源文件 | `YcTsfp.cpp`、`YcPlatformAdapter.cpp` |
| Windows DLL | `yc_ffi.dll`（Rust）、`yc_tip.dll`（TSF 壳） |
| Linux IBus 插件 | `libyc-ibus.so`（项目名可含在插件名中） |
| NAPI 模块（鸿蒙） | `libyc_native.so`（若与 `yc_ffi` 拆分，NAPI 仅做转发） |

### 3.3 Kotlin（Android）

| 项 | 规范 |
|----|------|
| 应用包名 | `com.yc.input` |
| 原生子包 | `com.yc.input.native` |
| 门面类 | `YcNative`、`YcPlatformAdapter` |
| 加载库 | `System.loadLibrary("yc_ffi")` |
| JNI 符号（若手写 C++） | `Java_com_yc_input_native_YcNative_ycHotSubmit` |

**目录示例**

```text
yc-shell-android/
  app/
  yc-native/
    src/main/java/com/yc/input/native/YcNative.kt
    src/main/jniLibs/arm64-v8a/libyc_ffi.so
```

### 3.4 Swift（iOS / macOS）

| 项 | 规范 |
|----|------|
| Bridging | `#include "yc_hot.h"` |
| 桥接类 | `YcBridge`、`YcPlatformAdapter` |
| IMK 控制器 | `YcInputController` |
| xcframework | `yc_ffi.xcframework` |
| Bundle ID | `com.yc.input`（主 App 与 Extension 同团队前缀） |

### 3.5 ArkTS（鸿蒙）

| 项 | 规范 |
|----|------|
| 声明文件 | `YcNative.ets` |
| import | `import ycNative from 'libyc_native.so'` |
| Ability 包名 | `com.yc.input` |

### 3.6 壳工程与 UI 仓库

| 仓库 | 说明 |
|------|------|
| `yc-shell-android` | `InputMethodService` + JNI |
| `yc-shell-ios` | Keyboard Extension + Swift |
| `yc-shell-harmonyos` | `InputMethodExtensionAbility` + NAPI |
| `yc-shell-windows` | TSF TIP + C++ |
| `yc-shell-macos` | IMK Server + Swift |
| `yc-shell-linux` | IBus + Fcitx5 插件 |
| `yc-ui-android` / `yc-ui-ios` / `yc-ui-harmonyos` / `yc-ui-desktop` | 各端 KeyView / CandBar |

设计文档仓保留现名 `ime-design`；实现仓使用 `yc-design` 或继续托管于 monorepo 根目录，**以实现仓库 README 为准**。

### 3.7 CLI 工具链

| 工具 | 用途 |
|------|------|
| `yc-pack` | 语言包 validate / build |
| `yc-lexicon` | 词库 compile → MMAP |
| `yc-tools` | 聚合 workspace（可选 meta crate） |

命令行全局安装名与 crate bin 名一致：`yc-pack`。

### 3.8 产物扩展名与数据目录

| 产物 | 扩展名 / 路径 |
|------|--------------|
| 语言包 | `.ycpack` |
| 皮肤包 | `.ycskin` |
| AiPack | `.ycaipack`（可选，与 `.aipack` 遗留区分） |
| Windows 数据 | `%LOCALAPPDATA%\YcInput\ycpacks\` |
| macOS 数据 | `~/Library/Application Support/YcInput/ycpacks/` |
| Linux 数据 | `$XDG_DATA_HOME/yc-input/ycpacks/` |
| Android / 鸿蒙 | `{applicationContext.filesDir}/ycpacks/` |
| iOS | App Group `{Group}/ycpacks/` |

`yc_core_init(const char* data_dir)` 的 `data_dir` 指向上述品牌根目录（不含 `ycpacks` 子路径），由壳工程在启动时传入。

---

## 4. 禁止项

| # | 禁止行为 |
|---|----------|
| 1 | 新代码使用 `ime_` / `ime-` / `Ime` 产品前缀（附录对照表中的「遗留」列除外） |
| 2 | 热路径新增 **无前缀** C 导出符号（如 `hot_submit`） |
| 3 | 平台胶水绕过 `yc-ffi` 直接链接 `yc-engine` 等内部 crate |
| 4 | 同时存在 `ime_hot.h` 与 `yc_hot.h` 两套头文件 |
| 5 | crate 间复制粘贴 FFI 结构体定义而不经 `cbindgen` 生成 |
| 6 | 将第三方 crate 重命名为 `yc-*` 前缀 |

---

## 5. 评审检查清单

实现或 PR 评审时逐项确认：

- [ ] 仓库 / crate / 目录名符合 `yc-{域}` kebab-case
- [ ] `cbindgen` 输出为 `yc_hot.h`，且已纳入 CI diff 检查
- [ ] 所有 `#[no_mangle]` 导出函数以 `yc_` 开头
- [ ] C 错误码使用 `YC_*` 宏或常量，无 `IME_*` 新增
- [ ] Android `loadLibrary("yc_ffi")` / iOS `yc_ffi.xcframework` 与构建产物名一致
- [ ] Kotlin / Swift / ArkTS 包名以 `com.yc.input` 为根
- [ ] 语言包 / 皮肤扩展名为 `.ycpack` / `.ycskin`
- [ ] `data_dir` 与六端品牌目录（`YcInput` / `yc-input`）一致
- [ ] CLI 工具名为 `yc-pack` 等，无 `ime-pack` 新增
- [ ] 无第二套 FFI 边界

---

## 6. 与现有设计文档的关系

| 文档 | 关系 |
|------|------|
| [IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) | 第 9 章仓库树中 `ime-*` 为 **遗留示例**；逻辑架构不变，仅命名按本规范迁移 |
| [RUST_PLATFORM_INTEGRATION.md](RUST_PLATFORM_INTEGRATION.md) | 对接流程不变；代码片段中的 `ime_hot.h` 等实现时替换为 `yc_*` |
| [LANGPACK_AUTHORING.md](LANGPACK_AUTHORING.md) | 工具链命令迁移为 `yc-pack`；扩展名 `.ycpack` |
| [README.md](../README.md) | 索引本规范；版本 v1.10 记录命名规范发布 |

**迁移策略**：先落地 `yc-core` + `yc-ffi` smoke test（M0），再逐端壳工程重命名；不要求一次性改写全部设计文档。

---

## 附录 A：遗留 `ime` → `yc` 迁移对照表

### A.1 仓库与 crate

| 遗留 | 新名 |
|------|------|
| `ime-design` | `yc-design`（实现仓；文档仓可暂保留现名） |
| `ime-core` | `yc-core` |
| `ime-ffi` | `yc-ffi` |
| `ime-session` | `yc-session` |
| `ime-engine` | `yc-engine` |
| `ime-lexicon` | `yc-lexicon` |
| `ime-plugin` | `yc-plugin` |
| `ime-intel` | `yc-intel` |
| `ime-ai` | `yc-ai` |
| `ime-handwriting` | `yc-handwriting` |
| `ime-ext` | `yc-ext` |
| `ime-data` | `yc-data` |

### A.2 壳工程与 UI

| 遗留 | 新名 |
|------|------|
| `ime-shell-android` | `yc-shell-android` |
| `ime-shell-ios` | `yc-shell-ios` |
| `ime-shell-harmonyos` | `yc-shell-harmonyos` |
| `ime-shell-windows` | `yc-shell-windows` |
| `ime-shell-macos` | `yc-shell-macos` |
| `ime-shell-linux` | `yc-shell-linux` |
| `ime-ui-android` | `yc-ui-android` |
| `ime-ui-ios` | `yc-ui-ios` |
| `ime-ui-harmonyos` | `yc-ui-harmonyos` |
| `ime-ui-desktop` | `yc-ui-desktop` |
| `ime-native`（Android 模块） | `yc-native` |

### A.3 FFI 与产物

| 遗留 | 新名 |
|------|------|
| `ime_hot.h` | `yc_hot.h` |
| `ime_core_init` | `yc_core_init` |
| `ime_core_shutdown` | `yc_core_shutdown` |
| `ime_hot_submit` | `yc_hot_submit` |
| `ime_hot_arena_ptr` | `yc_hot_arena_ptr` |
| `ime_session_validate` | `yc_session_validate` |
| `ime_session_stop` | `yc_session_stop` |
| `ime_cold_submit` | `yc_cold_submit` |
| `ime_cold_cancel` | `yc_cold_cancel` |
| `ImeHotAction` | `YcHotAction` |
| `ImeHotHeader` | `YcHotHeader` |
| `ImeCandidateSlot` | `YcCandidateSlot` |
| `IME_OK` | `YC_OK` |
| `IME_ERR_SESSION` | `YC_ERR_SESSION` |
| `IME_ERR_BUSY` | `YC_ERR_BUSY` |
| `IME_ERR_INTERNAL` | `YC_ERR_INTERNAL` |
| `libime_ffi.so` | `libyc_ffi.so` |
| `ime_ffi.dll` | `yc_ffi.dll` |
| `libime_ffi.dylib` | `libyc_ffi.dylib` |
| `ime_ffi.xcframework` | `yc_ffi.xcframework` |

### A.4 平台胶水类与文件

| 遗留 | 新名 |
|------|------|
| `ImeNative.kt` | `YcNative.kt` |
| `ImeBridge.swift` | `YcBridge.swift` |
| `ImePlatformAdapter` | `YcPlatformAdapter` |
| `ImeInputController.swift` | `YcInputController.swift` |
| `ImeTsfp.cpp` | `YcTsfp.cpp` |
| `ImeNative.ets` | `YcNative.ets` |
| `ime_napi.cpp` | `yc_napi.cpp` |

### A.5 工具与产物扩展名

| 遗留 | 新名 |
|------|------|
| `ime-pack` | `yc-pack` |
| `ime-lexicon` | `yc-lexicon` |
| `ime-tools` | `yc-tools` |
| `.imepack` | `.ycpack` |
| `.imeskin` | `.ycskin` |

### A.6 领域类型（保留不变）

以下名称 **不添加 `yc` 前缀**，迁移时仅更新其所在 crate 路径：

`SessionManager`、`Scheduler`、`PluginHost`、`EngineFactory`、`InputEngine`、`HandwritingService`、`AiAssistService`、`EditorId`、`UserAction`、`ImmSnapshot`、`ThemeTokens`、`LangPackSlot`

---

## 附录 B：建议 CI 检查（`yc-core` 实现阶段）

```yaml
# 片段：禁止新增 ime_ 符号
- name: Check no new ime_ exports
  run: |
    ! nm target/release/libyc_ffi.so | grep -E ' ime_'
- name: cbindgen sync
  run: |
    cbindgen crates/yc-ffi -o include/yc_hot.h
    git diff --exit-code include/yc_hot.h
```

---

*文档结束*
