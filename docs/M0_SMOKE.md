# M0 六端 Smoke 验收

> 各端 M0 目标：`yc_core_init` → `yc_session_begin` → `yc_hot_submit(Init)` 成功，无 crash。

## 通用（Rust）

```bash
cd yc-core
cargo test --workspace
cargo build -p yc-ffi --release
cargo build -p yc-ffi --features full
```

```powershell
.\scripts\sync-headers.ps1
.\scripts\build-desktop.ps1
```

根目录：

```powershell
.\scripts\build-all.ps1
```

## yc-cli（桌面 REPL，已具备 M2/M2.5）

```bash
cd yc-core && cargo run -p yc-cli
```

## Android — [yc-shell-android/README.md](../yc-shell-android/README.md)

1. `yc-core\scripts\build-android-ndk.ps1`（需 Android NDK + rust android targets）
2. `.\gradlew :app:assembleDebug`
3. Logcat：`YcImeService` / `YcNative` 日志中 init 返回 0

## iOS — [yc-shell-ios/README.md](../yc-shell-ios/README.md)

1. `yc-core/scripts/build-ios-xcframework.sh`
2. Xcode / xcodegen 打开工程，Run Extension scheme
3. 无 crash 即通过

## 鸿蒙 — [yc-shell-harmonyos/README.md](../yc-shell-harmonyos/README.md)

1. 安装 `aarch64-unknown-linux-ohos` toolchain
2. `yc-core/scripts/build-ohos.sh`
3. DevEco 同步；Hilog 见 init 成功

## Windows — [yc-shell-windows/README.md](../yc-shell-windows/README.md)

1. `build-desktop.ps1` 复制 `yc_ffi.dll` 到 `libs/x64/`
2. `cmake -B build && cmake --build build`
3. 加载 `yc_tip` DLL，日志见 `yc_platform_smoke`

## macOS — [yc-shell-macos/README.md](../yc-shell-macos/README.md)

1. 链接 `libs/libyc_ffi.dylib`（由 desktop 脚本或手动复制）
2. `swift build`（Package.swift）
3. 运行 `YcInputServer`，见 smoke 日志

## Linux — [yc-shell-linux/README.md](../yc-shell-linux/README.md)

1. 复制 `libyc_ffi.so` 到 `libs/x86_64/`
2. `cmake -B build && cmake --build build`
3. IBus/Fcitx5 插件 constructor 中 smoke 日志

## 头文件同步

`yc-core/scripts/sync-headers.ps1` 将 [`yc_hot.h`](../yc-core/include/yc_hot.h) 拷贝至六端 `include/` 目录，提交前应运行以保持 ABI 一致。
