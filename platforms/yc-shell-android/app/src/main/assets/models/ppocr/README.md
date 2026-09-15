# PP-OCRv6 assets (Android handwriting)

Place under `app/src/main/assets/models/ppocr/`:

- `det/inference.onnx` — detection
- `rec/inference.onnx` + `rec/inference.yml` — recognition (+ character dict)

Default tier: **PP-OCRv6_tiny** (mobile / IME). Refresh:

```bash
python tools/fetch_ppocr_assets.py
```

Engine: official `ppocr-sdk` (ONNX Runtime). Paths configured in `PaddleOcrEngine`.
