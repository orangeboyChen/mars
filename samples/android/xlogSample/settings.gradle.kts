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
        // `mars-xlog` comes from `./gradlew -Prelease publishToMavenLocal` in
        // mars/, or from JitPack when it is consumed as
        // com.github.orangeboyChen.mars:mars-xlog:<tag>.
        mavenLocal()
        google()
        mavenCentral()
    }
}

rootProject.name = "xlogSample"

include(":app")
