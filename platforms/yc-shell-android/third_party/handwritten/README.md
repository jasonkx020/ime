# Handwritten (Ismantic) — local HCCR for YC IME

Vendored pieces from [Ismantic/Handwritten](https://github.com/Ismantic/Handwritten) (Apache-2.0):

- `HCCRRecognizer.java` → `yc-native/.../com/shiyu/handwritten/runtime/`
- `hccr_jni.cc` + `preprocess.c/h` → `yc-native/src/main/jni/`
- NCNN INT8 weights → `app/src/main/assets/models/handwriting/`

NCNN Android prebuilt (Tencent ncnn 20240820) lives in `third_party/ncnn-android/{abi}/`.

## Refresh models

```bash
python tools/fetch_handwritten_assets.py
```

Do not redistribute CASIA HWDB training data.
