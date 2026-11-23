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

// The Rust core (yrs 0.28 via uniffi 0.32) is a local, git-ignored
// `lib/yniffiFFI.xcframework`. Build it first with
// `scripts/build-xcframework.sh` (static libs cross-compiled on a Linux host,
// assembled on the Mac); without it SwiftPM fails to resolve the `yniffiFFI`
// binary target. This fork publishes no release asset.
let FFIbinaryTarget: PackageDescription.Target = .binaryTarget(
    name: "yniffiFFI",
    path: "lib/yniffiFFI.xcframework"
)

let package = Package(
    name: "YSwift",
    platforms: [.iOS(.v13)],
    products: [
        .library(name: "YSwift", targets: ["YSwift"]),
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
