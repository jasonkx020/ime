# yc-shell-linux

Linux IBus and Fcitx5 input method shells for YC IME (M0 scaffold).

## Prerequisites

- CMake 3.20+
- GCC or Clang (x86_64)
- Rust `yc-ffi` built for `x86_64-unknown-linux-gnu`
- Optional dev packages: `libibus-1.0-dev`, `fcitx5-dev` (stubs build without them)

## Build `yc_ffi`

```bash
cd yc-core
cargo build -p yc-ffi --release --target x86_64-unknown-linux-gnu
mkdir -p ../yc-shell-linux/libs/x64
cp target/x86_64-unknown-linux-gnu/release/libyc_ffi.so ../yc-shell-linux/libs/x64/
./scripts/sync-headers.ps1   # or copy include/yc_hot.h manually
```

## Configure and build

```bash
cd yc-shell-linux
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

Outputs:

- `build/lib/ibus-yc.so`
- `build/lib/fcitx5-yc.so`

## M0 smoke — IBus

```bash
export LD_LIBRARY_PATH="$PWD/libs/x64:$PWD/build/lib:$LD_LIBRARY_PATH"
# Loading the module runs the constructor smoke test:
python3 -c "import ctypes; ctypes.CDLL('./build/lib/ibus-yc.so')"
```

Expect stderr: `[ibus-yc] M0 smoke rc=0`.

To install for IBus (M1+): copy `ibus-yc.so` and XML descriptor to the IBus component path, then restart `ibus-daemon`.

## M0 smoke — Fcitx5

```bash
export LD_LIBRARY_PATH="$PWD/libs/x64:$PWD/build/lib:$LD_LIBRARY_PATH"
python3 -c "import ctypes; ctypes.CDLL('./build/lib/fcitx5-yc.so')"
```

Expect stderr: `[fcitx5-yc] M0 smoke rc=0`.

To install for Fcitx5 (M1+): place the addon under `~/.local/share/fcitx5/addons/` and run `fcitx5-diagnose`.

## Layout

```text
yc-shell-linux/
  CMakeLists.txt
  libs/x64/libyc_ffi.so
  common/
    yc_platform_adapter.{h,cpp}
    include/yc_hot.h
  ibus-yc/ibus_engine.c
  fcitx5-yc/fcitx5_addon.cpp
```
