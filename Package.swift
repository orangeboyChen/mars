// swift-tools-version: 5.9
//
//  Mars - Swift Package Manager distribution (iOS + watchOS)
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
        .iOS(.v12),
        .watchOS(.v9)
    ],
    products: [
        .library(name: "MarsXlog", targets: ["MarsXlog"])
    ],
    targets: [
        // Prebuilt binary: iOS + watchOS, device and simulator slices.
        // Built by mars/build_xcframework.py, see docs/distribution.md.
        .binaryTarget(
            name: "MarsXlogBinary",
            url: "https://github.com/orangeboyChen/mars/releases/download/v2.0.1/MarsXlog.xcframework.zip",
            checksum: "b6980cb8104844dfaddd8d5b41caabd7baf208f937fbce9fa2a8a685b187b735"
        ),
        // Thin Swift wrapper: a binary target can not declare dependencies on
        // system libraries, and the static library needs libc++ and libz, so
        // they are declared here and propagated to every consumer.
        .target(
            name: "MarsXlog",
            dependencies: ["MarsXlogBinary"],
            path: "Sources/MarsXlog",
            linkerSettings: [
                .linkedLibrary("c++"),
                .linkedLibrary("z"),
            ]
        )
    ]
)
