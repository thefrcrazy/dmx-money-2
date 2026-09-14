#!/usr/bin/env bash
# Génère les catalogues AppIcon macOS et iOS à partir de l'icône 1024 px de la marque.
set -euo pipefail
source "$(dirname "$0")/env.sh"

SOURCE="$DMX_ROOT/shared/brand/icons/ios/AppIcon-512@2x.png"

mac_set="$DMX_ROOT/apple/DmxMoney-macOS/Assets.xcassets/AppIcon.appiconset"
ios_set="$DMX_ROOT/apple/DmxMoney-iOS/Assets.xcassets/AppIcon.appiconset"
rm -rf "$mac_set" "$ios_set"
mkdir -p "$mac_set" "$ios_set"

for catalog in "$DMX_ROOT/apple/DmxMoney-macOS/Assets.xcassets" "$DMX_ROOT/apple/DmxMoney-iOS/Assets.xcassets"; do
    printf '{\n  "info" : { "author" : "xcode", "version" : 1 }\n}\n' > "$catalog/Contents.json"
done

entries=""
for size in 16 32 128 256 512; do
    for scale in 1 2; do
        pixels=$((size * scale))
        file="icon_${size}x${size}@${scale}x.png"
        sips -z "$pixels" "$pixels" "$SOURCE" --out "$mac_set/$file" > /dev/null
        entries+="    { \"filename\" : \"$file\", \"idiom\" : \"mac\", \"scale\" : \"${scale}x\", \"size\" : \"${size}x${size}\" },"$'\n'
    done
done
cat > "$mac_set/Contents.json" <<JSON
{
  "images" : [
${entries%,$'\n'}
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

cp "$SOURCE" "$ios_set/AppIcon-1024.png"
cat > "$ios_set/Contents.json" <<'JSON'
{
  "images" : [
    { "filename" : "AppIcon-1024.png", "idiom" : "universal", "platform" : "ios", "size" : "1024x1024" }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

echo "==> Icônes d'application générées"
