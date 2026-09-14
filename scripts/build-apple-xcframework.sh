#!/usr/bin/env bash
# Construit DmxCoreFFI.xcframework (macOS universel 10.15+, iOS, simulateur iOS) et génère
# les bindings Swift dans apple/Packages/DmxKit.
#
#   PROFILE=release|debug   (défaut : release)
#   IOS_SIM_X86=1           ajoute la tranche simulateur Intel
#   SKIP_IOS=1              macOS seulement (itérations rapides)
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$DMX_ROOT"

PROFILE="${PROFILE:-release}"
PROFILE_DIR="$PROFILE"
[[ "$PROFILE" == "dev" ]] && PROFILE_DIR="debug"
[[ "$PROFILE" == "debug" ]] && PROFILE="dev" && PROFILE_DIR="debug"

KIT="$DMX_ROOT/apple/Packages/DmxKit"
WORK="$DMX_ROOT/target/apple-xcframework"
GENERATED="$KIT/Sources/DmxCore/Generated"
FRAMEWORK="$KIT/Frameworks/DmxCoreFFI.xcframework"

rm -rf "$WORK" && mkdir -p "$WORK/headers" "$WORK/macos" "$WORK/ios" "$WORK/ios-sim" "$GENERATED"

build() {
    local target="$1"; shift
    echo "==> dmx-ffi ($target)"
    cargo build -p dmx-ffi --profile "$PROFILE" --target "$target" "$@"
}

export MACOSX_DEPLOYMENT_TARGET=10.15
build aarch64-apple-darwin
build x86_64-apple-darwin

if [[ "${SKIP_IOS:-0}" != "1" ]]; then
    export IPHONEOS_DEPLOYMENT_TARGET=17.0
    build aarch64-apple-ios --no-default-features
    build aarch64-apple-ios-sim --no-default-features
    if [[ "${IOS_SIM_X86:-0}" == "1" ]]; then
        build x86_64-apple-ios --no-default-features
    fi
fi

echo "==> Bindings Swift"
cargo run -q -p uniffi-bindgen -- generate \
    --library "target/aarch64-apple-darwin/$PROFILE_DIR/libdmx_ffi.dylib" \
    --language swift \
    --config core/crates/dmx-ffi/uniffi.toml \
    --out-dir "$WORK/bindings"

cp "$WORK/bindings/DmxCore.swift" "$GENERATED/DmxCore.swift"
cp "$WORK/bindings/DmxCoreFFI.h" "$WORK/headers/DmxCoreFFI.h"
cp "$WORK/bindings/DmxCoreFFI.modulemap" "$WORK/headers/module.modulemap"

echo "==> Bibliothèques statiques"
lipo -create \
    "target/aarch64-apple-darwin/$PROFILE_DIR/libdmx_ffi.a" \
    "target/x86_64-apple-darwin/$PROFILE_DIR/libdmx_ffi.a" \
    -output "$WORK/macos/libdmx_ffi.a"

ARGS=(-library "$WORK/macos/libdmx_ffi.a" -headers "$WORK/headers")

if [[ "${SKIP_IOS:-0}" != "1" ]]; then
    cp "target/aarch64-apple-ios/$PROFILE_DIR/libdmx_ffi.a" "$WORK/ios/libdmx_ffi.a"
    SIM_LIBS=("target/aarch64-apple-ios-sim/$PROFILE_DIR/libdmx_ffi.a")
    if [[ "${IOS_SIM_X86:-0}" == "1" ]]; then
        SIM_LIBS+=("target/x86_64-apple-ios/$PROFILE_DIR/libdmx_ffi.a")
    fi
    lipo -create "${SIM_LIBS[@]}" -output "$WORK/ios-sim/libdmx_ffi.a"
    ARGS+=(-library "$WORK/ios/libdmx_ffi.a" -headers "$WORK/headers")
    ARGS+=(-library "$WORK/ios-sim/libdmx_ffi.a" -headers "$WORK/headers")
fi

rm -rf "$FRAMEWORK"
mkdir -p "$(dirname "$FRAMEWORK")"
xcodebuild -create-xcframework "${ARGS[@]}" -output "$FRAMEWORK"

echo "==> $FRAMEWORK"
