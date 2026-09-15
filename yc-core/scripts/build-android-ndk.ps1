# Build yc_ffi for Android ABIs via cargo (requires NDK + rust android targets).
# Lang-pack cold path needs `--features full` (data + plugin); without it yc_cold_submit returns -2.
#
# ring/cc-rs need NDK CC/AR (not host clang.exe). Linker alone is not enough for armeabi-v7a asm.

param(
    # Default arm64 only; pass -Abis arm64-v8a,armeabi-v7a,x86_64 for all.
    [string[]]$Abis = @("arm64-v8a")
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$RepoRoot = Split-Path -Parent $Root
$OutBase = Join-Path $RepoRoot "platforms\yc-shell-android\yc-native\src\main\jniLibs"
$Api = 24

if (-not $env:ANDROID_NDK_HOME) {
    $sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "D:\ProgramData\Android\Sdk" }
    $ndkRoot = Join-Path $sdk "ndk"
    if (Test-Path $ndkRoot) {
        $latest = Get-ChildItem $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
        if ($latest) { $env:ANDROID_NDK_HOME = $latest.FullName }
    }
}
if (-not $env:ANDROID_NDK_HOME -or -not (Test-Path $env:ANDROID_NDK_HOME)) {
    throw "ANDROID_NDK_HOME is not set or invalid"
}

$NDK = Join-Path $env:ANDROID_NDK_HOME "toolchains\llvm\prebuilt\windows-x86_64\bin"
$Clang = Join-Path $NDK "clang.exe"
$ClangXx = Join-Path $NDK "clang++.exe"
$LlvmAr = Join-Path $NDK "llvm-ar.exe"
foreach ($tool in @($Clang, $ClangXx, $LlvmAr)) {
    if (-not (Test-Path $tool)) { throw "NDK tool missing: $tool" }
}

function Set-AndroidTargetEnv {
    param(
        [string]$RustTarget,
        [string]$TriplePrefix
    )
    $clangCmd = Join-Path $NDK "$TriplePrefix$Api-clang.cmd"
    $clangXxCmd = Join-Path $NDK "$TriplePrefix$Api-clang++.cmd"
    if (-not (Test-Path $clangCmd)) {
        throw "NDK clang wrapper missing: $clangCmd"
    }

    $envKey = $RustTarget.Replace("-", "_").ToUpperInvariant()
    Set-Item -Path "Env:CARGO_TARGET_${envKey}_LINKER" -Value $clangCmd

    # cc-rs / ring: both underscore and hyphen forms are consulted
    $ccKeyUnderscore = "CC_" + $RustTarget.Replace("-", "_")
    $arKeyUnderscore = "AR_" + $RustTarget.Replace("-", "_")
    $cxxKeyUnderscore = "CXX_" + $RustTarget.Replace("-", "_")
    Set-Item -Path "Env:$ccKeyUnderscore" -Value $clangCmd
    Set-Item -Path "Env:$arKeyUnderscore" -Value $LlvmAr
    Set-Item -Path "Env:$cxxKeyUnderscore" -Value $clangXxCmd

    Set-Item -Path "Env:CC_$RustTarget" -Value $clangCmd
    Set-Item -Path "Env:AR_$RustTarget" -Value $LlvmAr
    Set-Item -Path "Env:CXX_$RustTarget" -Value $clangXxCmd
}

Push-Location $Root
try {
    $meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $targetDir = $meta.target_directory

    foreach ($abi in $Abis) {
        $pair = switch ($abi) {
            "arm64-v8a" { @{ Target = "aarch64-linux-android"; Prefix = "aarch64-linux-android" } }
            "armeabi-v7a" { @{ Target = "armv7-linux-androideabi"; Prefix = "armv7a-linux-androideabi" } }
            "x86_64" { @{ Target = "x86_64-linux-android"; Prefix = "x86_64-linux-android" } }
            default { throw "Unknown ABI: $abi" }
        }
        Set-AndroidTargetEnv -RustTarget $pair.Target -TriplePrefix $pair.Prefix

        Write-Host "Building $($pair.Target) (features=full) ..."
        $ccEnv = "CC_$($pair.Target.Replace('-', '_'))"
        Write-Host "  $ccEnv=$([Environment]::GetEnvironmentVariable($ccEnv))"
        cargo build -p yc-ffi --release --features full --target $pair.Target
        $lib = Join-Path $targetDir "$($pair.Target)\release\libyc_ffi.so"
        if (-not (Test-Path $lib)) {
            throw "libyc_ffi.so not found at $lib"
        }
        $destDir = Join-Path $OutBase $abi
        New-Item -ItemType Directory -Path $destDir -Force | Out-Null
        Copy-Item -Force $lib (Join-Path $destDir "libyc_ffi.so")
        Write-Host "Copied -> $destDir"
    }
    & (Join-Path $PSScriptRoot "sync-headers.ps1")

    $androidAssets = Join-Path $RepoRoot "platforms\yc-shell-android\app\src\main\assets\langpacks"
    $dist = Join-Path $RepoRoot "assets\dist"
    if (Test-Path $dist) {
        New-Item -ItemType Directory -Path $androidAssets -Force | Out-Null
        foreach ($pack in @("zh-pack-v1.imepack", "vi-v1.imepack", "th-v1.imepack")) {
            $src = Join-Path $dist $pack
            if (Test-Path $src) {
                Copy-Item -Force $src (Join-Path $androidAssets $pack)
                Write-Host "Assets <- $pack"
            }
        }
    }
} finally {
    Pop-Location
}
