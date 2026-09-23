plugins {
    id("com.android.application")
}

// Keep in sync with PROJ_VERSION in mars/libraries/mars_xlog_sdk/gradle.properties.
val xlogVersion = "1.2.6"

android {
    namespace = "com.tencent.mars.xlogsample"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.tencent.mars.xlogsample"
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildTypes {
        named("release") {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
}

dependencies {
    // 1.8.0 raises its own minSdk to 23; 1.7.1 still matches the 21 of
    // the mars-xlog AAR.
    implementation("androidx.appcompat:appcompat:1.7.1")
    // Ships libmarsxlog.so for every ABI, so the sample needs no NDK at all.
    implementation("com.tencent.mars:mars-xlog:$xlogVersion")
}
