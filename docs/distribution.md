# Distribution

## iOS / Apple platforms — Swift Package Manager

`Package.swift` in the repository root exposes the **xlog** module as a
prebuilt `MarsXlog.xcframework` binary target:

```swift
// Package.swift (consumer)
.package(url: "https://github.com/orangeboyChen/mars", from: "0.1.0")
```

```swift
import MarsXlog

let config = MarsXlogOpenConfig()
config.logDir = logDir
config.cacheDir = cacheDir
config.namePrefix = "Ham"
config.pubKey = "..."
config.level = .debug
config.consoleLogEnabled = true
MarsXlog.open(config)
MarsXlog.setExcludedFromBackup(true, forPath: logDir)

MarsXlog.info(module: "Net", function: #function, message: "hello")
MarsXlog.log(level: .error, module: "Net", function: #function, message: "boom")
MarsXlog.flush()
MarsXlog.close()
```

The API mirrors what app code does with `mars::xlog` directly: `XLogConfig`
(`logDir` / `cacheDir` / `namePrefix` / `pubKey`), `appender_open/close/
flush/flushSync`, `xlogger_SetLevel` and `appender_set_console_log`.

`Package.swift` has one wrapper target on top of the binary target: a binary
target cannot declare system library dependencies, and the static library needs
`libc++` and `libz`, so they are declared with `linkerSettings` on the wrapper
and propagated to consumers.

Slices:

| slice | architectures |
|---|---|
| `ios-arm64` | device arm64 |
| `ios-arm64_x86_64-simulator` | simulator arm64 (Apple Silicon) + x86_64 (Intel) |

> A fat library cannot hold a device arm64 and a simulator arm64 slice at the
> same time, which is why the artefact is an `.xcframework` and each platform
> variant is built separately.

Because the framework is a binary target, no CMake / OpenSSL toolchain is
needed on the consumer side. `MarsXlog.h` is an Objective-C wrapper of
`mars::xlog`, so Swift does not need C++ interop.

### Releasing a new version

1. Run **Actions → Release → Run workflow** and pick `major` / `minor` /
   `patch`, or type the version directly (`1.2.3` and `v1.2.3` both work).
   The workflow builds everything, creates the `vX.Y.Z` tag and the release,
   and opens a pull request that commits the matching `url` / `checksum` into
   `Package.swift`.
2. The default branch is protected (no direct pushes), so the `Package.swift`
   change always lands through that pull request on a
   `chore/package-swift-vX.Y.Z` branch. The tag and the release are published
   from the branch commit, so `from: "X.Y.Z"` resolves the new `Package.swift`
   even before the PR is merged; merging only keeps the default branch in sync.

`.github/workflows/release.yml` builds the xcframework (macOS runner) and the
Android AARs (Linux runner),
zips it and attaches `MarsXlog.xcframework.zip` to the GitHub release. It also
prints the checksum and warns when `Package.swift` is out of date. Release
assets are never rebuilt: if the tag already has the artefact the job is a
no-op, which keeps the checksum in `Package.swift` valid.

## Android — JitPack

`jitpack.yml` publishes the two library modules. JitPack has an Android SDK but
no NDK, so the native libraries built by
`.github/workflows/release.yml` are downloaded from the release of the same
tag before the AARs are assembled.

```gradle
allprojects {
    repositories {
        mavenCentral()
        maven { url "https://jitpack.io" }
    }
}

dependencies {
    implementation 'com.github.orangeboyChen.mars:mars-xlog:v0.0.2'
    // or, for the full mars (stn + sdt + xlog):
    implementation 'com.github.orangeboyChen.mars:mars-core:v0.0.2'
}
```

The published artifact ids are `mars-xlog` and `mars-core` (they come from
`artifactId` in the module build files, not from the Gradle project names).

### Without JitPack

Every release also contains the built artefacts, so they can be used directly:

- `mars-xlog.aar`
- `mars-core.aar`
- `mars-android-native.zip` — `libmarsxlog.so` / `libmarsstn.so` /
  `libc++_shared.so` for `armeabi-v7a`, `arm64-v8a` and `x86_64`

### Building the native libraries

```bash
export NDK_ROOT=$ANDROID_HOME/ndk/27.1.12297006
cd mars
python3 build_android.py v0.1.0 armeabi-v7a arm64-v8a x86_64            # mars-core
python3 build_android.py v0.1.0 --xlog-only armeabi-v7a arm64-v8a x86_64 # mars-xlog
```

32-bit `x86` is not built: the vendored OpenSSL in `mars/openssl` has no
`opensslconf_android-x86.h`.
