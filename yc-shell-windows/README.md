# yc-shell-windows

Windows TSF Text Input Processor (TIP) shell for YC IME (M0 scaffold).

## Prerequisites

- CMake 3.20+
- MSVC (x64)
- Rust `yc-ffi` built for `x86_64-pc-windows-msvc`

## Build `yc_ffi`

From the repo root:

```powershell
cd yc-core
cargo build -p yc-ffi --release
.\scripts\build-desktop.ps1
.\scripts\sync-headers.ps1
```

`build-desktop.ps1` copies `yc_ffi.dll` into `libs/x64/`. Also copy the import library:

```powershell
Copy-Item -Force yc-core\target\release\yc_ffi.dll.lib libs\x64\
```

## Configure and build `yc_tip`

```powershell
cd yc-shell-windows
cmake -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
```

Output: `build/bin/Release/yc_tip.dll` (with `yc_ffi.dll` copied alongside).

## M0 smoke

Load `yc_tip.dll` (e.g. `rundll32 build\bin\Release\yc_tip.dll,yc_tsfp_stub`) and check DebugView for:

```text
[yc_tip] M0 smoke rc=0
```

`rc=0` means `yc_core_init` → `yc_session_begin` → `yc_session_validate` succeeded.

## Layout

```text
yc-shell-windows/
  CMakeLists.txt
  libs/x64/yc_ffi.dll          # from build-desktop.ps1
  libs/x64/yc_ffi.dll.lib      # MSVC import lib
  yc_tip/
    yc_platform_adapter.{h,cpp}
    yc_tsfp.cpp
    include/yc_hot.h           # synced by sync-headers.ps1
```
