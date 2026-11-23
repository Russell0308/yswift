#!/usr/bin/env bash
# Builds lib/yniffiFFI.xcframework (iOS device + simulator) and the uniffi
# Swift bindings (lib/swift/scaffold).
#
# Rust is not installed on the Mac: the static libs are cross-compiled on a
# Linux build host over ssh (default `fw`), then assembled here with
# xcodebuild. Rust staticlibs need no Apple linker or SDK; the app links them.
#
#   scripts/build-xcframework.sh              build on fw, assemble, zip
#   BUILD_HOST=other scripts/build-xcframework.sh
#   SKIP_REMOTE=1 scripts/build-xcframework.sh   reassemble from lib/target-remote
#
# Output:
#   lib/yniffiFFI.xcframework        (Package.swift uses it whenever it exists)
#   lib/yniffiFFI.xcframework.zip    plus its SwiftPM checksum on the last line
#
# No macOS slice: the macOS targets need the Apple SDK to build and aren't used yet.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd -P)"
HOST="${BUILD_HOST:-fw}"
REMOTE_DIR="${REMOTE_DIR:-build/yswift}" # relative to the remote home
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios)
LIB_NAME="libuniffi_yniffi.a"
FRAMEWORK="yniffiFFI"
LIB="$ROOT/lib"
OUT="$LIB/target-remote"

for tool in rsync ssh xcodebuild lipo ditto swift; do
  command -v "$tool" >/dev/null || { echo "FAIL: $tool not found"; exit 1; }
done

if [[ -z "${SKIP_REMOTE:-}" ]]; then
  echo "▸ Sync source to $HOST:$REMOTE_DIR"
  rsync -a --delete --exclude .git --exclude target --exclude target-remote \
    --exclude '*.xcframework' --exclude '*.xcframework.zip' --exclude .build \
    "$ROOT/" "$HOST:$REMOTE_DIR/"

  echo "▸ Test, generate bindings, cross-compile on $HOST"
  # shellcheck disable=SC2087 # expand TARGETS locally on purpose
  ssh "$HOST" bash -s <<EOF
set -euo pipefail
source "\$HOME/.cargo/env"
cd "$REMOTE_DIR/lib"
rustup target add ${TARGETS[*]} >/dev/null
rustc --version
cargo test --locked --quiet
rm -rf out && mkdir -p out/scaffold
cargo run --locked --quiet --features=uniffi/cli --bin uniffi-bindgen -- \
  generate src/yniffi.udl --language swift --out-dir out/scaffold
for target in ${TARGETS[*]}; do
  cargo build --locked --release --lib --target "\$target"
  mkdir -p "out/\$target"
  cp "target/\$target/release/$LIB_NAME" "out/\$target/"
done
EOF

  echo "▸ Fetch artifacts"
  rm -rf "$OUT"
  rsync -a "$HOST:$REMOTE_DIR/lib/out/" "$OUT/"
fi

for target in "${TARGETS[@]}"; do
  [[ -f "$OUT/$target/$LIB_NAME" ]] || { echo "FAIL: missing $OUT/$target/$LIB_NAME"; exit 1; }
done

echo "▸ Swift bindings → lib/swift/scaffold"
mkdir -p "$LIB/swift/scaffold"
cp "$OUT/scaffold/yniffi.swift" "$OUT/scaffold/${FRAMEWORK}.h" "$OUT/scaffold/${FRAMEWORK}.modulemap" "$LIB/swift/scaffold/"

echo "▸ Assemble $FRAMEWORK.xcframework"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/headers" "$WORK/ios-simulator"
cp "$OUT/scaffold/${FRAMEWORK}.h" "$WORK/headers/"
cp "$OUT/scaffold/${FRAMEWORK}.modulemap" "$WORK/headers/module.modulemap"
lipo -create "$OUT/aarch64-apple-ios-sim/$LIB_NAME" "$OUT/x86_64-apple-ios/$LIB_NAME" \
  -output "$WORK/ios-simulator/$LIB_NAME"

rm -rf "$LIB/$FRAMEWORK.xcframework" "$LIB/$FRAMEWORK.xcframework.zip"
xcodebuild -create-xcframework \
  -library "$OUT/aarch64-apple-ios/$LIB_NAME" -headers "$WORK/headers" \
  -library "$WORK/ios-simulator/$LIB_NAME" -headers "$WORK/headers" \
  -output "$LIB/$FRAMEWORK.xcframework"

echo "▸ Zip and checksum"
(cd "$LIB" && ditto -c -k --sequesterRsrc --keepParent "$FRAMEWORK.xcframework" "$FRAMEWORK.xcframework.zip")
du -sh "$LIB/$FRAMEWORK.xcframework"/*/ "$LIB/$FRAMEWORK.xcframework.zip"
echo "checksum $(swift package compute-checksum "$LIB/$FRAMEWORK.xcframework.zip")"
echo "PASS: $LIB/$FRAMEWORK.xcframework"
