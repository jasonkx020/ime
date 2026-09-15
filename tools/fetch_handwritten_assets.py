#!/usr/bin/env python3
"""Fetch Ismantic/Handwritten NCNN assets into Android app assets."""
from __future__ import annotations

import os
import ssl
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "platforms/yc-shell-android/app/src/main/assets/models/handwriting"
BASE = "https://huggingface.co/Ismantic/Handwritten/resolve/main"

FILES = {
    "ncnn/model.ncnn.param": "model.ncnn.param",
    "ncnn/model.ncnn.bin": "model.ncnn.bin",
    "ncnn/charset.json": "charset.json",
}


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    ctx = ssl.create_default_context()
    for src, dst in FILES.items():
        url = f"{BASE}/{src}"
        out = ASSETS / dst
        print(f"GET {url} -> {out}")
        urllib.request.urlretrieve(url, out)
        print(f"  {out.stat().st_size} bytes")


if __name__ == "__main__":
    main()
