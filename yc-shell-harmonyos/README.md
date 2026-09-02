# yc-shell-harmonyos

HarmonyOS NEXT 壳工程（M0）：`InputMethodExtensionAbility` + NAPI 薄封装，转发 `yc_core_init`。

## 构建 Rust cdylib

```bash
../yc-core/scripts/build-ohos.sh
```

需要 `aarch64-unknown-linux-ohos` 工具链（DevEco Studio SDK native 目录）。产物：

```text
yc_native/libs/arm64-v8a/libyc_ffi.so
```

同步头文件：

```powershell
../yc-core/scripts/sync-headers.ps1
```

## NAPI 模块

`yc_native/napi/yc_napi.cpp` 编译为 `libyc_native.so`，链接 `libyc_ffi.so`。ArkTS 侧：

```typescript
import ycNative from 'libyc_native.so';
ycNative.ycCoreInit(context.filesDir);
```

CMake / hvigor 集成在 M1 补全；M0 仅保留源文件骨架。

## OHOS 环境变量（示例）

```bash
export OHOS_NDK_HOME=/path/to/openharmony/native
export CC_aarch64_unknown_linux_ohos=$OHOS_NDK_HOME/llvm/bin/aarch64-unknown-linux-ohos-clang
export CXX_aarch64_unknown_linux_ohos=$OHOS_NDK_HOME/llvm/bin/aarch64-unknown-linux-ohos-clang++
```

## 目录

```text
yc-shell-harmonyos/
  entry/              # 主 App (EntryAbility)
  yc_extension/       # InputMethodExtensionAbility + YcNative.ets
  yc_native/          # NAPI + libyc_ffi.so + include/yc_hot.h
```

数据目录：`{applicationContext.filesDir}`（品牌根，不含 `ycpacks/`）。
