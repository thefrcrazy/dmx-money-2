#!/usr/bin/env python3
"""Extrait les notes d'une version du CHANGELOG (utilisé par la publication GitHub)."""

import sys
from pathlib import Path


def notes(changelog: Path, version: str) -> str:
    lines: list[str] = []
    capture = False
    for line in changelog.read_text(encoding="utf-8").splitlines():
        if line.startswith("## "):
            if capture:
                break
            capture = line[3:].lstrip("[").startswith(version)
            continue
        if capture:
            lines.append(line)
    return "\n".join(lines).strip()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: release-notes.py <version> [chemin du CHANGELOG]")
    path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).resolve().parent.parent / "CHANGELOG.md"
    text = notes(path, sys.argv[1])
    print(text if text else f"DmxMoney {sys.argv[1]}")
