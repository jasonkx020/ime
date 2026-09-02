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
    }
}

rootProject.name = "yc-shell-android"
include(":app", ":yc-native", ":yc-ui-android")
project(":yc-ui-android").projectDir = file("../yc-ui-android")
