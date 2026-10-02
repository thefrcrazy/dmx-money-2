#!/usr/bin/env bash
# Preview selected compilation caches; deletion requires an explicit --apply.
set -euo pipefail
DMX_CACHE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
exec python3 - "$DMX_CACHE_ROOT" "$@" <<'PY'
import argparse
import collections
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

parser = argparse.ArgumentParser(description="Caches DmxMoney seulement : aucune suppression par défaut.")
parser.add_argument("--scope", choices=("incremental", "audits"), default="incremental")
parser.add_argument("--apply", action="store_true", help="supprimer les seuls dossiers affichés")
args = parser.parse_args(sys.argv[2:])
root = Path(sys.argv[1])
if not (root / "Cargo.toml").is_file() or not (root / "VERSION").is_file():
    parser.error("racine du dépôt non reconnue")

if args.scope == "incremental":
    targets = ("", "aarch64-apple-darwin", "x86_64-apple-darwin",
               "aarch64-apple-ios", "aarch64-apple-ios-sim", "x86_64-apple-ios")
    allowed = [Path("target") / target / profile / "incremental"
               for target in targets for profile in ("debug", "release")]
else:
    # Historical Xcode/Swift audit outputs. Keep UI demo databases and screenshots.
    allowed = [Path(path) for path in (
        "target/apple-build", "target/apple-audit-build", "target/apple-audit-modern",
        "target/apple-review-final", "target/xcode-dmxkit", "target/xcode-dmxkit-ios",
        "target/journal-modern-build", "target/journal-legacy-build",
        "target/journal-swift-tests", "target/journal-module-cache",
        "target/journal-swift-cache", "target/journal-modern-packages",
        "target/journal-legacy-packages", "target/apple-ui-verification/build",
    )]

tracked = subprocess.run(["git", "-C", str(root), "ls-files", "-z"],
                         check=True, capture_output=True).stdout.decode().split("\0")
selected = []
inodes = collections.defaultdict(lambda: [0, 0, 0])
for relative in allowed:
    path = root / relative
    if not path.exists() and not path.is_symlink():
        continue
    if any((root / parent).is_symlink() for parent in (relative, *relative.parents)):
        parser.error(f"lien symbolique refusé : {relative}")
    if not path.is_dir():
        parser.error(f"dossier attendu : {relative}")
    if any(name == str(relative) or name.startswith(str(relative) + "/") for name in tracked if name):
        parser.error(f"fichier versionné dans {relative}")
    size = 0
    for current, dirs, files in os.walk(path, followlinks=False):
        for name in files:
            item = Path(current) / name
            suffix = item.suffix.lower()
            build_db = (name == "build.db" and "XCBuildData" in item.parts
                        or name == "metadata.db" and "TestResults" in item.parts
                        or name.startswith("manifest.db") and "manifests" in item.parts)
            if name == ".env" or name.startswith(".env.") or name.endswith((".pfx.b64", ".p12.b64")) or suffix in (
                ".dmx", ".pfx", ".p12", ".pem", ".key", ".mobileprovision",
                ".provisionprofile", ".sqlite", ".sqlite3"
            ) or (
                (suffix == ".db" or name.endswith((".db-wal", ".db-shm", ".sqlite-wal",
                    ".sqlite-shm", ".sqlite3-wal", ".sqlite3-shm"))) and not build_db
            ):
                parser.error(f"donnée ou secret à préserver : {item.relative_to(root)}")
            info = item.lstat()
            if stat.S_ISREG(info.st_mode):
                size += info.st_blocks * 512
                record = inodes[(info.st_dev, info.st_ino)]
                record[0] += 1
                record[1] = info.st_nlink
                record[2] = info.st_blocks * 512
    selected.append(path)
    print(f"{size / 1_000_000_000:8.3f} Go par chemins  {relative}")

# Shared hardlinks outside the selection remain allocated after deletion.
reclaim = sum(size for links, total_links, size in inodes.values() if links == total_links)
print(f"Gain estimé hors liens conservés : {reclaim / 1_000_000_000:.3f} Go.")
print("Données de l'app, sauvegardes, secrets, captures UI, dist et dépendances conservés.")
if not args.apply:
    print("Simulation seulement. Ajouter --apply pour supprimer ces caches, après arrêt des builds.")
    raise SystemExit(0)

processes = subprocess.run(["ps", "-axo", "pid=,comm="], capture_output=True, text=True)
if processes.returncode:
    parser.error("impossible de vérifier les builds actifs ; suppression refusée")
builders = {"cargo", "rustc", "rust-analyzer", "xcodebuild", "swift", "swift-frontend", "dotnet", "MSBuild"}
active = [line.strip() for line in processes.stdout.splitlines()
          if len(line.split(None, 1)) == 2
          and Path(line.split(None, 1)[1]).name in builders]
if active:
    parser.error("compilation active, suppression refusée : " + "; ".join(active))
for path in selected:
    # Recheck the allowlisted directory before deletion; rmtree does not follow child symlinks.
    if path.is_symlink() or path.resolve() != path:
        parser.error(f"dossier déplacé ou lien symbolique : {path.relative_to(root)}")
    shutil.rmtree(path)
print(f"{len(selected)} dossiers de cache supprimés. Ils seront reconstruits au prochain build.")
PY
