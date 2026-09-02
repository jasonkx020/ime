# Build yc-ffi for host + sync headers + fixture packs (M3/M3.5).
$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$YcCore = Join-Path $RepoRoot "yc-core"
$FixturesDist = Join-Path $RepoRoot "fixtures\dist"

Push-Location $YcCore
try {
    $env:CARGO_HTTP_CHECK_REVOKE = "false"

    Write-Host "==> Build zh lexicon (100k sample + dat)"
    & (Join-Path $RepoRoot "scripts\build-zh-lexicon.ps1")

    Write-Host "==> Build fixture langpack + skin"
    New-Item -ItemType Directory -Force -Path $FixturesDist | Out-Null
    $ImePack = Join-Path $RepoRoot "tools\ime-pack\Cargo.toml"
    foreach ($pack in @("vi-v1", "th-v1", "zh-pack-v1")) {
        cargo run --manifest-path $ImePack -- build `
            -o (Join-Path $FixturesDist "$pack.imepack") `
            (Join-Path $RepoRoot "fixtures\langpacks\$pack")
    }
    cargo run --manifest-path $ImePack -- build-skin `
        -o (Join-Path $FixturesDist "samsung-light.imeskin") `
        (Join-Path $RepoRoot "fixtures\skins\samsung-light")

    Write-Host "==> cargo test --workspace"
    cargo test --workspace
    Write-Host "==> cargo build -p yc-ffi --release"
    cargo build -p yc-ffi --release
    Write-Host "==> cargo build -p yc-ffi --features full"
    cargo build -p yc-ffi --features full
    & (Join-Path $YcCore "scripts\sync-headers.ps1")
    & (Join-Path $YcCore "scripts\build-desktop.ps1")
    Write-Host "M3/M3.5 build-all complete."
} finally {
    Pop-Location
}
