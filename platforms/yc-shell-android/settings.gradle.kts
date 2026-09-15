pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven(url = "https://jitpack.io")
    }
}

rootProject.name = "yc-shell-android"
include(":app", ":yc-native", ":yc-ui-android", ":ppocr-sdk")
project(":yc-ui-android").projectDir = file("../yc-ui-android")
project(":ppocr-sdk").projectDir = file("third_party/ppocr-android/ppocr-sdk")
