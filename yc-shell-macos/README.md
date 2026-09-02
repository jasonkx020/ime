# yc-shell-macos

macOS 壳工程（M0）：`YcInputServer` 可执行文件骨架，后续接入 InputMethodKit。

## 构建 Rust 库

```bash
cd ../yc-core
cargo build -p yc-ffi --release
```

同步头文件：

```powershell
./scripts/sync-headers.ps1
```

## Swift 包（M0 smoke）

```bash
swift build
swift run YcInputServer
```

M0 仅验证 Swift 侧可声明并调用 `yc_core_init`；链接 `libyc_ffi` 需在 Xcode / CMake 目标中配置 `LIBRARY_SEARCH_PATHS`。

## 目录

```text
yc-shell-macos/
  Sources/YcInputServer/   # YcBridge.swift, YcServer.swift
  Bridge/yc_hot.h          # C ABI 头（sync-headers 覆盖）
```

数据目录：`~/Library/Application Support/YcInput/`（`yc_core_init` 品牌根，不含 `ycpacks/`）。
