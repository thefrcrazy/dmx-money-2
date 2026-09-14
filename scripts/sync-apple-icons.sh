#!/usr/bin/env bash
# Génère le catalogue d'icônes Lucide de DmxKit (images vectorielles en mode modèle) à partir de
# shared/icons/lucide. Les noms d'images sont les noms Lucide stockés en base.
set -euo pipefail
source "$(dirname "$0")/env.sh"

SOURCE="$DMX_ROOT/shared/icons/lucide"
CATALOG="$DMX_ROOT/apple/Packages/DmxKit/Sources/DmxKit/Resources/Icons.xcassets"

rm -rf "$CATALOG"
mkdir -p "$CATALOG"
cat > "$CATALOG/Contents.json" <<'JSON'
{
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON

count=0
for svg in "$SOURCE"/*.svg; do
    name="$(basename "$svg" .svg)"
    set="$CATALOG/$name.imageset"
    mkdir -p "$set"
    # CoreSVG ne résout pas `currentColor` : un trait noir suffit, l'image est teintée en mode modèle.
    sed -e 's/currentColor/#000000/g' -e 's/ class="[^"]*"//' -e '/<!--.*-->/d' "$svg" > "$set/$name.svg"
    cat > "$set/Contents.json" <<JSON
{
  "images" : [
    { "filename" : "$name.svg", "idiom" : "universal" }
  ],
  "info" : { "author" : "xcode", "version" : 1 },
  "properties" : {
    "preserves-vector-representation" : true,
    "template-rendering-intent" : "template"
  }
}
JSON
    count=$((count + 1))
done

echo "==> $count icônes dans $CATALOG"
