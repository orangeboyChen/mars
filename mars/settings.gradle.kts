pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
        // Last: a stale artifact in ~/.m2 must not shadow the real one.
        mavenLocal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {
        mavenLocal()
        google()
        mavenCentral()
    }
}

rootProject.name = "mars"

include(":libraries:mars_android_sdk")
include(":libraries:mars_xlog_sdk")
