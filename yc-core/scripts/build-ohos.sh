#!/usr/bin/env bash
# Build yc_ffi for OpenHarmony (requires aarch64-unknown-linux-ohos toolchain).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
OUT="$REPO_ROOT/yc-shell-harmonyos/yc_native/libs/arm64-v8a"
TARGET="aarch64-unknown-linux-ohos"

cd "$ROOT"
if ! rustup target list --installed | grep -q "$TARGET"; then
  echo "WARN: $TARGET not installed. Install OHOS Rust toolchain first."
  echo "See yc-shell-harmonyos/README.md"
  exit 0
fi

cargo build -p yc-ffi --release --target "$TARGET"
mkdir -p "$OUT"
cp "$ROOT/target/$TARGET/release/libyc_ffi.so" "$OUT/"
powershell -File "$ROOT/scripts/sync-headers.ps1" 2>/dev/null || true
echo "Copied libyc_ffi.so -> $OUT"
