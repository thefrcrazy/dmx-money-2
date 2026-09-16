#!/usr/bin/env bash
# Déploie le client PWA du compagnon mobile sur Cloudflare Workers KV.
set -euo pipefail
source "$(dirname "$0")/env.sh"

CONFIG="$DMX_ROOT/cloudflare/managed-bridge/wrangler.local.toml"
if [ ! -f "$CONFIG" ]; then
    echo "!! Fichier $CONFIG manquant." >&2
    exit 1
fi

"$DMX_ROOT/scripts/build-pwa.sh"

python3 - <<'PYTHON'
import os
import subprocess
from pathlib import Path

root = Path(os.environ['DMX_ROOT'])
dist = root / 'pwa/dist'
config = root / 'cloudflare/managed-bridge/wrangler.local.toml'
# Publish immutable assets first; entry points must never reference missing bundles.
files = [path for path in dist.rglob('*') if path.is_file() and not path.name.startswith('.')]
priority = lambda path: 2 if path.name == 'sw.js' else 1 if path.name == 'index.html' else 0
for path in sorted(files, key=lambda path: (priority(path), str(path))):
    key = path.relative_to(dist).as_posix()
    subprocess.run([
        'npx', 'wrangler', 'kv', 'key', 'put', key,
        '--config', str(config), '--binding', 'ASSETS', '--remote', '--path', str(path),
    ], check=True)
    print(f'✓ {key}', flush=True)
print('Déploiement PWA terminé avec succès.')
PYTHON
