// swift-tools-version:5.6

import PackageDescription
import Foundation

var globalSwiftSettings: [PackageDescription.SwiftSetting] = []

// Only enable if Swift 5.7+ is available and the environment variable `LOCALDEV` is
// set to a value (such as 'true')
#if swift(>=5.7)
    if ProcessInfo.processInfo.environment["YSWIFT_LOCAL"] != nil {
        /*
        Summation from https://www.donnywals.com/enabling-concurrency-warnings-in-xcode-14/
        Set `strict-concurrency` to `targeted` to enforce Sendable and actor-isolation
        checks in your code. This explicitly verifies that `Sendable` constraints are
        met when you mark one of your types as `Sendable`.

        This mode is essentially a bit of a hybrid between the behavior that's intended
        in Swift 6, and the default in Swift 5.7. Use this mode to have a bit of
        checking on your code that uses Swift concurrency without too many warnings
        and / or errors in your current codebase.

        Set `strict-concurrency` to `complete` to get the full suite of concurrency
        constraints, essentially as they will work in Swift 6.
        */
        globalSwiftSettings.append(.unsafeFlags(["-Xfrontend", "-strict-concurrency=complete"]))
    }
#endif

let FFIbinaryTarget: PackageDescription.Target
// The Rust core (yrs 0.28 via uniffi 0.32) ships as `yniffiFFI.xcframework`,
// built by `scripts/build-xcframework.sh` (cross-compiled on a Linux host,
// assembled on the Mac). If that build output is present next to this manifest
// (a local checkout, or a consumer using `.package(path:)`), use it. Otherwise
// fall back to a release asset.
//
// The release URL below is a placeholder: no release of this fork exists yet.
// The checksum is the one `build-xcframework.sh` prints for the matching zip.
let packageDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
let localXCFramework = "lib/yniffiFFI.xcframework"
if ProcessInfo.processInfo.environment["YSWIFT_LOCAL"] != nil
    || FileManager.default.fileExists(atPath: "\(packageDir)/\(localXCFramework)") {
    FFIbinaryTarget = .binaryTarget(
            name: "yniffiFFI",
            path: localXCFramework
    )
} else {
    FFIbinaryTarget = .binaryTarget(
            name: "yniffiFFI",
            url: "https://github.com/Russell0308/yswift/releases/download/0.3.0/yniffiFFI.xcframework.zip",
            checksum: "e020ff389f37698fefb5e8c960556810df5e2b8cd8a1e19c78e888f281b3fde5"
    )
}

let package = Package(
    name: "YSwift",
    platforms: [.iOS(.v13), .macOS(.v10_15)],
    products: [
        .library(name: "YSwift", targets: ["YSwift"]),
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-docc-plugin", from: "1.1.0"),
    ],
    targets: [
        FFIbinaryTarget,
        .target(
            name: "Yniffi",
            dependencies: ["yniffiFFI"],
            path: "lib/swift/scaffold"
        ),
        .target(
            name: "YSwift",
            dependencies: ["Yniffi"],
            swiftSettings: globalSwiftSettings
        ),
        .testTarget(
            name: "YSwiftTests",
            dependencies: ["YSwift"]
        ),
    ]
)
