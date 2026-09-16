#!/usr/bin/env bash
# Render the AppKit variant with Catalina artwork even on a modern CI host.
set -euo pipefail
source "$(dirname "$0")/env.sh"
cd "$DMX_ROOT"

WORK="$DMX_ROOT/target/apple-ui-verification"
mkdir -p "$WORK"
DATA="$(mktemp -d "$WORK/data.XXXXXX")"
SHOTS="$(mktemp -d "$WORK/screenshots.XXXXXX")"
cargo run -q -p seed-demo -- "$DATA"

xcodebuild -project apple/DmxMoney.xcodeproj -scheme DmxMoney-macOS \
    -configuration Debug -destination 'generic/platform=macOS' \
    -derivedDataPath "$WORK/build" ARCHS="$(uname -m)" ONLY_ACTIVE_ARCH=YES \
    MACOSX_DEPLOYMENT_TARGET="${DMXMONEY_UI_MIN_MACOS:-10.15}" \
    CODE_SIGNING_ALLOWED=NO build > "$WORK/build.log" 2>&1

export DMXMONEY_DATA_DIR="$DATA"
export DMXMONEY_SNAPSHOT_DIR="$SHOTS"
export DMXMONEY_BUNDLED_ICONS=1
export DMXMONEY_VERIFY_SETTINGS=1
python3 - "$WORK/build/Build/Products/Debug/DmxMoney.app/Contents/MacOS/DmxMoney" <<'PY'
import os
import subprocess
import sys
from pathlib import Path

subprocess.run([sys.argv[1]], check=True, timeout=180)
shots = Path(os.environ['DMXMONEY_SNAPSHOT_DIR'])
for name in ['dashboard', 'categories', 'settings-general', 'settings-companion',
             'settings-data', 'settings-about', 'dark-dashboard', 'light-after-dark-categories']:
    path = shots / f'{name}.png'
    if not path.is_file() or path.stat().st_size < 1024:
        raise RuntimeError(f'Missing UI capture: {path}')
print(f'AppKit UI and settings navigation verified: {shots}')
PY
