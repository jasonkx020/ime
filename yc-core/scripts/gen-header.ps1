# Generate yc_hot.h from Rust FFI definitions.
# Requires: cargo install cbindgen
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not (Get-Command cbindgen -ErrorAction SilentlyContinue)) {
    Write-Host "cbindgen not found; using hand-maintained include/yc_hot.h"
    exit 0
}

cbindgen crates/yc-ffi -c cbindgen.toml -o include/yc_hot.h
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Generated include/yc_hot.h"
