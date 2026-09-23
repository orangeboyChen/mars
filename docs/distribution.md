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

MarsXlog.open(logDir: logDir, namePrefix: "Test")
MarsXlog.setLevel(.debug)
MarsXlog.log(level: .info, tag: "demo", message: "hello mars")
MarsXlog.flush()
```

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

1. `cd mars && python3 build_xcframework.py --zip` (locally, to get the checksum)
   or push a tag and read the checksum from the workflow summary.
2. Update `checksum:` in `Package.swift`.
3. `git tag vX.Y.Z && git push origin vX.Y.Z`.

`.github/workflows/release-apple.yml` builds the xcframework on a macOS runner,
zips it and attaches `MarsXlog.xcframework.zip` to the GitHub release. It also
prints the checksum and warns when `Package.swift` is out of date. Release
assets are never rebuilt: if the tag already has the artefact the job is a
no-op, which keeps the checksum in `Package.swift` valid.

## Android — JitPack

`jitpack.yml` publishes the two library modules. JitPack has an Android SDK but
no NDK, so the native libraries built by
`.github/workflows/release-android.yml` are downloaded from the release of the
same tag before the AARs are assembled.

```gradle
allprojects {
    repositories {
        mavenCentral()
        maven { url "https://jitpack.io" }
    }
}

dependencies {
    implementation 'com.github.orangeboyChen.mars:mars_xlog_sdk:v0.1.0'
    // or, for the full mars (stn + sdt + xlog):
    implementation 'com.github.orangeboyChen.mars:mars_android_sdk:v0.1.0'
}
```

The exact module names are listed on <https://jitpack.io> when you look up
`orangeboyChen/mars`.

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
