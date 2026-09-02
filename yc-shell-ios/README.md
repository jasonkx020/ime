# yc-shell-ios

iOS 壳工程（M0）：主 App + Keyboard Extension 骨架，经 Swift Bridging 调用 `yc-ffi` C ABI。

## 构建 Rust 静态库

```bash
../yc-core/scripts/build-ios-xcframework.sh
```

产物：`Rust/yc_ffi.xcframework`（M0 为单架构 stub，CI 后续补全 `xcodebuild -create-xcframework`）。

同步 C 头文件：

```powershell
../yc-core/scripts/sync-headers.ps1
```

## Xcode 工程（可选 xcodegen）

```bash
brew install xcodegen   # 或 mint install yonaskolb/xcodegen
xcodegen generate       # 读取 project.yml
open YcInput.xcodeproj
```

在 Xcode 中为 **YcApp** 与 **YcKeyboard** 均链接 `Rust/yc_ffi.xcframework`（静态库，Embed: Do Not Embed）。

## 目录

```text
yc-shell-ios/
  YcApp/           # 主 App（语言包商店、设置）
  YcKeyboard/      # Keyboard Extension + YcBridge.swift
  Rust/            # yc_ffi.xcframework（构建脚本输出）
  project.yml      # xcodegen 最小配置
```

`yc_core_init(data_dir)` 的 `data_dir` 指向 App Group 品牌根目录（不含 `ycpacks/`），见 `docs/SOURCE_NAMING_CONVENTIONS.md`。
