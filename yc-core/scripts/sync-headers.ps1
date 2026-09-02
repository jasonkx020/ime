# Sync yc_hot.h + yc_layout.h to all six shell include directories.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$RepoRoot = Split-Path -Parent $Root

$HeaderPairs = @(
    @("yc_hot.h", @(
        (Join-Path $RepoRoot "yc-shell-android\yc-native\src\main\jniLibs\include\yc_hot.h"),
        (Join-Path $RepoRoot "yc-shell-ios\YcKeyboard\Bridge\yc_hot.h"),
        (Join-Path $RepoRoot "yc-shell-harmonyos\yc_native\include\yc_hot.h"),
        (Join-Path $RepoRoot "yc-shell-windows\yc_tip\include\yc_hot.h"),
        (Join-Path $RepoRoot "yc-shell-macos\Bridge\yc_hot.h"),
        (Join-Path $RepoRoot "yc-shell-linux\common\include\yc_hot.h")
    )),
    @("yc_layout.h", @(
        (Join-Path $RepoRoot "yc-shell-android\yc-native\src\main\jniLibs\include\yc_layout.h"),
        (Join-Path $RepoRoot "yc-shell-ios\YcKeyboard\Bridge\yc_layout.h"),
        (Join-Path $RepoRoot "yc-shell-harmonyos\yc_native\include\yc_layout.h"),
        (Join-Path $RepoRoot "yc-shell-windows\yc_tip\include\yc_layout.h"),
        (Join-Path $RepoRoot "yc-shell-macos\Bridge\yc_layout.h"),
        (Join-Path $RepoRoot "yc-shell-linux\common\include\yc_layout.h")
    ))
)

foreach ($pair in $HeaderPairs) {
    $src = Join-Path $Root "include\$($pair[0])"
    foreach ($dst in $pair[1]) {
        $dir = Split-Path -Parent $dst
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
        Copy-Item -Force $src $dst
        Write-Host "Synced -> $dst"
    }
}
