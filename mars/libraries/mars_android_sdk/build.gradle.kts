plugins {
    id("com.android.library")
    `maven-publish`
}

// Unlike :libraries:mars_xlog_sdk this module still ships the C++ STN
// libraries, so it packages the prebuilt `libs/<abi>/*.so` produced by
// `build_android.py` (jitpack.yml downloads them from the GitHub release).
//
// Those include the *C++* libmarsxlog.so, which libmarsstn.so links against,
// so mars-core and mars-xlog currently publish two different files under the
// same `jni/<abi>/libmarsxlog.so` path. An app that depends on both gets an
// AGP duplicate-file error; it disappears once the C++ STN is ported too.

/** Reads `gradle.properties` of this module, e.g. `PROJ_ARTIFACTID`. */
fun propertyValue(name: String): String =
    findProperty(name) as? String
        ?: error("${project.path} has no `$name` in its gradle.properties")

group = propertyValue("PROJ_GROUP")
version = propertyValue("PROJ_VERSION") + if (project.hasProperty("release")) "" else "-SNAPSHOT"

android {
    namespace = "com.tencent.mars"
    compileSdk = 36

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
            jniLibs.srcDirs("libs")
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

// The C++ libraries of this module come from build_android.py (or from the
// mars-android-native.zip that jitpack.yml downloads), so a build without them
// used to assemble an AAR whose jni/ directory was simply empty.
val verifyPrebuiltLibraries = tasks.register("verifyPrebuiltLibraries") {
    doLast {
        val missing = android.defaultConfig.ndk.abiFilters.filter { abi ->
            project.file("libs/$abi").listFiles { f -> f.name.endsWith(".so") }.isNullOrEmpty()
        }
        if (missing.isNotEmpty()) {
            throw GradleException(
                "No prebuilt .so for ${missing.sorted()} in ${project.file("libs")}. Run " +
                    "mars/build_android.py, or unzip mars-android-native.zip from the " +
                    "GitHub release the way jitpack.yml does."
            )
        }
    }
}

tasks.named("preBuild") {
    dependsOn(verifyPrebuiltLibraries)
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
