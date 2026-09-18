# en-v1 lexicon

## Files

| File | Notes |
|------|-------|
| `en_words.tsv` | High-frequency English + IME phrases (`word \\t freq \\t pinyin`) |

## Generate

```powershell
python .\scripts\build-en-lexicon.py
```

Then rebuild the pack:

```powershell
cargo run --manifest-path .\tools\ime-pack\Cargo.toml -- build `
  -o .\assets\dist\en-v1.imepack .\assets\langpacks\en-v1
Copy-Item .\assets\dist\en-v1.imepack* .\assets\langpacks\dist\ -Force
Copy-Item .\assets\dist\en-v1.imepack* .\platforms\yc-shell-android\app\src\main\assets\langpacks\ -Force
```
