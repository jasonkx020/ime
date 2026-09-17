#!/usr/bin/env python3
"""Build assets/langpacks/vi-v1/lexicon/vi_words.tsv from open Vietnamese frequency data.

Primary source: tabidots/vn-freqs (Leipzig Corpora + OpenSubtitles2016, pyvi-tokenized).
https://github.com/tabidots/vn-freqs

Raw cache: assets/cache/vi/vn_word_frequencies.tsv (gitignored).
"""
from __future__ import annotations

import re
import sys
import unicodedata
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CACHE_DIR = ROOT / "assets" / "cache" / "vi"
CACHE_TSV = CACHE_DIR / "vn_word_frequencies.tsv"
OUT = ROOT / "assets" / "langpacks" / "vi-v1" / "lexicon" / "vi_words.tsv"
SOURCE_URL = (
    "https://raw.githubusercontent.com/tabidots/vn-freqs/main/vn_word_frequencies.tsv"
)

# Vietnamese letters + space / hyphen (standard orthography)
VI_WORD_RE = re.compile(
    r"^[A-Za-zÀ-ỹĂăÂâÊêÔôƠơƯưĐđ]+(?:[ \-][A-Za-zÀ-ỹĂăÂâÊêÔôƠơƯưĐđ]+)*$"
)

# IME-critical phrases: ensure high rank even if rare in movie/news corpora
IME_BOOST: list[tuple[str, int]] = [
    ("xin chào", 49_500),
    ("chào bạn", 49_000),
    ("chào anh", 48_800),
    ("chào chị", 48_800),
    ("cảm ơn", 49_200),
    ("cảm ơn bạn", 48_500),
    ("xin lỗi", 49_100),
    ("tạm biệt", 48_500),
    ("hẹn gặp lại", 48_000),
    ("làm ơn", 47_500),
    ("tiếng việt", 47_000),
    ("đăng nhập", 46_500),
    ("đăng ký", 46_000),
    ("cài đặt", 46_000),
    ("tìm kiếm", 46_000),
]


def romanize(s: str) -> str:
    buf: list[str] = []
    for ch in s.replace("đ", "d").replace("Đ", "d"):
        for c in unicodedata.normalize("NFD", ch):
            if unicodedata.category(c) != "Mn" and c.isascii() and c.isalnum():
                buf.append(c.lower())
    return "".join(buf)


def ensure_source() -> Path:
    if CACHE_TSV.exists() and CACHE_TSV.stat().st_size > 10_000:
        return CACHE_TSV
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    print(f"Downloading {SOURCE_URL} …", flush=True)
    urllib.request.urlretrieve(SOURCE_URL, CACHE_TSV)
    return CACHE_TSV


def count_to_freq(count: int, rank: int) -> int:
    """Map corpus count/rank to a stable IME freq (higher = better, fits u32 ngram sums)."""
    if count <= 0:
        return 1
    # Prefer rank so top ~20k stay under ~50k and ngram sums do not saturate quickly.
    return max(1, 50_000 - min(rank, 49_000))


def parse_vn_freqs(path: Path) -> dict[str, int]:
    """rank \\t count \\t word \\t pos → word → best freq."""
    rows: dict[str, int] = {}
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        try:
            rank = int(parts[0])
            count = int(parts[1])
        except ValueError:
            continue
        word = unicodedata.normalize("NFC", parts[2].strip())
        if not word or not VI_WORD_RE.match(word):
            continue
        # Prefer lowercase surface for non-proper mixed case (keep Việt Nam etc.)
        if word[:1].isupper() and not any(c.isupper() for c in word[1:]):
            # Single capital start → common sentence form; normalize to lower
            word = word[0].lower() + word[1:]
        freq = count_to_freq(count, rank)
        prev = rows.get(word)
        if prev is None or freq > prev:
            rows[word] = freq
    return rows


def main() -> int:
    src = ensure_source()
    rows = parse_vn_freqs(src)
    print(f"Parsed {len(rows)} words from {src.name}", flush=True)

    for word, boost in IME_BOOST:
        w = unicodedata.normalize("NFC", word.strip())
        rows[w] = max(rows.get(w, 0), boost)

    # Drop empty romanized keys
    items: list[tuple[str, int, str]] = []
    for word, freq in rows.items():
        key = romanize(word)
        if not key:
            continue
        items.append((word, freq, key))

    items.sort(key=lambda t: (-t[1], t[2], t[0]))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with OUT.open("w", encoding="utf-8", newline="\n") as f:
        f.write("word\tfreq\tromanized\n")
        for word, freq, key in items:
            f.write(f"{word}\t{freq}\t{key}\n")

    print(f"Wrote {len(items)} entries → {OUT.relative_to(ROOT)}", flush=True)
    # Sanity: xin chào must be present and high
    sample = [t for t in items if t[0] == "xin chào"]
    if sample:
        print(f"  xin chào freq={sample[0][1]} key={sample[0][2]}", flush=True)
    else:
        print("WARNING: missing xin chào", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
