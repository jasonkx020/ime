# ime-pack ranking data

Static tables used by `build-zh-lexicon` (compile-time only; not shipped in the hot path).

| File | Source | Role |
|------|--------|------|
| `common_chars.txt` | Jun Da modern Chinese frequency (via hanziDB.csv) | Rank common chars for `freq` |
| `t2s_map.txt` | OpenCC `TSCharacters.txt` (Apache-2.0) | Traditional → simplified demotion |

Do not edit casually; regenerate from upstream if updating the ranking policy.
