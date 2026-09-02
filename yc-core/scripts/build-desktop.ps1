# Build yc_ffi for Windows x64 (MSVC).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$RepoRoot = Split-Path -Parent $Root

Push-Location $Root
try {
    cargo build -p yc-ffi --release
    $meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $targetDir = $meta.target_directory
    $dll = Join-Path $targetDir "release\yc_ffi.dll"
    $lib = Join-Path $targetDir "release\yc_ffi.dll.lib"
    if (-not (Test-Path $dll)) {
        throw "yc_ffi.dll not found at $dll"
    }
    $DestDir = Join-Path $RepoRoot "yc-shell-windows\libs\x64"
    New-Item -ItemType Directory -Path $DestDir -Force | Out-Null
    Copy-Item -Force $dll (Join-Path $DestDir "yc_ffi.dll")
    if (Test-Path $lib) {
        Copy-Item -Force $lib (Join-Path $DestDir "yc_ffi.dll.lib")
    }
    $LinuxDest = Join-Path $RepoRoot "yc-shell-linux\libs\x86_64"
    Write-Host "Copied $dll -> $DestDir"
} finally {
    Pop-Location
}
