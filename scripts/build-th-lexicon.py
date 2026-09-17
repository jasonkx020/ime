#!/usr/bin/env python3
"""Build assets/langpacks/th-v1/lexicon/th_words.tsv from open Thai frequency data.

Primary source: PyThaiNLP Thai Textbook Corpus (TTC) word frequencies — CC0.
https://github.com/PyThaiNLP/pythainlp (corpus/ttc_freq.txt)

Lookup keys are Thai script (same as word) for Kedmanee prefix matching.

Raw cache: assets/cache/th/ttc_freq.txt (gitignored).
"""
from __future__ import annotations

import re
import sys
import unicodedata
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE_DIR = ROOT / "assets" / "cache" / "th"
CACHE_TXT = CACHE_DIR / "ttc_freq.txt"
OUT = ROOT / "assets" / "langpacks" / "th-v1" / "lexicon" / "th_words.tsv"

# Prefer pythainlp repo copy (stable path under corpus/)
SOURCE_URLS = [
    "https://raw.githubusercontent.com/PyThaiNLP/pythainlp/dev/pythainlp/corpus/ttc_freq.txt",
    "https://raw.githubusercontent.com/PyThaiNLP/pythainlp/master/pythainlp/corpus/ttc_freq.txt",
]

THAI_RE = re.compile(r"^[\u0E00-\u0E7F]+$")

# IME-critical phrases (greetings / UI) — boost even if lower in textbook corpus
IME_BOOST: list[tuple[str, int]] = [
    ("สวัสดี", 49_500),
    ("ขอบคุณ", 49_200),
    ("ขอโทษ", 49_000),
    ("ยินดีต้อนรับ", 48_500),
    ("ภาษาไทย", 48_000),
    ("กรุงเทพ", 47_500),
    ("ประเทศไทย", 47_500),
    ("เข้าสู่ระบบ", 47_000),
    ("ตั้งค่า", 46_500),
    ("ค้นหา", 46_500),
]


def ensure_source() -> Path:
    if CACHE_TXT.exists() and CACHE_TXT.stat().st_size > 10_000:
        return CACHE_TXT
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    last_err: Exception | None = None
    for url in SOURCE_URLS:
        try:
            print(f"Downloading {url} …", flush=True)
            urllib.request.urlretrieve(url, CACHE_TXT)
            if CACHE_TXT.stat().st_size > 10_000:
                return CACHE_TXT
        except Exception as e:  # noqa: BLE001
            last_err = e
            print(f"  failed: {e}", flush=True)
    raise SystemExit(f"Could not download TTC freq list: {last_err}")


def count_to_freq(count: int, rank: int) -> int:
    if count <= 0:
        return 1
    return max(1, 50_000 - min(rank, 49_000))


def parse_ttc(path: Path) -> dict[str, int]:
    """word[ \\t]+count → word → freq."""
    rows: dict[str, int] = {}
    text = path.read_text(encoding="utf-8")
    rank = 0
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # TTC: "word count" (space) or tab
        parts = re.split(r"[\t ]+", line, maxsplit=1)
        if len(parts) < 2:
            continue
        word = unicodedata.normalize("NFC", parts[0].strip())
        try:
            count = int(parts[1].strip().split()[0])
        except ValueError:
            continue
        if not word or not THAI_RE.match(word):
            continue
        # Skip ultra-short noise (single combining marks etc. already filtered)
        if len(word) < 1:
            continue
        rank += 1
        freq = count_to_freq(count, rank)
        prev = rows.get(word)
        if prev is None or freq > prev:
            rows[word] = freq
    return rows


def main() -> int:
    src = ensure_source()
    rows = parse_ttc(src)
    print(f"Parsed {len(rows)} words from {src.name}", flush=True)

    for word, boost in IME_BOOST:
        w = unicodedata.normalize("NFC", word.strip())
        rows[w] = max(rows.get(w, 0), boost)

    items: list[tuple[str, int, str]] = []
    for word, freq in rows.items():
        # Key = Thai word itself (Kedmanee prefix lookup)
        key = word
        items.append((word, freq, key))

    items.sort(key=lambda t: (-t[1], t[0]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8", newline="\n") as f:
        f.write("word\tfreq\tromanized\n")
        for word, freq, key in items:
            f.write(f"{word}\t{freq}\t{key}\n")

    print(f"Wrote {len(items)} entries → {OUT.relative_to(ROOT)}", flush=True)
    sample = [t for t in items if t[0] == "สวัสดี"]
    if sample:
        print(f"  สวัสดี freq={sample[0][1]} key={sample[0][2]}", flush=True)
    else:
        print("WARNING: missing สวัสดี", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
