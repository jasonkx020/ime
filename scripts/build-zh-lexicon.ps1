# Build zh-pack-v1 lexicon from open-source pinyin data (real Chinese words).
# Sources: mozillazg/phrase-pinyin-data (MIT), mozillazg/pinyin-data (MIT), THUOCL (MIT).
$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$LexDir = Join-Path $RepoRoot "fixtures\langpacks\zh-pack-v1\lexicon"
$CacheDir = Join-Path $RepoRoot "fixtures\cache\pinyin"
$ThuoclDir = Join-Path $RepoRoot "fixtures\cache\thuocl"
$Sample = Join-Path $LexDir "zh_words.sample.tsv"
$OutTsv = Join-Path $LexDir "zh_words.tsv"
$CoreTsv = Join-Path $LexDir "zh_words.core.tsv"
$OutDat = Join-Path $LexDir "zh_words.dat"
$ImePack = Join-Path $RepoRoot "tools\ime-pack\Cargo.toml"

New-Item -ItemType Directory -Force -Path $LexDir, $CacheDir, $ThuoclDir | Out-Null

$PhraseUrl = "https://raw.githubusercontent.com/mozillazg/phrase-pinyin-data/master/pinyin.txt"
$CharUrl = "https://raw.githubusercontent.com/mozillazg/pinyin-data/master/pinyin.txt"
$PhraseFile = Join-Path $CacheDir "phrase.txt"
$CharFile = Join-Path $CacheDir "char.txt"

function Ensure-Download($Url, $Dest) {
    if (-not (Test-Path $Dest) -or (Get-Item $Dest).Length -lt 1024) {
        Write-Host "    download $Url"
        Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
    }
}

Write-Host "==> Fetch pinyin sources (cached under fixtures/cache)"
Ensure-Download $PhraseUrl $PhraseFile
Ensure-Download $CharUrl $CharFile

$ThuoclFiles = @(
    "THUOCL_life.txt", "THUOCL_IT.txt", "THUOCL_car.txt", "THUOCL_chengyu.txt",
    "THUOCL_diming.txt", "THUOCL_lishimingren.txt", "THUOCL_poem.txt",
    "THUOCL_medical.txt", "THUOCL_animal.txt", "THUOCL_food.txt"
)
foreach ($f in $ThuoclFiles) {
    $dest = Join-Path $ThuoclDir $f
    if (-not (Test-Path $dest)) {
        $url = "https://raw.githubusercontent.com/thunlp/THUOCL/master/data/$f"
        try {
            Write-Host "    download THUOCL $f"
            Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
        } catch {
            Write-Host "    skip THUOCL $f ($($_.Exception.Message))"
        }
    }
}

Write-Host "==> Build zh lexicon TSV + core + dat"
$env:CARGO_HTTP_CHECK_REVOKE = "false"
Push-Location $RepoRoot
try {
    cargo run --manifest-path $ImePack -- build-zh-lexicon `
        --phrase-pinyin $PhraseFile `
        --char-pinyin $CharFile `
        --sample-tsv $Sample `
        --thuocl-dir $ThuoclDir `
        --output $OutTsv `
        --core-output $CoreTsv `
        --core-limit 8000 `
        --dat-output $OutDat
} finally {
    Pop-Location
}

Write-Host "    zh_words.tsv  -> $OutTsv"
Write-Host "    zh_words.core.tsv (committed subset) -> $CoreTsv"
Write-Host "    zh_words.dat  -> $OutDat"
