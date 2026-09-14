#!/usr/bin/env bash
# Installe les icônes de l'app Linux :
#   - les icônes Lucide converties en icônes symboliques GTK (traits transformés en surfaces,
#     pour que GTK les recolore correctement : thème clair/sombre et couleurs de comptes) ;
#   - l'icône d'application aux tailles attendues par les thèmes hicolor.
set -euo pipefail
source "$(dirname "$0")/env.sh"

THEME="$DMX_ROOT/linux/dmx-money-gtk/data/icons/hicolor"
ACTIONS="$THEME/scalable/actions"
BRAND="$DMX_ROOT/shared/brand/icons/icon.png"

rm -rf "$ACTIONS"
mkdir -p "$ACTIONS"
cargo run --quiet -p gen-symbolic-icons -- "$DMX_ROOT/shared/icons/lucide" "$ACTIONS"

cat > "$THEME/index.theme" <<'THEME_EOF'
[Icon Theme]
Name=Hicolor
Comment=Icônes de DmxMoney
Directories=scalable/actions,scalable/apps,512x512/apps,256x256/apps,128x128/apps,64x64/apps

[scalable/actions]
Size=16
MinSize=16
MaxSize=512
Context=Actions
Type=Scalable

[scalable/apps]
Size=512
MinSize=16
MaxSize=512
Context=Applications
Type=Scalable

[512x512/apps]
Size=512
Context=Applications
Type=Threshold

[256x256/apps]
Size=256
Context=Applications
Type=Threshold

[128x128/apps]
Size=128
Context=Applications
Type=Threshold

[64x64/apps]
Size=64
Context=Applications
Type=Threshold
THEME_EOF

for size in 64 128 256 512; do
    directory="$THEME/${size}x${size}/apps"
    mkdir -p "$directory"
    if command -v sips > /dev/null; then
        sips -z "$size" "$size" "$BRAND" --out "$directory/com.dmxmoney.app.png" > /dev/null
    elif command -v convert > /dev/null; then
        convert "$BRAND" -resize "${size}x${size}" "$directory/com.dmxmoney.app.png"
    else
        echo "!! ni sips ni convert : copie de l'icône sans redimensionnement ($size)"
        cp "$BRAND" "$directory/com.dmxmoney.app.png"
    fi
done

echo "==> icônes installées dans $THEME"
