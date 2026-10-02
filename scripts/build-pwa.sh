#!/usr/bin/env bash
# Construit le client PWA du compagnon mobile dans pwa/dist.
#
# Le déploiement Pages prépare ce dossier sous /mobile/ ; les QR d'appairage utilisent
# l'adresse commune https://dmxmoney-companion.pages.dev/mobile/.
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
