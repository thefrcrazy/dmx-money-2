#!/usr/bin/env python3
"""Convertit les SVG Lucide en géométries XAML pour l'app WinUI (icônes teintables).

Lucide dessine au trait : le C# généré expose les données de tracé, rendues par un
`Path` avec Stroke/StrokeThickness, ce qui suit la couleur du texte.
"""
from __future__ import annotations

import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "shared" / "icons" / "lucide"
TARGET = ROOT / "windows" / "src" / "DmxMoney.App" / "Icons" / "LucideIcons.g.cs"

NUMBER = r"-?\d*\.?\d+"


def attributes(element: str) -> dict[str, str]:
    return {key: value for key, value in re.findall(r'([a-zA-Z-]+)="([^"]*)"', element)}


def number(values: dict[str, str], key: str, default: float = 0.0) -> float:
    try:
        return float(values.get(key, default))
    except ValueError:
        return default


def points_path(raw: str, close: bool) -> str:
    values = [float(value) for value in re.findall(NUMBER, raw)]
    pairs = list(zip(values[0::2], values[1::2]))
    if not pairs:
        return ""
    path = f"M{pairs[0][0]},{pairs[0][1]}" + "".join(f"L{x},{y}" for x, y in pairs[1:])
    return path + ("Z" if close else "")


def circle_path(cx: float, cy: float, rx: float, ry: float) -> str:
    return (
        f"M{cx - rx},{cy}"
        f"A{rx},{ry} 0 1 0 {cx + rx},{cy}"
        f"A{rx},{ry} 0 1 0 {cx - rx},{cy}Z"
    )


def rect_path(x: float, y: float, width: float, height: float, rx: float, ry: float) -> str:
    rx = min(rx or ry, width / 2)
    ry = min(ry or rx, height / 2)
    if rx <= 0 or ry <= 0:
        return f"M{x},{y}H{x + width}V{y + height}H{x}Z"
    return (
        f"M{x + rx},{y}"
        f"H{x + width - rx}"
        f"A{rx},{ry} 0 0 1 {x + width},{y + ry}"
        f"V{y + height - ry}"
        f"A{rx},{ry} 0 0 1 {x + width - rx},{y + height}"
        f"H{x + rx}"
        f"A{rx},{ry} 0 0 1 {x},{y + height - ry}"
        f"V{y + ry}"
        f"A{rx},{ry} 0 0 1 {x + rx},{y}Z"
    )


def convert(svg: str) -> tuple[str, list[str]]:
    parts: list[str] = []
    unsupported: list[str] = []
    for match in re.finditer(r"<([a-zA-Z]+)((?:\s+[a-zA-Z-]+=\"[^\"]*\")*)\s*/?>", svg):
        tag, raw = match.group(1), match.group(2)
        values = attributes(raw)
        if tag == "svg":
            continue
        if tag == "path":
            if "d" in values:
                parts.append(values["d"].strip())
        elif tag == "circle":
            radius = number(values, "r")
            parts.append(circle_path(number(values, "cx"), number(values, "cy"), radius, radius))
        elif tag == "ellipse":
            parts.append(circle_path(number(values, "cx"), number(values, "cy"), number(values, "rx"), number(values, "ry")))
        elif tag == "line":
            parts.append(f"M{number(values, 'x1')},{number(values, 'y1')}L{number(values, 'x2')},{number(values, 'y2')}")
        elif tag == "rect":
            parts.append(rect_path(
                number(values, "x"), number(values, "y"),
                number(values, "width"), number(values, "height"),
                number(values, "rx"), number(values, "ry"),
            ))
        elif tag == "polyline":
            parts.append(points_path(values.get("points", ""), close=False))
        elif tag == "polygon":
            parts.append(points_path(values.get("points", ""), close=True))
        else:
            unsupported.append(tag)
    return " ".join(part for part in parts if part), unsupported


def main() -> None:
    icons: dict[str, str] = {}
    problems: dict[str, list[str]] = {}
    for file in sorted(SOURCE.glob("*.svg")):
        geometry, unsupported = convert(file.read_text())
        if not geometry:
            problems[file.stem] = unsupported or ["vide"]
            continue
        if unsupported:
            problems[file.stem] = unsupported
        icons[file.stem] = geometry

    TARGET.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "// Généré par scripts/gen-winui-icons.py — ne pas modifier à la main.",
        "// Tracés Lucide (viewBox 24x24) rendus par un Path teinté comme le texte.",
        "",
        "namespace DmxMoney.App;",
        "",
        "public static class LucideIcons",
        "{",
        "    public static readonly IReadOnlyDictionary<string, string> Data = new Dictionary<string, string>",
        "    {",
    ]
    for name, geometry in icons.items():
        lines.append(f'        ["{name}"] = "{geometry}",')
    lines += [
        "    };",
        "",
        "    /// <summary>Tracé de l'icône, ou celui de « Tag » si le nom est inconnu.</summary>",
        "    public static string Geometry(string name)",
        "        => Data.TryGetValue(name, out var geometry) ? geometry : Data[\"Tag\"];",
        "}",
        "",
    ]
    TARGET.write_text("\n".join(lines))
    print(f"{len(icons)} icônes écrites dans {TARGET.relative_to(ROOT)}")
    if problems:
        print("éléments non convertis :", {name: sorted(set(tags)) for name, tags in problems.items()})


if __name__ == "__main__":
    main()
