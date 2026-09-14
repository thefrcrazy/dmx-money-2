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

export DMX_VERSION
DMX_VERSION="$(tr -d '[:space:]' < "$DMX_ROOT/VERSION")"
