plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "com.yc.input"
    compileSdk = libs.versions.compileSdk.get().toInt()

    defaultConfig {
        minSdk = libs.versions.minSdk.get().toInt()

        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }

        externalNativeBuild {
            cmake {
                cppFlags += "-std=c++17"
                arguments += listOf(
                    "-DYC_JNI_LIBS_DIR=${projectDir}/src/main/jniLibs",
                )
            }
        }
    }

    externalNativeBuild {
        cmake {
            path = file("src/main/jni/CMakeLists.txt")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    androidResources {
        noCompress += listOf("onnx", "yml")
    }
}

dependencies {
    implementation("androidx.annotation:annotation:1.9.1")
    implementation(project(":yc-ui-android"))
    // Official PaddleOCR Android SDK (ONNX Runtime, PP-OCRv6)
    // api so OpenCV/ONNX .so propagate into the app APK.
    api(project(":ppocr-sdk"))
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
}
