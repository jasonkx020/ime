#!/usr/bin/env bash
# Build static lib for iOS and package xcframework (requires Xcode + rust targets).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
OUT="$REPO_ROOT/yc-shell-ios/Rust/yc_ffi.xcframework"
mkdir -p "$(dirname "$OUT")"

cd "$ROOT"
for triple in aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim; do
  echo "Building $triple ..."
  rustup target add "$triple" 2>/dev/null || true
  cargo build -p yc-ffi --release --target "$triple"
done

# Minimal xcframework layout (M0): copy device static lib; full lipo/xcodebuild in CI later.
DEVICE_LIB="$ROOT/target/aarch64-apple-ios/release/libyc_ffi.a"
mkdir -p "$OUT/ios-arm64"
cp "$DEVICE_LIB" "$OUT/ios-arm64/"
echo "Stub xcframework at $OUT (M0: single-arch static lib)"
"$ROOT/scripts/sync-headers.ps1" 2>/dev/null || powershell -File "$ROOT/scripts/sync-headers.ps1"
