pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {
        google()
        mavenCentral()
        // `mars-xlog` comes from `./gradlew -Prelease publishToMavenLocal` in
        // mars/, or from JitPack when it is consumed as
        // com.github.orangeboyChen.mars:mars-xlog:<tag>. Kept last so a stale
        // copy in ~/.m2 can not shadow the real artifact.
        mavenLocal()
    }
}

rootProject.name = "xlogSample"

include(":app")
