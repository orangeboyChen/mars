// swift-tools-version: 5.9
//
//  Mars — Swift Package Manager distribution (Apple platforms)
//
//  Usage:
//      .package(url: "https://github.com/orangeboyChen/mars", from: "0.1.0")
//
//  The package ships a prebuilt MarsXlog.xcframework (binary target) built by
//  .github/workflows/release-apple.yml, so no CMake / OpenSSL toolchain is
//  needed on the consumer side.
//
//  Swift:
//      import MarsXlog
//      MarsXlog.open(logDir: dir, namePrefix: "Test")
//      MarsXlog.log(level: .info, tag: "demo", message: "hello")
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
        .binaryTarget(
            name: "MarsXlog",
            url: "https://github.com/orangeboyChen/mars/releases/download/v0.1.0/MarsXlog.xcframework.zip",
            checksum: "0000000000000000000000000000000000000000000000000000000000000000"
        )
    ]
)
