# Rust 与 Android / iOS / 鸿蒙 / Windows / macOS / Linux 对接方案与教程

> 版本：1.2  
> 关联：[IME_ARCHITECTURE.md](IME_ARCHITECTURE.md) 2.4 Rust 技术栈、附录 A C ABI  
> 读者：各平台壳工程开发者、Rust 核心工程师

---

## 1. 总览

### 1.1 设计目标

| 目标 | 做法 |
|------|------|
| **六端复用核心** | 组词、Session、词库、语言包逻辑全部在 Rust |
| **单一 FFI 边界** | 仅 `ime-ffi` crate 暴露 C ABI；平台只调 C 函数 |
| **热路径低延迟** | 定长 struct + 可选共享 Arena；JNI / NAPI / Swift / **C++ TSF·IMK·IBus** 薄封装 |
| **冷路径可扩展** | Tokio IO 线程 + 回调到主线程；FlatBuffers 载荷 |
| **Extension 可裁剪** | Cargo feature 控制体积；iOS 静态库 / 鸿蒙与 Android 共用 cdylib / **桌面全 feature** |

### 1.2 分层与数据流

```text
┌─────────────────────────────────────────────────────────────────┐
│  System IMF                                                     │
│  Android: InputMethodService + InputConnection                  │
│  iOS:     UIInputViewController + textDocumentProxy             │
│  鸿蒙:    InputMethodExtensionAbility + InputClient (IME Kit)   │
│  Windows: TSF ITfTextInputProcessor + ITfContext                │
│  macOS:   IMKInputController + IMKCandidates                    │
│  Linux:   IBus Engine / Fcitx5 InputMethodEngineV3              │
└────────────────────────────┬────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────┐
│  Presentation（Kotlin / Swift / ArkTS / C++ / Qt·GTK 可选）      │
│  KeyView · CandBar · Toolbar · ThemeTokens 渲染                  │
└────────────────────────────┬────────────────────────────────────┘
                             │ UiBinder
┌────────────────────────────▼────────────────────────────────────┐
│  Platform Adapter（平台胶水，不含业务）                            │
│  Android: ImeNative.kt + JNI (libime_ffi.so)                      │
│  iOS:     ImeBridge.swift + ime_hot.h                             │
│  鸿蒙:    ImeNative.ets + NAPI C++ (libime_ffi.so)                │
│  Windows: ImeTsfp.cpp + ime_ffi.dll                               │
│  macOS:   ImeBridge.swift + libime_ffi.dylib                      │
│  Linux:   ime_ibus.cpp / ime_fcitx5.cpp + libime_ffi.so          │
└────────────────────────────┬────────────────────────────────────┘
                             │ C ABI（ime_hot.h）
┌────────────────────────────▼────────────────────────────────────┐
│  ime-ffi（唯一跨语言边界）                                         │
│  ime_hot_submit · ime_session_* · ime_cold_*                     │
└────────────────────────────┬────────────────────────────────────┘
                             │ 纯 Rust 调用
┌────────────────────────────▼────────────────────────────────────┐
│  ime-session · ime-engine · ime-lexicon · ime-plugin · ime-data … │
└─────────────────────────────────────────────────────────────────┘
```

**原则**：Kotlin / Swift / ArkTS / C++ **永远不**直接依赖 `ime-engine` 等内部 crate，只包含 `ime_hot.h` 并链接 `libime_ffi`。

---

## 2. 仓库与 Crate 布局

```text
ime-core/                          # Cargo workspace
  Cargo.toml
  cbindgen.toml
  crates/
    ime-ffi/                       # cdylib + staticlib，对外唯一
    ime-session/
    ime-engine/
    ime-lexicon/
    ime-plugin/
    ime-data/
    ...

ime-shell-android/                 # Android Studio 工程
  app/
  ime-native/                      # JNI + Rust 构建脚本
    build.gradle.kts
    src/main/java/.../ImeNative.kt
    src/main/rust/                 # 可选：指向 ime-core 的符号链接

ime-shell-ios/                     # Xcode 工程
  ImeKeyboard/                     # Keyboard Extension target
  ImeApp/                          # 主 App（语言包下载）
  Rust/                            # xcframework 输出目录
    ime_ffi.xcframework

ime-shell-harmonyos/               # DevEco Studio 工程（HarmonyOS NEXT）
  entry/
  ime_extension/
  ime_native/

ime-shell-windows/                 # TSF Text Input Processor
  ime_tip/                         # ITfTextInputProcessor 实现
    ImeTsfp.cpp / ImeTsfp.h
    ImePlatformAdapter.cpp
  ime_ui/                          # Win32 / WinUI 3 自绘键盘（可选）
  libs/x64/ime_ffi.dll

ime-shell-macos/                   # InputMethodKit
  ImeInputController.swift
  ImeServer.swift
  Bridge/ime_hot.h
  libs/libime_ffi.dylib

ime-shell-linux/                   # IBus + Fcitx5 双后端
  ibus-ime/                        # IBusEngine 插件
  fcitx5-ime/                      # Fcitx5 Addon
  common/ime_platform_adapter.cpp
  libs/x86_64/libime_ffi.so
```

### 2.1 `ime-ffi` 产物形态

| 平台 | 链接形式 | 产物名 | 说明 |
|------|----------|--------|------|
| Android | `cdylib` | `libime_ffi.so` | arm64-v8a, armeabi-v7a, x86_64 |
| 鸿蒙 | `cdylib` | `libime_ffi.so` | `aarch64-unknown-linux-ohos`；模拟器 `x86_64-unknown-linux-ohos` |
| iOS App / Extension | `staticlib` | `libime_ffi.a` | 打进 xcframework；Extension 与主 App 各链接一份 |
| **Windows** | **`cdylib`** | **`ime_ffi.dll`** | **`x86_64-pc-windows-msvc`**；可选 **`aarch64-pc-windows-msvc`** |
| **macOS** | **`cdylib` 或 `staticlib`** | **`libime_ffi.dylib`** | **`aarch64-apple-darwin` / `x86_64-apple-darwin`** |
| **Linux** | **`cdylib`** | **`libime_ffi.so`** | **`x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu`** |
| 单元测试 / desktop dev | `rlib` | — | `cargo test` 不经过 JNI / NAPI / TSF |

`Cargo.toml`（`ime-ffi`）示例：

```toml
[lib]
name = "ime_ffi"
crate-type = ["cdylib", "staticlib", "rlib"]

[dependencies]
ime-session = { path = "../ime-session" }
ime-engine  = { path = "../ime-engine" }
# ...

[features]
default = ["session", "engine", "lexicon"]
full = ["session", "engine", "lexicon", "plugin", "ai", "ext"]
```

---

## 3. C ABI 契约（cbindgen）

### 3.1 生成头文件

`cbindgen.toml`：

```toml
language = "C"
include_guard = "IME_HOT_H"
autogen_warning = "/* 自动生成，请勿手改 */"
[export]
include = ["ImeHotAction", "ImeHotHeader", "ImeCandidateSlot"]
[fn]
rename_args = "None"
```

构建：

```bash
cd ime-core
cargo build -p ime-ffi --release
cbindgen crates/ime-ffi -o include/ime_hot.h
```

头文件同时拷贝到：

- `ime-shell-android/ime-native/src/main/jniLibs/include/ime_hot.h`
- `ime-shell-ios/ImeKeyboard/Bridge/ime_hot.h`
- `ime-shell-harmonyos/ime_native/include/ime_hot.h`
- `ime-shell-windows/ime_tip/include/ime_hot.h`
- `ime-shell-macos/Bridge/ime_hot.h`
- `ime-shell-linux/common/include/ime_hot.h`

### 3.2 热路径 API（与附录 A 对齐）

```c
// include/ime_hot.h（节选）
#define IME_OK              0
#define IME_ERR_SESSION     -1
#define IME_ERR_BUSY        -2
#define IME_ERR_INTERNAL    -3

typedef struct {
    uint64_t editor_id;
    uint64_t client_seq;
    uint32_t action_type;
    uint32_t key_code;
    uint32_t candidate_id;
    uint32_t flags;
    uint8_t  reserved[8];
} ImeHotAction;

typedef struct {
    uint64_t editor_id;
    uint64_t seq;
    uint32_t status_flags;
    uint32_t composing_len;
    uint32_t cand_count;
    uint32_t cmd_count;
} ImeHotHeader;

int32_t ime_core_init(const char* data_dir);
void    ime_core_shutdown(void);

int32_t ime_hot_submit(const ImeHotAction* action);
void*   ime_hot_arena_ptr(void);
size_t  ime_hot_arena_size(void);
int32_t ime_hot_latest_seq(uint64_t editor_id, uint64_t* out_seq);

uint64_t ime_session_get_active(void);
int32_t  ime_session_validate(uint64_t editor_id);
void     ime_session_stop(uint64_t editor_id, uint32_t reason);
```

### 3.3 冷路径 API（异步）

```c
typedef void (*ImeColdCallback)(int32_t task_id, uint64_t editor_id,
                                const uint8_t* payload, size_t len, int32_t err);

int32_t ime_cold_submit(uint64_t editor_id, uint32_t kind,
                        const uint8_t* payload, size_t len,
                        ImeColdCallback cb);   // 从任意线程调用；cb 在 IO 线程

void ime_cold_cancel(int32_t task_id);
```

Rust 侧 `ime-ffi` 用 `catch_unwind` 包裹，panic 映射为 `IME_ERR_INTERNAL`。

---

## 4. Android 对接

### 4.1 构建链路

```text
Gradle (ime-native module)
  → cargo-ndk / rust-android-gradle
  → cargo build --target aarch64-linux-android (等)
  → libime_ffi.so → jniLibs/arm64-v8a/
  → Kotlin System.loadLibrary("ime_ffi")
```

**推荐工具**：[cargo-ndk](https://github.com/bbqsrc/cargo-ndk) + Android Gradle Plugin。

`ime-native/build.gradle.kts`（要点）：

```kotlin
android {
    defaultConfig {
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }
    externalNativeBuild {
        // 或使用 rust-android-gradle 插件触发 cargo
    }
}

tasks.register("cargoBuildImeFfi") {
    // ./gradlew cargoBuildImeFfi
    commandLine("cargo", "ndk", "-t", "arm64-v8a", "-t", "armeabi-v7a",
                "-o", "src/main/jniLibs", "build", "-p", "ime-ffi", "--release")
}
```

### 4.2 JNI 薄封装（Kotlin + 可选 Rust jni crate）

**方案 A（推荐）**：Kotlin `external` 直接声明 C 函数，**不**再写一层 Rust JNI。

`ImeNative.kt`：

```kotlin
package com.example.ime.native

object ImeNative {
    init { System.loadLibrary("ime_ffi") }

    external fun imeCoreInit(dataDir: String): Int
    external fun imeCoreShutdown()
    external fun imeHotSubmit(action: ByteArray): Int   // 定长 40B，memcpy ImeHotAction
    external fun imeHotArenaPtr(): Long                  // GetDirectBufferAddress 用
    external fun imeHotLatestSeq(editorId: Long): Long
    external fun imeSessionValidate(editorId: Long): Boolean
    external fun imeSessionStop(editorId: Long, reason: Int)
}
```

`ime_native_jni.cpp`（极简 JNI，仅转发）：

```cpp
#include <jni.h>
#include "ime_hot.h"

extern "C" JNIEXPORT jint JNICALL
Java_com_example_ime_native_ImeNative_imeHotSubmit(JNIEnv* env, jclass, jbyteArray arr) {
    jbyte* bytes = env->GetByteArrayElements(arr, nullptr);
    jint ret = ime_hot_submit(reinterpret_cast<const ImeHotAction*>(bytes));
    env->ReleaseByteArrayElements(arr, bytes, JNI_ABORT);
    return ret;
}
```

**方案 B**：在 `ime-ffi` 内用 [`jni` crate](https://docs.rs/jni) 导出 `Java_com_example_ime_ImeBridge_*`，适合希望 JNI 签名也由 Rust 生成的团队。

### 4.3 Platform Adapter（Kotlin）

```kotlin
class AndroidPlatformAdapter(
    private val mainHandler: Handler,
    private val dataDir: File,
) : PlatformAdapter {

    private val actionBuf = ByteBuffer.allocateDirect(40).order(ByteOrder.LITTLE_ENDIAN)

    fun init() {
        check(ImeNative.imeCoreInit(dataDir.absolutePath) == IME_OK)
    }

    override fun submitHot(editorId: Long, userAction: UserAction): HotResult {
        if (!ImeNative.imeSessionValidate(editorId)) return HotResult.SessionInvalid
        packAction(actionBuf, editorId, userAction)
        val code = ImeNative.imeHotSubmit(actionBuf.array()) // 或 DirectBuffer 地址
        if (code != IME_OK) return HotResult.Error(code)
        return readSnapshotFromArena(editorId)
    }

    override fun invokeCold(editorId: Long, req: ColdRequest, callback: (ColdResponse) -> Unit) {
        // FlatBuffers 序列化 payload → native cold_submit
        // 回调 post 到 mainHandler
    }
}
```

### 4.4 线程模型（Android）

| 线程 | 职责 |
|------|------|
| **主线程** | 触摸、绘制、`readSnapshotFromArena`、执行 `UiCommand` |
| **IME 服务线程** | `InputMethodService` 生命周期；`submitHot` 可在主线程同步调用 |
| **Rust IO 线程** | Tokio runtime：`ime_cold_submit`、语言包下载、SQLite |
| **JNI 回调** | `ImeColdCallback` 收到后 `Handler.post` 回主线程 |

**禁止**：在 `Binder` 线程长时间阻塞；热路径 `ime_hot_submit` 目标 &lt; 16ms，内部不得 `await` 网络。

### 4.5 InputMethodService 集成要点

```kotlin
class ImeService : InputMethodService() {
    private lateinit var adapter: AndroidPlatformAdapter
    private lateinit var uiBinder: UiBinder

    override fun onCreate() {
        super.onCreate()
        adapter = AndroidPlatformAdapter(mainLooper, filesDir)
        adapter.init()
        uiBinder = UiBinder(adapter, CandBarView(this), KeyView(this))
    }

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        val editorId = EditorFingerprint.from(attribute, currentInputConnection)
        val sessionId = adapter.onEditorFocus(editorId, attribute)
        uiBinder.bind(sessionId)
    }

    override fun onFinishInput() {
        uiBinder.unbind()
        adapter.onEditorBlur()
    }
}
```

---

## 5. iOS 对接

### 5.1 构建链路

```text
ime-core/
  scripts/build-ios.sh
    → rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
    → cargo build --release --target aarch64-apple-ios -p ime-ffi
    → cargo build --release --target aarch64-apple-ios-sim -p ime-ffi
    → xcodebuild -create-xcframework \
         -library target/.../libime_ffi.a -headers include/ \
         -output Rust/ime_ffi.xcframework
```

`build-ios.sh` 示例：

```bash
#!/usr/bin/env bash
set -euo pipefail
TARGETS=("aarch64-apple-ios" "aarch64-apple-ios-sim")
for t in "${TARGETS[@]}"; do
  cargo build --release -p ime-ffi --target "$t"
done
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libime_ffi.a -headers include \
  -library target/aarch64-apple-ios-sim/release/libime_ffi.a -headers include \
  -output ../ime-shell-ios/Rust/ime_ffi.xcframework
```

Xcode：**Keyboard Extension** 与 **主 App** 均链接 `ime_ffi.xcframework`，Embed 选 **Do Not Embed**（静态库）。

### 5.2 Swift 桥接

`ImeBridge.swift`：

```swift
import Foundation

// 通过 Bridging Header 引入 ime_hot.h：
// #include "ime_hot.h"

enum ImeError: Int32 {
    case ok = 0
    case session = -1
    case busy = -2
    case internalError = -3
}

final class IosPlatformAdapter: PlatformAdapter {
    private let actionStorage: UnsafeMutablePointer<ImeHotAction>

    init(dataDir: URL) {
        actionStorage = .allocate(capacity: 1)
        let dir = (dataDir.path as NSString).utf8String
        guard ime_core_init(dir) == IME_OK else { fatalError("ime_core_init") }
    }

    deinit {
        ime_core_shutdown()
        actionStorage.deallocate()
    }

    func submitHot(editorId: UInt64, action: UserAction) -> HotResult {
        guard ime_session_validate(editorId) != 0 else { return .sessionInvalid }
        packAction(&actionStorage.pointee, editorId: editorId, action: action)
        let code = ime_hot_submit(actionStorage)
        guard code == IME_OK else { return .error(code) }
        return readSnapshotFromArena(editorId: editorId)
    }
}
```

`ImeKeyboard-Bridging-Header.h`：

```c
#include "ime_hot.h"
```

### 5.3 UIInputViewController 集成要点

```swift
class KeyboardViewController: UIInputViewController {
    private var adapter: IosPlatformAdapter!
    private var uiBinder: UiBinder!

    override func viewDidLoad() {
        super.viewDidLoad()
        let dataDir = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: "group.com.example.ime")!
        adapter = IosPlatformAdapter(dataDir: dataDir)
        uiBinder = UiBinder(adapter: adapter, keyView: keyView, candBar: candBar)
    }

    override func textDidChange(_ textInput: UITextInput?) {
        let fp = EditorFingerprint.make(from: textDocumentProxy)
        let editorId = adapter.onEditorFocus(fingerprint: fp)
        uiBinder.bind(editorId: editorId)
    }
}
```

### 5.4 线程模型（iOS）

| 线程 | 职责 |
|------|------|
| **主线程** | UIKit 键盘 UI、读 Arena、`textDocumentProxy` 上屏 |
| **Rust IO 线程** | 冷路径；回调用 `DispatchQueue.main.async` |
| **Extension 限制** | 内存 ~30–60MB；用 `default` + `lang-pack-runtime` feature |

**禁止**：在 Extension 内 `dlopen` 未签名 dylib；语言包仅数据驱动（见架构 3.5）。

### 5.5 App Group 与主 App 分工

```text
主 App (ImeApp)
  PluginHost.install → 写入 {AppGroup}/langpacks/
  ime_cold_submit(LangPackInstall)   // 可开完整 feature

Keyboard Extension
  PluginHost.listInstalled / enable  // 只读
  ime_core_init(AppGroup/data)       // 最小 feature
```

两 target 链接**同一** `ime_ffi.xcframework`，通过 Cargo feature 在编译时裁剪。

---

## 6. 鸿蒙（HarmonyOS NEXT）对接

> 目标系统：**HarmonyOS NEXT**（纯鸿蒙，ArkTS + 原生 NAPI）；与 Android 同为 **`.so` cdylib** 链路，壳层用 **ArkTS + NAPI** 替代 Kotlin + JNI。

### 6.1 系统 IMF 与工程模型

| 项 | 鸿蒙 |
|----|------|
| 输入法扩展 | `InputMethodExtensionAbility`（`@kit.IMEKit`） |
| 文本提交 | `inputMethod.InputClient`：`insertText` / `deleteBackward` / `sendKeyFunction` |
| 编辑框属性 | `EditorAttribute`：`inputType`、`enterKeyType`、`bundleName` |
| UI | ArkUI 声明式（`.ets`） |
| 主 App | `EntryAbility`：语言包商店、下载、设置（完整 Cargo feature） |
| 扩展 | `ime_extension` 模块：键盘 UI + 组词热路径（`default` + `lang-pack-runtime`） |

```text
ime-shell-harmonyos/
  entry/                 # 主 Ability
  ime_extension/         # InputMethodExtensionAbility
  ime_native/            # NAPI 模块，导出 libime_ffi.so + ime_napi.so
```

### 6.2 构建链路

```text
DevEco Studio / hvigor
  → scripts/build-ohos.sh（cargo + OHOS NDK）
  → cargo build --target aarch64-unknown-linux-ohos -p ime-ffi
  → libime_ffi.so → ime_native/libs/arm64-v8a/
  → CMake 编译 ime_napi.cpp（链接 libime_ffi.so）
  → ArkTS import 'libime_native.so'
```

**环境变量**（示例）：

```bash
export OHOS_NDK_HOME=/path/to/openharmony/native
export CC_aarch64_unknown_linux_ohos=$OHOS_NDK_HOME/llvm/bin/aarch64-unknown-linux-ohos-clang
export CXX_aarch64_unknown_linux_ohos=$OHOS_NDK_HOME/llvm/bin/aarch64-unknown-linux-ohos-clang++
export AR_aarch64_unknown_linux_ohos=$OHOS_NDK_HOME/llvm/bin/llvm-ar
```

`scripts/build-ohos.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail
rustup target add aarch64-unknown-linux-ohos x86_64-unknown-linux-ohos
TARGETS=("aarch64-unknown-linux-ohos" "x86_64-unknown-linux-ohos")
OUT=../ime-shell-harmonyos/ime_native/libs
for t in "${TARGETS[@]}"; do
  cargo build --release -p ime-ffi --target "$t"
  abi=$([ "$t" = "aarch64-unknown-linux-ohos" ] && echo arm64-v8a || echo x86_64)
  mkdir -p "$OUT/$abi"
  cp "target/$t/release/libime_ffi.so" "$OUT/$abi/"
done
```

`ime_native/CMakeLists.txt`（要点）：`add_library(ime_native SHARED ime_napi.cpp)`，`target_link_libraries(ime_native ime_ffi)`。

### 6.3 NAPI 薄封装（对齐 Android JNI）

**方案 A（推荐）**：C++ NAPI 模块转发至 **同一套** `ime_hot.h`，与 Android JNI 逻辑对称。

`ime_napi.cpp`：

```cpp
#include "napi/native_api.h"
#include "ime_hot.h"

static napi_value ImeHotSubmit(napi_env env, napi_callback_info info) {
    size_t argc = 1;
    napi_value args[1];
    napi_get_cb_info(env, info, &argc, args, nullptr, nullptr);
    void* data = nullptr;
    size_t len = 0;
    napi_get_arraybuffer_info(env, args[0], &data, &len);
    if (len < sizeof(ImeHotAction)) {
        napi_throw_error(env, nullptr, "ImeHotAction buffer too small");
        return nullptr;
    }
    int32_t ret = ime_hot_submit(reinterpret_cast<const ImeHotAction*>(data));
    napi_value result;
    napi_create_int32(env, ret, &result);
    return result;
}

EXTERN_C_START
static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        { "imeHotSubmit", nullptr, ImeHotSubmit, nullptr, nullptr, nullptr, napi_default, nullptr },
        { "imeCoreInit", nullptr, ImeCoreInit, nullptr, nullptr, nullptr, napi_default, nullptr },
        // imeHotArenaPtr, imeSessionValidate, ...
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}
EXTERN_C_END

static napi_module imeModule = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "ime_native",
    .nm_priv = nullptr,
    .reserved = { 0 },
};
extern "C" __attribute__((constructor)) void RegisterImeModule(void) {
    napi_module_register(&imeModule);
}
```

**方案 B**：Rust 侧用 [`napi-ohos`](https://ohos.rs) / `napi-derive-ohos` 直接导出 NAPI，适合团队已全面使用 ohos-rs；仍建议热路径只暴露定长 struct，与 `ime-ffi` 共用实现。

### 6.4 ArkTS Platform Adapter

`ime_native/index.d.ts`（类型声明）：

```typescript
export const imeCoreInit: (dataDir: string) => number;
export const imeHotSubmit: (action: ArrayBuffer) => number;
export const imeHotLatestSeq: (editorId: bigint) => bigint;
export const imeSessionValidate: (editorId: bigint) => boolean;
```

`HarmonyPlatformAdapter.ets`：

```typescript
import imeNative from 'libime_native.so';

export class HarmonyPlatformAdapter implements PlatformAdapter {
  private actionBuf: ArrayBuffer = new ArrayBuffer(40);

  init(dataDir: string): void {
    if (imeNative.imeCoreInit(dataDir) !== 0) {
      throw new Error('ime_core_init failed');
    }
  }

  submitHot(editorId: bigint, action: UserAction): HotResult {
    if (!imeNative.imeSessionValidate(editorId)) {
      return HotResult.SessionInvalid;
    }
    packAction(this.actionBuf, editorId, action);
    const code = imeNative.imeHotSubmit(this.actionBuf);
    if (code !== 0) return HotResult.Error(code);
    return this.readSnapshotFromArena(editorId);
  }

  invokeCold(editorId: bigint, req: ColdRequest, callback: (resp: ColdResponse) => void): void {
    // FlatBuffers 编码 payload → imeColdSubmit
    // 回调通过 mainThreadExecutor 切回 UI 线程
  }
}
```

### 6.5 InputMethodExtensionAbility 集成要点

```typescript
import { InputMethodExtensionAbility } from '@kit.IMEKit';
import { HarmonyPlatformAdapter } from '../adapter/HarmonyPlatformAdapter';

export default class ImeExtensionAbility extends InputMethodExtensionAbility {
  private adapter: HarmonyPlatformAdapter = new HarmonyPlatformAdapter();
  private uiBinder: UiBinder | null = null;

  onCreate(): void {
    const dataDir = this.context.getApplicationContext().filesDir; // 应用级目录
    this.adapter.init(dataDir);
    this.uiBinder = new UiBinder(this.adapter, /* KeyView, CandBar */);
  }

  onInputStart(editorAttribute: inputMethod.EditorAttribute, inputClient: inputMethod.InputClient): void {
    const editorId = EditorFingerprint.from(editorAttribute, inputClient);
    const sessionId = this.adapter.onEditorFocus(editorId, editorAttribute);
    this.uiBinder?.bind(sessionId, inputClient);
  }

  onInputStop(): void {
    this.uiBinder?.unbind();
    this.adapter.onEditorBlur();
  }
}
```

`EditorFingerprint` 字段来源：

| 字段 | 鸿蒙来源 |
|------|----------|
| `bundle_name` | `editorAttribute.bundleName` |
| `input_type` | `editorAttribute.inputType` |
| `enter_key_type` | `editorAttribute.enterKeyType` |
| `field_id` | `editorAttribute` 稳定哈希（或系统提供的实例 id） |

### 6.6 线程模型（鸿蒙）

| 线程 | 职责 |
|------|------|
| **UI 线程** | ArkUI 渲染、读 Arena、`InputClient` 上屏 |
| **Rust IO 线程** | Tokio：`ime_cold_submit`、语言包下载 |
| **冷路径回调** | `taskpool` 或 `EventHandler` post 回 UI 线程 |

**禁止**：在 `onInputStart` 同步路径阻塞网络；热路径与 Android 相同目标 P95 ≤ 16ms。

### 6.7 语言包与主 App 共享

鸿蒙无 iOS App Group，采用 **同 Bundle 应用级沙箱**：

```text
主 EntryAbility（完整 feature）
  PluginHost.install → {applicationContext.filesDir}/langpacks/

InputMethodExtensionAbility（lang-pack-runtime）
  PluginHost.listInstalled / enable → 只读同一 filesDir/langpacks/
```

若未来需跨 Bundle 共享，可走 **DataShare** / 分布式数据对象；本方案优先同应用内路径，与 Android `filesDir` 模式一致。

### 6.8 与 Android / iOS 差异对照

| 项 | Android | iOS | 鸿蒙 |
|----|---------|-----|------|
| UI 语言 | Kotlin | Swift | ArkTS |
| 胶水 | JNI | Bridging Header | NAPI (C++) |
| Rust 产物 | `.so` | `.a` / xcframework | `.so` |
| 扩展 Ability | `InputMethodService` | Keyboard Extension | `InputMethodExtensionAbility` |
| 语言包共享 | `filesDir` / 同签 | App Group | `applicationContext.filesDir` |
| `native_so` 引擎 OTA | ✓ 同签 | ✗ | ✓ 同 Bundle 签名（策略同 Android） |
| 模拟器 target | x86_64-linux-android | ios-sim | x86_64-unknown-linux-ohos |

### 6.9 鸿蒙 M0 验证步骤

```bash
# 1. 编译 Rust
cd ime-core && ./scripts/build-ohos.sh

# 2. DevEco 打开 ime-shell-harmonyos，Sync 原生模块
# 3. Run ime_extension 到真机/模拟器
# 4. 系统设置 → 输入法 → 启用本 IME
# 5. 日志确认 ime_core_init 返回 0
```

---

## 7. Windows 对接（TSF）

### 7.1 框架与注册

Windows 现代输入法基于 **TSF**（Text Services Framework），实现 `ITfTextInputProcessor` 并注册为 TIP（Text Input Processor）。

```text
ime_tip.dll（COM 组件）
  → DllRegisterServer / DllUnregisterServer
  → ITfTextInputProcessor::Activate(ITfThreadMgr*)
  → 创建 Keyboard / Candidate UI（HWND 子窗口或独立浮层）
  → PlatformAdapter → ime_ffi.dll
```

**不推荐** 新实现 IMM32（`HIMC`）路径；仅可在 TSF 不可用时作只读兼容层。

### 7.2 构建链路

```bash
rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc
cd ime-core
cargo build -p ime-ffi --release --target x86_64-pc-windows-msvc
# → target/x86_64-pc-windows-msvc/release/ime_ffi.dll
```

CMake / MSBuild 将 `ime_ffi.dll` 与 `ime_tip.dll` 一并安装；`ime_core_init` 的 `data_dir` 传 `%LOCALAPPDATA%\ImeApp\`。

### 7.3 Platform Adapter（C++）

```cpp
// ImePlatformAdapter.cpp
#include "ime_hot.h"

class WindowsPlatformAdapter {
public:
    int Init(const wchar_t* dataDir) {
        return ime_core_init(WideToUtf8(dataDir).c_str());
    }
    HotResult SubmitHot(uint64_t editorId, const UserAction& action) {
        if (!ime_session_validate(editorId)) return HotResult::SessionInvalid;
        ImeHotAction hot{};
        PackAction(hot, editorId, action);
        if (ime_hot_submit(&hot) != IME_OK) return HotResult::Error;
        return ReadSnapshotFromArena(editorId);
    }
    void ExecuteCommit(const UiCommand& cmd, ITfContext* context) {
        // ITfInsertAtSelection::InsertTextAtSelection
    }
};
```

### 7.4 焦点与 Session

| 事件 | 行为 |
|------|------|
| `ITfEditSession` 获得焦点 | 从 `ITfContext` + `HWND` + `InputScope` 构建 `EditorFingerprint` → `create` Session |
| 焦点切换控件 | 指纹变化 → `stop` + `create` |
| `InputScope` 为密码 | `PrivacyLevel = ForbiddenCloud` |
| 键盘隐藏 | `stopAll` |

### 7.5 线程模型

| 线程 | 职责 |
|------|------|
| **UI 线程** | TSF 回调、绘制 KeyView、`ime_hot_submit`、读 Arena |
| **Rust IO 线程** | Tokio：`ime_cold_submit`、语言包下载 |
| **冷路径回调** | `PostMessage` / `DispatcherQueue` 回 UI 线程 |

### 7.6 物理键盘

- `Ctrl+Space` / `Win+Space`（系统级）切换 IME；内部 `SwitchScheme` / `ToggleAscii` 走热路径。
- `VK_BACK` → `KeyPress(Backspace)`；候选 `1`–`9` → `SelectCandidate`。

---

## 8. macOS 对接（InputMethodKit）

### 8.1 IMK 架构

```text
ImeApp.app
  IMKServer（main bundle）
    → IMKInputController 子类
    → 候选：IMKCandidates 或自绘 NSPanel
    → ImeBridge.swift → libime_ffi.dylib
```

与 iOS Keyboard Extension **不同**：macOS IME 为 **普通 App 进程**，无 50MB 沙盒上限，可开 `full` Cargo feature。

### 8.2 构建链路

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
./scripts/build-macos.sh   # 产出 universal libime_ffi.dylib 或按 arch 分包
```

`Info.plist` 注册 `InputMethod` server；`ImeApp` 与 IME 通常 **同一 bundle**。

### 8.3 Platform Adapter（Swift）

```swift
final class MacPlatformAdapter {
    func initCore(dataDir: URL) {
        dataDir.path.withCString { ime_core_init($0) }
    }
    func submitHot(editorId: UInt64, action: UserAction) -> HotResult {
        guard ime_session_validate(editorId) != 0 else { return .sessionInvalid }
        var hot = packAction(editorId, action)
        guard ime_hot_submit(&hot) == IME_OK else { return .error }
        return readSnapshot(editorId)
    }
    func execute(_ cmd: UiCommand, controller: IMKInputController) {
        switch cmd {
        case .commit(let text): controller.client()?.insertText(text, replacementRange: ...)
        case .setComposing(let s): controller.client()?.setMarkedText(s, ...)
        }
    }
}
```

### 8.4 Session 与隐私

- `IMKInputController.inputClient()` 文档切换 → 新 `EditorFingerprint`。
- `NSEventModifierFlags` + Secure Input（密码框）→ `ForbiddenCloud`。
- 菜单栏 IME 列表切换语言 → `SwitchLang` 热路径。

### 8.5 语言包路径

`~/Library/Application Support/ImeApp/langpacks/` — 主 App 与 IMK **同进程读写**，无需 App Group。

---

## 9. Linux 对接（IBus / Fcitx5）

### 9.1 双后端策略

| 后端 | 适用场景 | 插件类型 |
|------|----------|----------|
| **IBus** | GNOME、Ubuntu 默认 | `IBusEngine` 动态库，XML 注册 `ibus-ime.xml` |
| **Fcitx5** | KDE、中文社区 | `fcitx5::AddonInstance`，`ime.conf` 描述文件 |

**共享**：`libime_ffi.so` + `common/ime_platform_adapter.cpp`；仅 **上屏与生命周期胶水** 分两套。

### 9.2 构建链路

```bash
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
cargo build -p ime-ffi --release
# IBus 插件链接 libime_ffi.so；安装至 /usr/lib/ibus-ime/ 或 ~/.local/lib/ibus/
```

### 9.3 IBus Engine 要点

```cpp
class ImeIBusEngine : public IBusEngine {
    void focus_in() override {
        editor_id_ = adapter_.OnFocusIn(unique_name_, surrounding_text_);
        bind_ui(editor_id_);
    }
    void focus_out() override { adapter_.OnFocusOut(editor_id_); }
    gboolean process_key_event(uint keyval, uint keycode, uint modifiers) override {
        return adapter_.SubmitKey(editor_id_, keyval, modifiers);
    }
    void commit_text(const std::string& text) {
        ibus_engine_commit_text(engine_, ibus_text_new_from_string(text.c_str()));
    }
};
```

### 9.4 Fcitx5 要点

- 实现 `InputMethodEngineV3`：`keyEvent`、`activate`、`deactivate`。
- 预编辑：`InputContext::updatePreedit`；上屏：`commitString`。
- 与 IBus 共用 `ime_platform_adapter` 的 `submitHot` / Arena 读取。

### 9.5 Wayland / X11

- 焦点由 IBus/Fcitx 框架管理；自绘面板优先 **框架内嵌**（IBus `IBusPanelService`）避免 layer-shell 碎片。
- 多显示器：CandBar 跟随 `InputContext` 屏幕坐标（Fcitx5 `InputPanel` API）。

### 9.6 数据目录

`$XDG_DATA_HOME/ime/langpacks/`（默认 `~/.local/share/ime/langpacks/`）；配置 `$XDG_CONFIG_HOME/ime/settings.toml`。

### 9.7 桌面六端差异对照（节选）

| 项 | Windows | macOS | Linux |
|----|---------|-------|-------|
| UI | Win32 / WinUI | AppKit | GTK / Qt / 框架面板 |
| 胶水 | C++ TSF | Swift IMK | C++ IBus/Fcitx5 |
| Rust 产物 | `.dll` | `.dylib` | `.so` |
| 语言包 | `%LOCALAPPDATA%` | `Application Support` | `$XDG_DATA_HOME` |
| `native_so` OTA | ✓ 同签 | ✓ 同 Team | ✓ 同签 |
| Extension 内存限制 | 无 | 无 | 无 |
| 物理键盘 | 完整 | 完整 | 完整 |

---

## 10. 共享内存 Arena 读取（全平台一致）

Rust 在 `ime_core_init` 时分配 `ImeHotArena`（双缓冲），导出指针：

```rust
// crates/ime-ffi/src/arena.rs
static ARENA: OnceLock<HotArena> = OnceLock::new();

#[no_mangle]
pub extern "C" fn ime_hot_arena_ptr() -> *mut c_void {
    ARENA.get().map(|a| a.ptr()).unwrap_or(std::ptr::null_mut())
}
```

**Android**（Direct ByteBuffer）：

```kotlin
private val arena: ByteBuffer by lazy {
    ByteBuffer.wrap(ByteArray(0)).also { } // 实际：JNI 返回地址后 wrap
    // 或使用 Unsafe / GetDirectBufferAddress
}
fun readSnapshot(editorId: Long): ImmSnapshot {
    val seq = ImeNative.imeHotLatestSeq(editorId)
    if (seq <= lastSeq) return ImmSnapshot.empty
    val header = parseHeader(arena) // 按 ime_hot.h 偏移读 ImeHotHeader
    // ...
}
```

**iOS**：

```swift
func readSnapshot(editorId: UInt64) -> ImmSnapshot {
    let ptr = ime_hot_arena_ptr()
    let header = ptr.assumingMemoryBound(to: ImeHotHeader.self).pointee
    // ...
}
```

**鸿蒙**（ArrayBuffer + NAPI 返回指针或拷贝）：

```typescript
readSnapshot(editorId: bigint): ImmSnapshot {
  const seq = imeNative.imeHotLatestSeq(editorId);
  if (seq <= this.lastSeq) return ImmSnapshot.empty();
  const arenaPtr = imeNative.imeHotArenaPtr(); // BigInt 地址或共享 ArrayBuffer
  const header = parseHeader(arenaPtr);
  // ...
}
```

---

## 11. 冷路径回调注册

Rust IO 线程完成后调用平台回调；**必须在适配层切回 UI 线程**。

```rust
// ime-ffi/src/cold.rs
type ColdCb = extern "C" fn(task_id: i32, editor_id: u64, payload: *const u8, len: usize, err: i32);

#[no_mangle]
pub extern "C" fn ime_cold_submit(
    editor_id: u64, kind: u32, payload: *const u8, len: usize, cb: ColdCb,
) -> i32 {
    ffi_guard(|| {
        let bytes = unsafe { std::slice::from_raw_parts(payload, len) };
        runtime::spawn_cold(editor_id, kind, bytes, move |result| {
            cb(result.task_id, editor_id, result.ptr, result.len, result.err);
        });
        IME_OK
    })
}
```

Android 静态回调 + `Handler`；iOS / macOS 用 `DispatchQueue.main.async`；鸿蒙用 `taskpool` + `EventHandler`；**Windows** `PostMessage` / WinUI `DispatcherQueue`；**Linux** `g_idle_add` / Qt `QMetaObject::invokeMethod` 回 UI 线程。

---

## 12. 分步教程：从零跑通 M0

以下假设已安装 Rust、Android Studio、Xcode、DevEco Studio（鸿蒙），以及 **Windows SDK / macOS Xcode CLT / Linux IBus 开发包**。

### 步骤 0：环境准备

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-linux-android armv7-linux-androideabi
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# Android NDK（通过 Android Studio SDK Manager 安装）
# 环境变量
export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/<version>
cargo install cargo-ndk

# 鸿蒙 OHOS NDK（DevEco SDK 自带 native 目录）
export OHOS_NDK_HOME=/path/to/openharmony/native
rustup target add aarch64-unknown-linux-ohos x86_64-unknown-linux-ohos
# 桌面
rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc
rustup target add aarch64-apple-darwin x86_64-apple-darwin
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
cargo install cbindgen
```

### 步骤 1：创建 workspace 与空实现

```bash
mkdir -p ime-core/crates/ime-ffi/src
cd ime-core
cargo init --name ime-core
# 编辑 Cargo.toml 为 workspace，添加 ime-ffi crate
```

`crates/ime-ffi/src/lib.rs`（最小 smoke）：

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};

#[repr(C)]
pub struct ImeHotAction {
    pub editor_id: u64,
    pub client_seq: u64,
    pub action_type: u32,
    pub key_code: u32,
    pub candidate_id: u32,
    pub flags: u32,
    pub reserved: [u8; 8],
}

pub const IME_OK: i32 = 0;

fn ffi_guard<F: FnOnce() -> i32>(f: F) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => -3,
    }
}

#[no_mangle]
pub extern "C" fn ime_core_init(_data_dir: *const i8) -> i32 {
    ffi_guard(|| IME_OK)
}

#[no_mangle]
pub extern "C" fn ime_hot_submit(_action: *const ImeHotAction) -> i32 {
    ffi_guard(|| IME_OK)
}
```

```bash
cbindgen crates/ime-ffi -o include/ime_hot.h
cargo build -p ime-ffi --release
```

### 步骤 2：Android 链接验证

```bash
cd ime-shell-android
cargo ndk -t arm64-v8a -o ime-native/src/main/jniLibs build -p ime-ffi --release
./gradlew :ime-native:assembleDebug
```

在模拟器安装后，Logcat 应看到 `ime_core_init` 返回 0。

### 步骤 3：iOS 链接验证

```bash
cd ime-core
./scripts/build-ios.sh
# Xcode 打开 ime-shell-ios，Run Keyboard Extension scheme
```

Extension 启动无 crash 即 M0 通过。

### 步骤 3b：鸿蒙链接验证

```bash
cd ime-core && ./scripts/build-ohos.sh
# DevEco 打开 ime-shell-harmonyos，Run ime_extension
```

系统输入法列表中启用后，Hilog 应看到 `ime_core_init` 返回 0。

### 步骤 4：接通热路径（M1）

1. 实现 `ime-session`：`ime_session_validate` / `ime_session_stop`
2. `ime_hot_submit` → `scheduler.handle` → 写 Arena
3. Kotlin / Swift / ArkTS `UiBinder` 读 Arena 刷新 CandBar
4. 接 `InputConnection` / `textDocumentProxy` / `InputClient` 执行 `UiCommand.Commit`

### 步骤 5：接通冷路径（M3+）

1. `ime-data` 启动 Tokio runtime（`std::thread::spawn` 单例）
2. 实现 `ime_cold_submit`：皮肤加载、语言包 install
3. Adapter 层 FlatBuffers 编解码（Java / Swift / ArkTS 生成代码）

---

## 13. 调试与排错

| 现象 | 排查 |
|------|------|
| Android `UnsatisfiedLinkError` | ABI 不匹配；检查 `jniLibs` 与 `abiFilters` |
| iOS `Undefined symbols` | Extension 未链接 `ime_ffi.xcframework`；或 feature 不一致 |
| 热路径卡顿 | `submitHot` 内做了 IO；用 Android Studio CPU Profiler / Instruments |
| Session 错乱 | 回调未校验 `editor_id`；对照架构 1.5 |
| iOS Extension 被杀 | 内存超支；减 feature、unload 非活跃语言包 mmap |
| 鸿蒙 `libime_native.so` 加载失败 | ABI / `module.json5` 未声明 nativeLibrary；检查 `libs/arm64-v8a` |
| 鸿蒙 Extension 无 langpack | 路径未用 `getApplicationContext().filesDir`；主 App 未 install |
| Windows TIP 未出现在列表 | 未 `regsvr32 ime_tip.dll`；TSF 注册表项缺失 |
| macOS IME 未出现在菜单栏 | `Info.plist` 缺 `InputMethod`；未 `RegisterInputSource` |
| Linux IBus 找不到引擎 | `ibus-daemon -drx`；XML 路径未在 `~/.config/ibus/bus/` |
| Fcitx5 插件未加载 | `fcitx5-diagnose`；addon 未安装至 `~/.local/share/fcitx5/addons/` |

| FFI panic | 应被 `ffi_guard` 吃掉；查 Rust 日志 `RUST_LOG=ime_ffi=debug` |

**日志**：Android `android_logger`；iOS / macOS `os_log`；鸿蒙 `hilog`；**Windows** `OutputDebugString` / ETW；**Linux** `journald` / `stderr`；统一 `tracing`。

---

## 14. CI 建议

```yaml
# .github/workflows/rust-ffi.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test -p ime-ffi
      - run: cbindgen crates/ime-ffi -o include/ime_hot.h
      - run: git diff --exit-code include/ime_hot.h   # 头文件与代码同步

  android:
    runs-on: ubuntu-latest
    steps:
      - run: cargo ndk -t arm64-v8a build -p ime-ffi --release

  ios:
    runs-on: macos-latest
    steps:
      - run: ./scripts/build-ios.sh

  harmonyos:
    runs-on: ubuntu-latest
    steps:
      - run: rustup target add aarch64-unknown-linux-ohos
      - run: ./scripts/build-ohos.sh

  windows:
    runs-on: windows-latest
    steps:
      - run: cargo build -p ime-ffi --release --target x86_64-pc-windows-msvc

  macos:
    runs-on: macos-latest
    steps:
      - run: ./scripts/build-macos.sh

  linux:
    runs-on: ubuntu-latest
    steps:
      - run: cargo build -p ime-ffi --release --target x86_64-unknown-linux-gnu
```

---

## 15. 与里程碑对应

| 里程碑 | 对接交付物 |
|--------|-----------|
| **M0** | `ime_hot.h` + 移动三端 + **桌面三端** 链接 smoke |
| **M1** | `ime_hot_submit` + Arena 读候选 + Session validate |
| **M2** | `switchLayout` / `switchLang` 热路径 |
| **M3** | `ime_cold_submit` 换肤回调 |
| **M3.5** | 主 App 冷路径 install 语言包；Extension enable |
| **M4** | AI 冷路径 + 取消 `ime_cold_cancel` |
| **M5.5** | Windows TSF + macOS IMK + Linux IBus 拼音上屏；Fcitx5 插件可选 |

---

## 16. 检查清单（评审用）

- [ ] 平台代码仅依赖 `ime_hot.h`，无第二套 FFI
- [ ] cbindgen 头文件纳入 CI，与 `#[repr(C)]` 同步
- [ ] Android 全目标 ABI 已测；鸿蒙 arm64 + x86_64 模拟器已测
- [ ] iOS Extension / 鸿蒙 ime_extension 使用最小 Cargo feature
- [ ] 热路径无网络、无 SQLite、无 FlatBuffers 解析
- [ ] 冷路径回调已切主线程
- [ ] `editor_id` 在所有回调入口校验
- [ ] Release 开启 LTO（`[profile.release] lto = true`）
- [ ] 语言包路径：iOS App Group / 鸿蒙+Android `applicationContext.filesDir` 一致
- [ ] Windows `regsvr32` / macOS InputSource / Linux IBus XML 注册已文档化
- [ ] 桌面三端 `data_dir` 路径与架构 1.3 一致
- [ ] Extension 只读语言包；主 App 负责 download（**桌面 IME 可自管 download**）

---

## 附录：目录与文件清单

| 路径 | 说明 |
|------|------|
| `ime-core/cbindgen.toml` | 头文件生成配置 |
| `ime-core/include/ime_hot.h` | 生成物，提交 Git |
| `ime-core/scripts/build-ios.sh` | iOS xcframework 脚本 |
| `ime-core/scripts/build-ohos.sh` | 鸿蒙 libime_ffi.so 脚本 |
| `ime-core/scripts/build-macos.sh` | macOS dylib / universal 脚本 |
| `ime-core/scripts/build-windows.ps1` | Windows DLL 脚本 |
| `ime-shell-android/ime-native/` | JNI + jniLibs |
| `ime-shell-ios/Rust/ime_ffi.xcframework` | 构建产物（可 CI 缓存） |
| `ime-shell-ios/ImeKeyboard/Bridge/` | Bridging Header + Swift Adapter |
| `ime-shell-harmonyos/ime_native/` | NAPI + libs + ArkTS 声明 |
| `ime-shell-harmonyos/ime_extension/` | InputMethodExtensionAbility |
| `ime-shell-windows/ime_tip/` | TSF TIP + ime_ffi.dll |
| `ime-shell-macos/` | IMK Server + Swift Bridge |
| `ime-shell-linux/ibus-ime/` | IBus 插件 |
| `ime-shell-linux/fcitx5-ime/` | Fcitx5 插件 |

---

*文档结束*
