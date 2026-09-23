/*
 * Builds `libmarsxlog.so`, the Rust JNI library behind
 * `com.tencent.mars.xlog.Xlog`, and stages it where the Android Gradle Plugin
 * picks up prebuilt native libraries.
 *
 * Apply it from an Android module:
 *
 *     apply(from = rootProject.file("gradle/mars-cargo.gradle.kts"))
 *
 * Every ABI below (which must match the module's `ndk.abiFilters`) is either
 *
 *   * copied from `libs/<abi>/`, the prebuilt libraries that `jitpack.yml`
 *     downloads from the GitHub release, or
 *   * cross-compiled with cargo when the directory holds no `libmarsxlog.so`.
 *
 * JitPack has neither an NDK nor a Rust toolchain, so the prebuilt path keeps
 * the AAR buildable there, while an ordinary checkout always builds the
 * library from the Rust sources next door.
 */

private data class RustTarget(
    /** Rust target triple. */
    val triple: String,
    /** NDK clang driver prefix, e.g. `aarch64-linux-android`. */
    val clangPrefix: String,
)

private val androidAbis: Map<String, RustTarget> = mapOf(
    "armeabi-v7a" to RustTarget("armv7-linux-androideabi", "armv7a-linux-androideabi"),
    "arm64-v8a" to RustTarget("aarch64-linux-android", "aarch64-linux-android"),
    "x86_64" to RustTarget("x86_64-linux-android", "x86_64-linux-android"),
)

/** Android API level the NDK clang driver targets. */
val nativeMinApi: Int = (findProperty("mars.native.minApi") as String?)
    ?.toIntOrNull()
    ?: 21

/** Directory of the Rust workspace that owns `mars-xlog-jni`. */
val rustWorkspace: File = (findProperty("mars.rust.dir") as String?)
    ?.let(::file)
    ?: rootDir.parentFile.resolve("rust")

/** Crate producing `libmarsxlog.so`. */
val rustCrate: String = (findProperty("mars.rust.crate") as String?) ?: "mars-xlog-jni"

/**
 * NDK release the cargo build links with. It is pinned because the linker
 * default changed between releases: NDK 27 emits 4 KiB-aligned ELF, which
 * cannot be `dlopen`ed on the 16 KiB-page devices Android 15 introduced.
 */
val ndkVersion: String = (findProperty("mars.ndk.version") as String?) ?: "27.1.12297006"

val generatedJniLibs: Provider<Directory> = layout.buildDirectory.dir("generated/jniLibs")

val prebuiltAbis: List<String> =
    androidAbis.keys.filter { file("libs/${it}/libmarsxlog.so").isFile }

val cargoAbis: List<String> = (androidAbis.keys - prebuiltAbis.toSet()).toList()

private fun cargoExecutable(): File {
    (findProperty("mars.cargo") as String?)?.let { return file(it) }
    System.getenv("CARGO")?.let { return file(it) }
    val path = System.getenv("PATH").orEmpty().split(File.pathSeparator)
    path.firstNotNullOfOrNull { dir ->
        File(dir, "cargo").takeIf { it.isFile && it.canExecute() }
            ?: File(dir, "cargo.exe").takeIf { it.isFile }
    }?.let { return it }
    return File(System.getProperty("user.home"), ".cargo/bin/cargo")
}

private fun androidSdkDirectory(): File {
    sequenceOf("ANDROID_HOME", "ANDROID_SDK_ROOT")
        .mapNotNull { System.getenv(it) }
        .firstOrNull()
        ?.let { return file(it) }
    val localProperties = rootProject.file("local.properties")
    if (localProperties.isFile) {
        localProperties.readLines()
            .firstOrNull { it.startsWith("sdk.dir=") }
            ?.substringAfter("sdk.dir=")
            ?.replace("\\\\:", ":")
            ?.let { return file(it) }
    }
    throw GradleException(
        "No Android SDK found: set ANDROID_HOME (or ANDROID_SDK_ROOT), or add " +
            "sdk.dir=<path> to ${localProperties}."
    )
}

private fun ndkDirectory(): File {
    System.getenv("ANDROID_NDK_HOME")?.let { return file(it) }
    val ndkRoot = androidSdkDirectory().resolve("ndk")
    ndkRoot.resolve(ndkVersion).takeIf { it.isDirectory }?.let { return it }
    val installed = ndkRoot.listFiles()?.filter { it.isDirectory }?.map { it.name }?.sorted()
    throw GradleException(
        "NDK $ndkVersion is not installed under $ndkRoot (found: ${installed ?: emptyList<String>()}). " +
            "Install it with the SDK manager, set ANDROID_NDK_HOME, or override the " +
            "version with -Pmars.ndk.version=<release>: the linker default that " +
            "decides the page size of libmarsxlog.so differs between releases."
    )
}

private fun ndkBinDirectory(): File {
    val os = System.getProperty("os.name").lowercase()
    val hostTags = when {
        "windows" in os -> listOf("windows-x86_64")
        "mac" in os || "darwin" in os -> listOf("darwin-x86_64", "darwin-arm64")
        else -> listOf("linux-x86_64", "linux-arm64")
    }
    val prebuilt = ndkDirectory().resolve("toolchains/llvm/prebuilt")
    val tag = hostTags.firstOrNull { prebuilt.resolve(it).isDirectory } ?: hostTags.first()
    return prebuilt.resolve(tag).resolve("bin")
}

/** `armv7-linux-androideabi` -> `ARMV7_LINUX_ANDROIDEABI`, used by cargo's per-target env vars. */
private fun cargoKey(triple: String): String = triple.uppercase().replace('-', '_')

/** `armv7-linux-androideabi` -> `armv7_linux_androideabi`, used by the `cc` crate. */
private fun ccKey(triple: String): String = triple.lowercase().replace('-', '_')

/** `e_machine` of the Android ABIs, used to validate a staged library. */
private val elfMachines: Map<String, Int> = mapOf(
    // EM_ARM, EM_AARCH64 and EM_X86_64 of <elf.h>.
    "armeabi-v7a" to 40,
    "arm64-v8a" to 183,
    "x86_64" to 62,
)

/**
 * `e_machine` of [file], or `null` when it is not an ELF file at all.
 *
 * Android is little endian, so the 16-bit field is read that way; every ELF
 * header puts it at offset 18 (`e_type` is the two bytes before it).
 */
private fun elfMachine(file: File): Int? {
    val header = file.inputStream().use { it.readNBytes(20) }
    if (header.size < 20) return null
    if (header[0] != 0x7F.toByte() || header[1] != 'E'.code.toByte()
        || header[2] != 'L'.code.toByte() || header[3] != 'F'.code.toByte()
    ) return null
    return (header[18].toInt() and 0xFF) or ((header[19].toInt() and 0xFF) shl 8)
}

val cargoBuildTasks: List<TaskProvider<Exec>> = cargoAbis.map { abi ->
    val target = androidAbis.getValue(abi)
    val targetDirectory = rustWorkspace.resolve("target/${target.triple}/release")
    val library = targetDirectory.resolve("libmarsxlog.so")
    val cargo = cargoExecutable()

    tasks.register<Exec>("cargoBuild${abi.split('-').joinToString("") { it.replaceFirstChar(Char::uppercase) }}") {
        group = "build"
        description = "Cross-compiles libmarsxlog.so for $abi with cargo ($target.triple)."

        workingDir = rustWorkspace
        executable = cargo.absolutePath
        // Configuration must not depend on the toolchain: `help`, `tasks` and
        // an IDE sync have to work on a machine without Rust, so the check
        // runs here instead of while the task is registered.
        doFirst {
            check(cargo.isFile) {
                "cargo not found at $cargo: install Rust (https://rustup.rs) or set -Pmars.cargo=<path>."
            }
        }
        args(
            // `cargo rustc`, not `cargo build`: the flags after `--` reach only
            // the final crate of this target. `RUSTFLAGS` (and even
            // CARGO_TARGET_<triple>_RUSTFLAGS, which an exported `RUSTFLAGS`
            // outranks) would also reach the *host* units — build scripts and
            // proc-macros — and Apple's ld64 rejects `-Wl,-z` outright, so a
            // cold cargo build would fail on macOS while CI stayed green.
            "rustc",
            "--locked",
            "--release",
            "--package", rustCrate,
            "--target", target.triple,
            "--",
            // Android 15 devices can use 16 KiB pages and refuse to load an
            // ELF whose segments are 4 KiB aligned, which is what lld emits by
            // default on the NDK releases CI installs; mars/CMakeLists.txt
            // forced the same values for the C++ build.
            "-C", "link-arg=-Wl,-z,max-page-size=16384",
            "-C", "link-arg=-Wl,-z,common-page-size=16384",
        )

        // cargo reads the per-target linker from CARGO_TARGET_<TRIPLE>_LINKER
        // and the `cc` crate (used by every C dependency) from CC/AR/CXX with
        // the same target prefix. The NDK clang driver is a wrapper that
        // already selects the sysroot, so pointing all three at it is enough.
        val ndkBin = ndkBinDirectory()
        val clang = "$ndkBin/${target.clangPrefix}${nativeMinApi}-clang"
        val cc = ccKey(target.triple)
        environment("CARGO_TARGET_${cargoKey(target.triple)}_LINKER", clang)
        environment("CC_$cc", clang)
        environment("CXX_$cc", "$ndkBin/${target.clangPrefix}${nativeMinApi}-clang++")
        environment("AR_$cc", "$ndkBin/llvm-ar")
        environment("PATH", "$ndkBin${File.pathSeparator}${System.getenv("PATH")}")

        inputs.dir(rustWorkspace.resolve("crates"))
        inputs.file(rustWorkspace.resolve("Cargo.toml"))
        inputs.file(rustWorkspace.resolve("Cargo.lock"))
        inputs.property("minApi", nativeMinApi)
        inputs.property("ndk", ndkBin.absolutePath)
        outputs.file(library)
    }
}

val generateJniLibs = tasks.register<Sync>("generateJniLibs") {
    group = "build"
    description = "Stages libmarsxlog.so for every ABI into the module's jniLibs directory."

    dependsOn(cargoBuildTasks)

    prebuiltAbis.forEach { abi ->
        from(file("libs/$abi")) {
            include("libmarsxlog.so")
            into(abi)
        }
    }
    cargoAbis.forEach { abi ->
        val target = androidAbis.getValue(abi)
        from(rustWorkspace.resolve("target/${target.triple}/release")) {
            include("libmarsxlog.so")
            into(abi)
        }
    }

    into(generatedJniLibs)

    doLast {
        val root = generatedJniLibs.get().asFile
        val missing = androidAbis.keys.filter { !root.resolve("$it/libmarsxlog.so").isFile }
        if (missing.isNotEmpty()) {
            throw GradleException(
                "libmarsxlog.so is missing for ${missing.joinToString()}. " +
                    if (System.getenv("JITPACK") == "true") {
                        "JitPack has no Rust toolchain: jitpack.yml downloads " +
                            "mars-android-native.zip from the GitHub release before the " +
                            "build, check its 'before_install' step."
                    } else {
                        "Build it with cargo, or place the prebuilt libraries in " +
                            "${file("libs")}/<abi>/."
                    }
            )
        }

        // A prebuilt library is copied verbatim, so make sure it really is an
        // Android shared object of this ABI — a stale or misplaced file would
        // otherwise be shipped without a single warning.
        androidAbis.forEach { (abi, _) ->
            val library = root.resolve("$abi/libmarsxlog.so")
            val machine = elfMachine(library)
            if (machine != null && machine != elfMachines.getValue(abi)) {
                throw GradleException(
                    "$library is an ELF for machine $machine, expected " +
                        "${elfMachines.getValue(abi)} ($abi)."
                )
            }
            if (machine == null) {
                logger.lifecycle("$library is not an ELF file, skipped the header check")
            }
        }
    }
}

// Published for the applying module, which adds the directory to the AGP
// `jniLibs` source set. AGP rejects a `Provider` there, so this is a plain
// `File`.
extra["mars.jniLibsDir"] = generatedJniLibs.get().asFile

// `preBuild` runs before every packaging task, so the staged libraries are in
// place when AGP merges the jniLibs directories into the AAR.
tasks.named("preBuild") {
    dependsOn(generateJniLibs)
}
