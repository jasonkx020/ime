# Third-party notices (handwriting / OCR)

## PaddleOCR Android SDK (`ppocr-sdk`)
- Source: https://github.com/PaddlePaddle/PaddleOCR (`deploy/ppocr-android/ppocr-sdk`)
- License: Apache License 2.0
- Used for: Android ONNX Runtime OCR pipeline (PP-OCRv6)

## ONNX Runtime Android
- `com.microsoft.onnxruntime:onnxruntime-android:1.21.1`
- Used for: on-device inference of PP-OCRv6 ONNX models

## OpenCV Android
- `com.quickbirdstudios:opencv:4.5.3.0`
- Used for: image preprocess / crop in ppocr-sdk

## Models
- PP-OCRv6_tiny det/rec ONNX (`inference.onnx` + rec `inference.yml`)
- Download: `python tools/fetch_ppocr_assets.py`
- BOS: paddle-model-ecology PP-OCRv6_tiny_*_onnx_infer.tar

paddleocr4android / Paddle-Lite / Handwritten (NCNN) have been removed from this Android shell.
