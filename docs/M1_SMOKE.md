# M1 六端 Smoke 验收

> M1 目标：拼音热路径组词候选 + Session 隔离 + Samsung 皮肤 + 上屏。

## Rust 回归

```bash
cd yc-core
cargo test --workspace
```

Arena 现含 `YcUiCommandSlot`（Commit/SetComposing 等），各端 parser 需与之对齐。

## 一键构建

```powershell
.\scripts\build-all.ps1
```

## Android

1. `yc-core\scripts\build-android-ndk.ps1`（真机 libyc_ffi.so）
2. Android Studio 打开 `yc-shell-android`，Sync & Run
3. 启用 YC Input，输入 `nihao` → 点候选「你好」→ 上屏
4. Logcat：`YcImeService`

## Windows

```powershell
yc-core\scripts\build-desktop.ps1
cmake -S yc-shell-windows -B yc-shell-windows\build
cmake --build yc-shell-windows\build --config Release
```

DebugView：`[yc_tip] M1 smoke rc=0 commit=你好`

## Linux

复制 `libyc_ffi.so` → `yc-shell-linux/libs/x86_64/`，CMake 构建后加载 ibus-yc / fcitx5-yc，stderr 见 M1 commit 日志。

## iOS

1. `yc-core/scripts/build-ios-xcframework.sh`
2. Xcode / xcodegen 运行 Keyboard Extension
3. `YcKeyboardViewController` + `SamsungKeyboardView`

## macOS

```bash
cd yc-shell-macos && swift build
.build/debug/YcInputServer   # 打印 M1 commit
```

## 鸿蒙

DevEco 打开 `yc-shell-harmonyos`；Hilog 见 `commit:` 日志。

## 验收清单

| 项 | 方式 |
|----|------|
| `nihao` → 「你好」 | 六端 E2E |
| Session 隔离 | 切换输入框后旧 editor_id validate 失败 |
| 皮肤 | 对照 KEYBOARD_UI_DESIGN §11.1 Token |
| P95 ≤16ms | submit→arena 读（无 IO） |
