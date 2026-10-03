#!/usr/bin/env bash
# Environnement de build commun. À sourcer : `source scripts/env.sh`.
#
# Sur macOS, on force le SDK de Xcode.app : les Command Line Tools d'une version bêta de macOS
# peuvent fournir un SDK que l'éditeur de liens de rustc ne sait pas lire.

DMX_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
export DMX_ROOT

if [[ "$(uname -s)" == "Darwin" && -d /Applications/Xcode.app ]]; then
    export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
    SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
    export SDKROOT
fi

# Rust's Mach-O strip path can misalign LINKEDIT for dyld on Xcode/macOS27.
# Keep the native library loadable; Xcode still controls stripping of the final application.
# https://github.com/rust-lang/rust/issues/157750
if [[ "$(uname -s)" == "Darwin" ]]; then
    export CARGO_PROFILE_DEV_STRIP=none
    export CARGO_PROFILE_TEST_STRIP=none
    export CARGO_PROFILE_RELEASE_STRIP=none
fi

export DMX_VERSION
DMX_VERSION="$(tr -d '[:space:]' < "$DMX_ROOT/VERSION")"
