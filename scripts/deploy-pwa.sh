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

python3 - <<PY
import os
import subprocess

dist_dir = "$DMX_ROOT/pwa/dist"
config = "$CONFIG"

for root, dirs, files in os.walk(dist_dir):
    for f in files:
        if f.startswith('.'):
            continue
        full_path = os.path.join(root, f)
        key = os.path.relpath(full_path, dist_dir)
        cmd = [
            'npx', 'wrangler', 'kv', 'key', 'put',
            '--namespace-id=b3d0c645407c48669e7232fa7e907c6e',
            '--remote',
            key,
            f'--path={full_path}'
        ]
        res = subprocess.run(cmd, capture_output=True, text=True)
        if res.returncode != 0:
            print(f"Échec sur {key}: {res.stderr}")
            exit(1)
        print(f"✓ {key}")
print("Déploiement PWA terminé avec succès.")
PY
