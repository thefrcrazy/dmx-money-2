#!/usr/bin/env bash
# Construit et publie le compagnon mobile sur le projet Cloudflare Pages commun.
set -euo pipefail
source "$(dirname "$0")/env.sh"
export PATH="$HOME/.bun/bin:$PATH"

"$DMX_ROOT/scripts/build-pwa.sh"

cd "$DMX_ROOT/cloudflare/remote-relay"
bun install --frozen-lockfile
bun run pages:assets
bun run pages:deploy
echo "==> https://dmxmoney-companion.pages.dev/mobile/"
