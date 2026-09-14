#!/usr/bin/env bash
# Construit l'app macOS dans dist/<variante>/DmxMoney.app.
#
# Deux variantes, comme en DmxMoney 1.x :
#   modern  (défaut) : interface SwiftUI native, Apple Silicon, macOS 26 Tahoe
#   legacy           : interface AppKit compatible, Intel, macOS 10.15 Catalina
#   universal        : interface legacy, les deux architectures (essai local)
#
#   ./scripts/build-macos.sh                  # modern (arm64)
#   ./scripts/build-macos.sh legacy           # Intel / Catalina
#   ./scripts/build-macos.sh universal
#   CONFIG=Debug ./scripts/build-macos.sh     # itérations rapides
#   SKIP_CORE=1 ./scripts/build-macos.sh      # garde l'XCFramework déjà construit
set -euo pipefail
source "$(dirname "$0")/env.sh"

VARIANT="${1:-modern}"
CONFIG="${CONFIG:-Release}"

case "$VARIANT" in
    modern) SCHEME="DmxMoney-macOS-Modern"; ARCHS="arm64"; DEPLOYMENT="26.0" ;;
    legacy) SCHEME="DmxMoney-macOS"; ARCHS="x86_64"; DEPLOYMENT="10.15" ;;
    universal) SCHEME="DmxMoney-macOS"; ARCHS="arm64 x86_64"; DEPLOYMENT="10.15" ;;
    *) echo "!! variante inconnue : $VARIANT (modern | legacy | universal)" >&2; exit 2 ;;
esac

BUILD="$DMX_ROOT/target/apple-build/$VARIANT"
DIST="$DMX_ROOT/dist/$VARIANT"

if [[ "${SKIP_CORE:-0}" != "1" || ! -d "$DMX_ROOT/apple/Packages/DmxKit/Frameworks/DmxCoreFFI.xcframework" ]]; then
    SKIP_IOS=1 bash "$DMX_ROOT/scripts/build-apple-xcframework.sh"
fi
bash "$DMX_ROOT/scripts/sync-apple-icons.sh" > /dev/null
bash "$DMX_ROOT/scripts/sync-apple-app-icons.sh" > /dev/null

(cd "$DMX_ROOT/apple" && xcodegen generate > /dev/null)

echo "==> DmxMoney $VARIANT ($SCHEME, $ARCHS, macOS $DEPLOYMENT+, $CONFIG)"
xcodebuild -project "$DMX_ROOT/apple/DmxMoney.xcodeproj" -scheme "$SCHEME" \
    -configuration "$CONFIG" -destination 'generic/platform=macOS' \
    -derivedDataPath "$BUILD" \
    ARCHS="$ARCHS" ONLY_ACTIVE_ARCH=NO MACOSX_DEPLOYMENT_TARGET="$DEPLOYMENT" \
    CODE_SIGNING_ALLOWED=NO build | tail -1

mkdir -p "$DIST"
rm -rf "$DIST/DmxMoney.app"
cp -R "$BUILD/Build/Products/$CONFIG/DmxMoney.app" "$DIST/DmxMoney.app"

# Signature scellée, avec un Team ID dès que possible.
#
# Sans équipe, l'éditeur de liens ne signe que l'exécutable : les ressources (dont
# Metadata.appintents) ne sont pas scellées et `codesign --verify` échoue. Même scellée, une
# signature ad hoc n'a pas de Team ID, et l'index des intentions (linkd) ignore l'app : Raccourcis
# répond « couldn't communicate with the app » et Siri ne trouve pas ses phrases. On prend donc la
# première identité « Apple Development » du trousseau (DMX_SIGN_IDENTITY pour en forcer une),
# l'ad hoc ne restant qu'un repli.
SIGN_IDENTITY="${DMX_SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null | awk -F'"' '/Apple Development/ {print $2; exit}')}"
if [[ -z "$SIGN_IDENTITY" ]]; then
    echo "    aucune identité Apple Development : signature ad hoc (Siri et Raccourcis ne verront pas l'app)"
    SIGN_IDENTITY="-"
fi
codesign --force --sign "$SIGN_IDENTITY" --options runtime --timestamp=none \
    --entitlements "$DMX_ROOT/apple/DmxMoney-macOS/DmxMoney.entitlements" \
    "$DIST/DmxMoney.app"
codesign --verify --strict "$DIST/DmxMoney.app"
echo "    signée : $(codesign -dv "$DIST/DmxMoney.app" 2>&1 | awk -F= '/^TeamIdentifier/ {print "équipe " $2}')"

# Une seule copie connue de LaunchServices pour com.dmxmoney.app : le produit intermédiaire de
# Xcode porte le même identifiant, et Raccourcis pourrait lancer celui-là (ou une ancienne build
# sans intentions) au lieu de l'app de dist/.
LSREGISTER=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
"$LSREGISTER" -u "$BUILD/Build/Products/$CONFIG/DmxMoney.app" >/dev/null 2>&1 || true
# N'enregistrer que la variante qui tourne nativement ici : sur un Mac Apple Silicon, la build
# legacy (x86_64) porte le même identifiant et ne connaît pas les intentions Siri.
if [[ "$VARIANT" == "modern" && "$(uname -m)" == "arm64" ]] || [[ "$VARIANT" != "modern" && "$(uname -m)" == "x86_64" ]]; then
    "$LSREGISTER" -f "$DIST/DmxMoney.app" >/dev/null 2>&1 || true
else
    "$LSREGISTER" -u "$DIST/DmxMoney.app" >/dev/null 2>&1 || true
fi

echo "==> $DIST/DmxMoney.app"
lipo -info "$DIST/DmxMoney.app/Contents/MacOS/DmxMoney" | sed 's/^/    /'
echo "    ouvrir : open \"$DIST/DmxMoney.app\""
