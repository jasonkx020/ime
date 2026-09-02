# yc-shell-android (M0)

Android shell for YC Input Method. Package `com.yc.input`.

## Prerequisites

- Android Studio Ladybug+ or JDK 17 + Android SDK (API 35)
- Android NDK (for `yc-native` JNI)
- Rust toolchain with Android targets (for real `libyc_ffi.so`)

## Build `libyc_ffi.so`

From the repo root, build Rust FFI for all Android ABIs and sync headers:

```powershell
cd yc-core
.\scripts\build-android-ndk.ps1
```

This copies:

- `libyc_ffi.so` → `yc-native/src/main/jniLibs/<abi>/`
- `yc_hot.h` → `yc-native/src/main/jniLibs/include/yc_hot.h`

Without a prebuilt `libyc_ffi.so`, Gradle still compiles using **stub FFI symbols** embedded in `libyc_jni.so` (see `yc_jni.cpp` / `CMakeLists.txt`).

## Build APK

```powershell
cd yc-shell-android
.\gradlew :app:assembleDebug
```

Output: `app/build/outputs/apk/debug/app-debug.apk`

## Enable IME on device

1. Install the APK.
2. Settings → System → Languages & input → On-screen keyboard → enable **YC Input**.
3. Open any text field and switch to YC Input.

`YcImeService` calls `YcNative.smoke()` on create to exercise `yc_core_init` → `yc_session_begin` → `yc_hot_submit`.

## Module layout

| Module | Role |
|--------|------|
| `:app` | Launcher activity (`com.yc.input.MainActivity`) |
| `:yc-native` | `InputMethodService`, JNI glue (`libyc_jni.so`), prebuilt `libyc_ffi.so` |

## JNI

Kotlin `external` methods in `com.yc.input.native.YcNative` are implemented in `yc-native/src/main/jni/yc_jni.cpp` and forward to the C ABI in `yc_hot.h`.
