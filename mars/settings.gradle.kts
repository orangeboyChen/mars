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
        google()
        mavenCentral()
        // Last: a stale artifact in ~/.m2 must not shadow the real one.
        mavenLocal()
    }
}

rootProject.name = "mars"

// mars-core (the C++ STN library) used to be here too; the repository now
// ships mars-xlog only, which is built from the Rust workspace.
include(":libraries:mars_xlog_sdk")
