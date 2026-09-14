#!/usr/bin/env bash
# Construit le client PWA du compagnon mobile dans pwa/dist.
#
# Les apps de bureau embarquent ce dossier : le pont local sert alors *sa* PWA, et le QR
# d'appairage pointe vers le pont, plus vers la PWA publique déployée sur le Worker.
set -euo pipefail
source "$(dirname "$0")/env.sh"

export PATH="$HOME/.bun/bin:$PATH"
if ! command -v bun > /dev/null; then
    echo "!! bun est requis (https://bun.sh) pour construire la PWA." >&2
    exit 1
fi

cd "$DMX_ROOT/pwa"
bun install --frozen-lockfile
bun run build
echo "==> $DMX_ROOT/pwa/dist"
