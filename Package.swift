// swift-tools-version: 5.9
//
//  Mars - Swift Package Manager distribution (iOS)
//
//      .package(url: "https://github.com/orangeboyChen/mars", from: "0.1.2")
//
//  and then
//
//      import MarsXlog
//
//      let config = MarsXlogOpenConfig()
//      config.logDir = logDir
//      config.cacheDir = cacheDir
//      config.namePrefix = "Ham"
//      config.pubKey = "..."
//      MarsXlog.open(config)
//
//      MarsXlog.info(module: "Net", function: #function, message: "hello")
//      MarsXlog.flush()
//
//  The package ships a prebuilt MarsXlog.xcframework built by
//  .github/workflows/release.yml, so consumers need neither CMake nor OpenSSL.
//
import PackageDescription

let package = Package(
    name: "mars",
    platforms: [
        .iOS(.v12)
    ],
    products: [
        .library(name: "MarsXlog", targets: ["MarsXlog"])
    ],
    targets: [
        // Prebuilt binary: ios-arm64 + ios-arm64_x86_64-simulator.
        .binaryTarget(
            name: "MarsXlogBinary",
            url: "https://github.com/orangeboyChen/mars/releases/download/v0.1.2/MarsXlog.xcframework.zip",
            checksum: "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        // Swift API over the C ABI of the Rust crate mars-xlog-ffi. A binary
        // target can not declare dependencies on system libraries, so they
        // would be declared here — the Rust static library needs none: not
        // libc++, because there is no C++ left, and not libz, because the
        // zlib implementation is Rust too.
        .target(
            name: "MarsXlog",
            dependencies: ["MarsXlogBinary"],
            path: "Sources/MarsXlog"
        )
    ]
)
