#!/usr/bin/env python3
"""Écrit le flux de mise à jour macOS (JSON lu par UpdateChecker) sur la sortie standard.

Une entrée par architecture, comme le `latest.json` de DmxMoney 1.x :
Apple Silicon (macOS 11+) et Intel/Catalina (macOS 10.15+).

    update-feed.py <version> <owner/repo>
"""

import json
import sys
from pathlib import Path

# (clé de plateforme, suffixe du DMG, macOS minimum)
BUILDS = [
    ("darwin-arm64", "apple-silicon", "11.0"),
    ("darwin-x86_64", "intel-catalina", "10.15"),
]


def release_notes(version: str) -> str:
    """Même extraction que `release-notes.py` (dont le nom interdit un import direct)."""
    changelog = Path(__file__).resolve().parent.parent / "CHANGELOG.md"
    lines: list[str] = []
    capture = False
    for line in changelog.read_text(encoding="utf-8").splitlines():
        if line.startswith("## "):
            if capture:
                break
            capture = line[3:].lstrip("[").startswith(version)
            continue
        if capture and line.strip():
            lines.append(line.strip())
    # Le flux alimente une alerte : on garde les premières puces, pas la section entière.
    summary = [line for line in lines if line.startswith("- ")][:4]
    return "\n".join(summary) if summary else "\n".join(lines[:4])


def dmg_name(version: str, suffix: str) -> str:
    return f"DmxMoney-{version}-{suffix}.dmg"


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit("usage: update-feed.py <version> <owner/repo>")
    version, repository = sys.argv[1], sys.argv[2]
    base = f"https://github.com/{repository}/releases/download/v{version}"
    feed = {
        "version": version,
        "notes": release_notes(version),
        "platforms": {
            platform: {
                "url": f"{base}/{dmg_name(version, suffix)}",
                "minimumSystemVersion": minimum,
            }
            for platform, suffix, minimum in BUILDS
        },
    }
    print(json.dumps(feed, ensure_ascii=False, indent=2))
