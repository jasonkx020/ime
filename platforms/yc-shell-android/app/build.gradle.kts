plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "com.yc.input"
    compileSdk = libs.versions.compileSdk.get().toInt()

    defaultConfig {
        applicationId = "com.yc.input"
        minSdk = libs.versions.minSdk.get().toInt()
        targetSdk = libs.versions.targetSdk.get().toInt()
        versionCode = 1
        versionName = "0.1.0-m1"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    // PP-OCRv6 ONNX models must stay uncompressed for reliable AssetManager access.
    androidResources {
        noCompress += listOf("onnx", "yml")
    }
}

dependencies {
    implementation(project(":yc-native"))
    implementation(project(":yc-ui-android"))
    // Explicit so native .so are always packaged (nested library api can be flaky).
    implementation("com.microsoft.onnxruntime:onnxruntime-android:1.21.1")
    implementation("com.quickbirdstudios:opencv:4.5.3.0")
}
