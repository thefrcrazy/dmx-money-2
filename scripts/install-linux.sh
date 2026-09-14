#!/usr/bin/env bash
# Installe l'app Linux dans un préfixe (/app pour Flatpak, AppDir/usr pour AppImage,
# /usr/local pour une installation manuelle). Le binaire doit déjà être compilé.
set -euo pipefail
source "$(dirname "$0")/env.sh"

PREFIX="${1:-/usr/local}"
BINARY="${2:-$DMX_ROOT/target/release/dmx-money-gtk}"
DATA="$DMX_ROOT/linux/dmx-money-gtk/data"

if [[ ! -x "$BINARY" ]]; then
    echo "!! binaire introuvable : $BINARY (lancez d'abord scripts/build-linux.sh)" >&2
    exit 1
fi

install -Dm755 "$BINARY" "$PREFIX/bin/dmxmoney"
install -Dm644 "$DATA/com.dmxmoney.app.desktop" "$PREFIX/share/applications/com.dmxmoney.app.desktop"
install -Dm644 "$DATA/com.dmxmoney.app.metainfo.xml" "$PREFIX/share/metainfo/com.dmxmoney.app.metainfo.xml"
install -Dm644 "$DATA/com.dmxmoney.app.mime.xml" "$PREFIX/share/mime/packages/com.dmxmoney.app.xml"

# Icônes : Lucide en symbolique (recolorées par le thème) et icône d'application.
find "$DATA/icons/hicolor" -type f | while read -r icon; do
    install -Dm644 "$icon" "$PREFIX/share/icons/hicolor/${icon#"$DATA/icons/hicolor/"}"
done

# Client PWA du compagnon mobile, servi par le pont local quand il est activé.
if [[ -d "$DMX_ROOT/pwa/dist" ]]; then
    mkdir -p "$PREFIX/share/dmx-money/pwa"
    cp -r "$DMX_ROOT/pwa/dist/." "$PREFIX/share/dmx-money/pwa/"
else
    echo "-- pwa/dist absent : le pont servira l'API sans la PWA locale"
fi

echo "==> DmxMoney installé dans $PREFIX"
