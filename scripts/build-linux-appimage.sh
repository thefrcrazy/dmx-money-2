#!/usr/bin/env bash
# Construit une AppImage x86_64 (à lancer sous Linux).
#
# Prérequis : GTK4 et libadwaita de développement, `appimagetool` dans le PATH
# (https://github.com/AppImage/AppImageKit/releases). linuxdeploy est utilisé s'il est
# disponible, pour embarquer les bibliothèques GTK.
set -euo pipefail
source "$(dirname "$0")/env.sh"

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "!! AppImage : à construire sous Linux." >&2
    exit 1
fi

OUT="$DMX_ROOT/target/appimage"
APPDIR="$OUT/DmxMoney.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR"

bash "$DMX_ROOT/scripts/build-linux.sh"
bash "$DMX_ROOT/scripts/install-linux.sh" "$APPDIR/usr" "$DMX_ROOT/target/release/dmx-money-gtk"

# Fichiers attendus à la racine de l'AppDir.
install -Dm644 "$APPDIR/usr/share/applications/com.dmxmoney.app.desktop" "$APPDIR/com.dmxmoney.app.desktop"
install -Dm644 "$APPDIR/usr/share/icons/hicolor/256x256/apps/com.dmxmoney.app.png" "$APPDIR/com.dmxmoney.app.png"
cat > "$APPDIR/AppRun" <<'RUN_EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "$0")")"
export PATH="$HERE/usr/bin:$PATH"
export XDG_DATA_DIRS="$HERE/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
export GDK_PIXBUF_MODULEDIR="${GDK_PIXBUF_MODULEDIR:-}"
exec "$HERE/usr/bin/dmxmoney" "$@"
RUN_EOF
chmod +x "$APPDIR/AppRun"

VERSION="$(cat "$DMX_ROOT/VERSION")"
if command -v linuxdeploy > /dev/null; then
    linuxdeploy --appdir "$APPDIR" --plugin gtk --output appimage
    mv DmxMoney*.AppImage "$OUT/DmxMoney-$VERSION-x86_64.AppImage"
else
    ARCH=x86_64 appimagetool "$APPDIR" "$OUT/DmxMoney-$VERSION-x86_64.AppImage"
fi

echo "==> $OUT/DmxMoney-$VERSION-x86_64.AppImage"
