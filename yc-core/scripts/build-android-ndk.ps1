# Build yc_ffi for Android ABIs via cargo (requires NDK + rust android targets).
param(
    [string[]]$Abis = @("arm64-v8a", "armeabi-v7a", "x86_64")
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$RepoRoot = Split-Path -Parent $Root
$OutBase = Join-Path $RepoRoot "yc-shell-android\yc-native\src\main\jniLibs"

Push-Location $Root
try {
    $meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $targetDir = $meta.target_directory

    foreach ($abi in $Abis) {
        $target = switch ($abi) {
            "arm64-v8a" { "aarch64-linux-android" }
            "armeabi-v7a" { "armv7-linux-androideabi" }
            "x86_64" { "x86_64-linux-android" }
            default { throw "Unknown ABI: $abi" }
        }
        Write-Host "Building $target ..."
        cargo build -p yc-ffi --release --target $target
        $lib = Join-Path $targetDir "$target\release\libyc_ffi.so"
        if (-not (Test-Path $lib)) {
            throw "libyc_ffi.so not found at $lib"
        }
        $destDir = Join-Path $OutBase $abi
        New-Item -ItemType Directory -Path $destDir -Force | Out-Null
        Copy-Item -Force $lib (Join-Path $destDir "libyc_ffi.so")
        Write-Host "Copied -> $destDir"
    }
    & (Join-Path $PSScriptRoot "sync-headers.ps1")
} finally {
    Pop-Location
}
