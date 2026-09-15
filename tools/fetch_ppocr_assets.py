#!/usr/bin/env python3
"""Fetch PP-OCRv6_tiny ONNX assets for Android handwriting (official ppocr-sdk)."""
from __future__ import annotations

import io
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "platforms/yc-shell-android/app/src/main/assets/models/ppocr"
BOS = "https://paddle-model-ecology.bj.bcebos.com/paddlex/official_inference_model/paddle3.0.0"

# Mobile-friendly tier for IME handwriting.
MODELS = {
    "det": f"{BOS}/PP-OCRv6_tiny_det_onnx_infer.tar",
    "rec": f"{BOS}/PP-OCRv6_tiny_rec_onnx_infer.tar",
}


def download_tar(url: str) -> tarfile.TarFile:
    print(f"GET {url}")
    data = urllib.request.urlopen(url, timeout=300).read()
    print(f"  downloaded {len(data)} bytes")
    return tarfile.open(fileobj=io.BytesIO(data), mode="r:*")


def extract_onnx_dir(tf: tarfile.TarFile, dest: Path, want_yml: bool) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    members = [m for m in tf.getmembers() if m.isfile()]
    onnx = next((m for m in members if m.name.endswith("inference.onnx")), None)
    if onnx is None:
        raise RuntimeError("inference.onnx not found in archive")
    with tf.extractfile(onnx) as src, open(dest / "inference.onnx", "wb") as out:
        out.write(src.read())
    print(f"  -> {dest / 'inference.onnx'} ({(dest / 'inference.onnx').stat().st_size} bytes)")
    if want_yml:
        yml = next((m for m in members if m.name.endswith("inference.yml")), None)
        if yml is None:
            raise RuntimeError("inference.yml not found in rec archive")
        with tf.extractfile(yml) as src, open(dest / "inference.yml", "wb") as out:
            out.write(src.read())
        print(f"  -> {dest / 'inference.yml'} ({(dest / 'inference.yml').stat().st_size} bytes)")


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    # Remove legacy Paddle-Lite .nb layout if present
    for name in ("det.nb", "cls.nb", "rec.nb"):
        p = ASSETS / name
        if p.exists():
            p.unlink()
            print(f"removed legacy {p}")
    labels = ASSETS / "labels"
    if labels.exists():
        for f in labels.rglob("*"):
            if f.is_file():
                f.unlink()
        try:
            labels.rmdir()
        except OSError:
            pass

    extract_onnx_dir(download_tar(MODELS["det"]), ASSETS / "det", want_yml=False)
    extract_onnx_dir(download_tar(MODELS["rec"]), ASSETS / "rec", want_yml=True)
    print("done: PP-OCRv6_tiny ONNX assets ready")


if __name__ == "__main__":
    main()
