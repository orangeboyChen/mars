# mars — the xlog pipeline, in Rust

This is a fork of [Tencent/mars](https://github.com/Tencent/mars) whose **xlog**
implementation has been rewritten in Rust. The C++ implementation of mars —
xlog, STN, SDT and every vendored dependency it pulled in (OpenSSL, Boost,
zstd, googletest) — has been removed; every artifact this repository ships is
built from the Rust crates in [`rust/`](rust).

Nothing here needs CMake, an NDK toolchain build or a C++ compiler any more:

| artifact | built from |
|---|---|
| `mars-xlog.aar` (Android) | `rust/crates/mars-xlog-jni`, cross-compiled per ABI by `mars/gradle/mars-cargo.gradle.kts` |
| `MarsXlog.xcframework` (Apple) | `rust/crates/mars-xlog-ffi`, cross-compiled by `mars/build_xcframework.py` |
| `mars_xlog_ffi` / `mars_xlog_compat` (C ABI, CLI) | `rust/crates/mars-xlog-ffi`, `rust/crates/mars-xlog-compat` |

## Layout

```
rust/                       the Rust workspace
  crates/mars-xlog-core     block buffer, log file format, zlib/zstd helpers
  crates/mars-xlog-crypt    ECDH + AES-GCM record encryption
  crates/mars-xlog-buffer   mmap append buffer (LogZlibBuffer / LogZstdBuffer)
  crates/mars-xlog-appender the process-wide appender and logger instances
  crates/mars-xlog-ffi      C ABI (cdylib + staticlib) and its hand-written header
  crates/mars-xlog-jni      JNI bindings of com.tencent.mars.xlog.Xlog
  crates/mars-xlog-compat   CLI + golden .xlog fixtures of the wire format
mars/                       the Android Gradle build (Kotlin DSL) and the
                            XCFramework builder
Sources/MarsXlog            the Swift API published by Package.swift
samples/android/xlogSample  a sample app that depends on mars-xlog
```

## Build and test

```bash
cd rust
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

```bash
# Android: cross-compiles the JNI crate for armeabi-v7a / arm64-v8a / x86_64
# and bundles libmarsxlog.so into the AAR (needs an NDK for the linker only)
cd mars && ./gradlew :libraries:mars_xlog_sdk:assembleRelease

# Apple: device + simulator slices of libmars_xlog_ffi.a in an XCFramework
cd mars && python3 build_xcframework.py --zip
```

## The wire format is pinned by golden files

The 16 `.xlog` files in `rust/crates/mars-xlog-compat/fixtures` were produced by
the *original C++* encoders (one per combination of zlib/zstd, sync/async,
encryption on/off and flush policy). `cargo test -p mars-xlog-compat` decodes
every one of them, so the port can still read what the C++ wrote and vice versa
after the C++ itself is gone.

Two differences are known and accepted:

* **zlib** — the port uses `zlib-rs`, which does not emit byte-identical output
  to system zlib for payloads above a few KB. The stream is still decodable by
  both sides.
* **zstd** — the C++ build vendored 1.4.4, the Rust build uses the `zstd` crate
  1.5.x, so compressed sizes differ. Pointing the C++ build at the system zstd
  made the two byte-identical, which is how the port itself was validated.

## Distribution

* Android: `implementation 'com.github.orangeboyChen.mars:mars-xlog:<tag>'`
  through [JitPack](https://jitpack.io), or `mars-xlog.aar` from a release.
* Apple: Swift Package Manager, `Package.swift` → `import MarsXlog`.

See [docs/distribution.md](docs/distribution.md).

## License

MIT, like the upstream project — see [LICENSE](LICENSE).
