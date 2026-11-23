# YSwift

This library builds on top of [Yrs](https://github.com/y-crdt/y-crdt) to provide Swift language bindings that
seamlessly interoperate with other Yjs implementations.

**This repository is WIP (Work In Progress)**
Not all features and capabilities from Yrs or Yjs are exposed at this time.
We plan to add them as the library evolves.

The repository includes two swift packages:

`yniffiFFI` a static binary packaged as an XCFramework in the `lib` directory, built with the Rust compiler and overlaid using [UniFFI](https://github.com/mozilla/uniffi-rs/).
`YSwift` which is an overlay to provide more idiomatic Swift language operations.

## This fork: build the XCFramework first

This fork (branch `yrs-0.28`: yrs 0.28.2, uniffi 0.32.2) publishes no release
asset. `Package.swift` always uses the local, git-ignored
`lib/yniffiFFI.xcframework`, so build it before you build the package or
any app that depends on it:

```sh
scripts/build-xcframework.sh               # build on fw, assemble on the Mac
BUILD_HOST=other scripts/build-xcframework.sh
SKIP_REMOTE=1 scripts/build-xcframework.sh # reassemble from lib/target-remote
```

The script rsyncs the source to a Linux build host over ssh (default `fw`,
which needs rustup in `~/.cargo`), runs the Rust tests, generates the Swift
bindings into `lib/swift/scaffold`, and cross-compiles the static libs for
`aarch64-apple-ios`, `aarch64-apple-ios-sim` and `x86_64-apple-ios`. Back on the
Mac it assembles `lib/yniffiFFI.xcframework` with `xcodebuild
-create-xcframework`. No Rust is needed on the Mac. The package is iOS only:
the XCFramework has no macOS slice.

Consumers depend on a local checkout, for example in XcodeGen:

```yaml
packages:
  YSwift:
    path: /Users/macruss/Projects/yswift
```

Without the XCFramework, package resolution fails on the `yniffiFFI` binary
target. Rebuild it after any change under `lib/`.

Every fallible Yniffi call throws `YrsError`, including Rust panics, which are
caught at the FFI boundary. The `YSwift` layer keeps its non-throwing API and
traps on errors.

## Decision log

This project maintains a [decision log](./devnotes/DevLog.md).
Please consult it in case there is some ambiguity in terms of why certain implementation details look as they are.

## License

This project is available as open source under the terms of the [MIT License](https://opensource.org/licenses/MIT).

## Thanks to

Amazing people at Mozilla for their outsanding work on [UniFFI](https://github.com/mozilla/uniffi-rs/) and all of the supporting work they've done on using, packaging and distributing Rust code for Swift and Kotlin codebases.
