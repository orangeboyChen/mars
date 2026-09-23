plugins {
    id("com.android.library")
    `maven-publish`
}

// Builds `libmarsxlog.so` from the Rust workspace and stages it in
// `build/generated/jniLibs`, which AGP merges into the AAR.
apply(from = rootProject.file("gradle/mars-cargo.gradle.kts"))

/** Reads `gradle.properties` of this module, e.g. `PROJ_ARTIFACTID`. */
fun propertyValue(name: String): String =
    findProperty(name) as? String
        ?: error("${project.path} has no `$name` in its gradle.properties")

group = propertyValue("PROJ_GROUP")
version = propertyValue("PROJ_VERSION") + if (project.hasProperty("release")) "" else "-SNAPSHOT"

android {
    namespace = "com.tencent.mars.xlog"
    compileSdk = 36

    // The NDK cargo links with, see gradle/mars-cargo.gradle.kts: the linker
    // default that decides the page size of libmarsxlog.so differs between
    // releases, so AGP has to strip with the same one.
    ndkVersion = "27.1.12297006"
    defaultConfig {
        // 21, not the 19 of the C++ build: the Rust standard library for the
        // Android targets requires API 21.
        minSdk = 21
        ndk {
            abiFilters += listOf("armeabi-v7a", "arm64-v8a", "x86_64")
        }
    }

    sourceSets {
        named("main") {
            @Suppress("DEPRECATION")
            jniLibs.srcDirs((project.extra.get("mars.jniLibsDir") as File))
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }

    buildTypes {
        named("release") {
            proguardFile("proguard-rules.pro")
        }
    }
}


afterEvaluate {
    // The packaged ABIs must match what gradle/mars-cargo.gradle.kts builds
    // (mars-core: what build_android.py produces): shipping an AAR that misses
    // one of them is worse than failing here.
    val packaged = android.defaultConfig.ndk.abiFilters.sorted()
    val built = listOf("arm64-v8a", "armeabi-v7a", "x86_64")
    check(packaged == built) {
        "${project.path} packages $packaged but the native build produces $built."
    }
}

publishing {
    publications {
        register<MavenPublication>("release") {
            groupId = project.group.toString()
            artifactId = propertyValue("PROJ_ARTIFACTID")
            version = project.version.toString()

            pom {
                name.set(propertyValue("PROJ_NAME"))
                description.set(propertyValue("PROJ_DESCRIPTION"))
                url.set(propertyValue("PROJ_WEBSITEURL"))
            }

            // `components["release"]` only exists once AGP has created the
            // variants, which happens after this script has been evaluated.
            afterEvaluate {
                from(components.named("release").get())
            }
        }
    }
}
