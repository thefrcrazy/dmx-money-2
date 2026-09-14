#!/usr/bin/env bash
# Compile l'app GTK4 en release. À lancer sous Linux (ou sur macOS avec GTK via Homebrew,
# pour un simple contrôle de compilation).
set -euo pipefail
source "$(dirname "$0")/env.sh"

if [[ "$(uname -s)" == "Darwin" ]]; then
    export PKG_CONFIG_PATH="$(brew --prefix)/lib/pkgconfig:$(brew --prefix)/share/pkgconfig:${PKG_CONFIG_PATH:-}"
fi

bash "$DMX_ROOT/scripts/gen-linux-icons.sh"
cargo build --release -p dmx-money-gtk
echo "==> $DMX_ROOT/target/release/dmx-money-gtk"
