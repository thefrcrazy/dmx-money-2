#!/usr/bin/env python3
"""Génère les icônes natives de chaque plateforme depuis shared/icons/native.json.

Les noms d'icônes stockés en base sont ceux du catalogue Lucide, que seule la PWA dessine tel
quel. Les apps natives affichent les icônes de leur système :
  - Windows : windows/src/DmxMoney.App/Icons/FluentIcons.g.cs (glyphes Segoe Fluent Icons ; sous
    Windows 10, dont la police Segoe MDL2 Assets n'a pas tous ces glyphes, « mdl2 » remplace
    ceux qui y manquent) ;
  - Apple : apple/Packages/DmxKit/Sources/DmxKit/Design/SymbolNames.swift (SF Symbols) ;
  - Linux : linux/dmx-money-gtk/src/icon_names.rs et les icônes symboliques GNOME, embarquées
    sous un nom préfixé pour être trouvées quel que soit le thème installé : thème Adwaita
    (shared/icons/adwaita) et GNOME Icon Development Kit, CC0 (shared/icons/gnome-kit).

Usage :
  gen-native-icons.py [--check] [--fetch --adwaita liste.txt] [--fluent IconsData.json] [--mdl2 page.md]
    --check    compare au lieu d'écrire (CI) : code de sortie 1 si un fichier n'est pas à jour ;
    --fetch    télécharge les icônes GNOME qui manquent dans shared/icons ;
    --adwaita  liste « catégorie/nom-symbolic » du thème Adwaita : vérifie les noms et situe les
               fichiers à télécharger ;
    --fluent   vérifie les glyphes contre IconsData.json de la galerie WinUI ;
    --mdl2     vérifie que Windows 10 a chaque glyphe, d'après le tableau de la page Segoe MDL2
               Assets de la documentation Microsoft (windows-dev-docs, segoe-ui-symbol-font.md).
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "shared" / "icons" / "native.json"
CATALOGUE = ROOT / "shared" / "icons" / "icons.json"
VENDORED = {
    "adwaita": ROOT / "shared" / "icons" / "adwaita",
    "kit": ROOT / "shared" / "icons" / "gnome-kit",
}
URLS = {
    "adwaita": "https://raw.githubusercontent.com/GNOME/adwaita-icon-theme/master/Adwaita/symbolic/{category}/{name}-symbolic.svg",
    "kit": "https://gitlab.gnome.org/Teams/Design/icon-development-kit/-/raw/main/icons/{name}.svg",
}
WINDOWS = ROOT / "windows" / "src" / "DmxMoney.App" / "Icons" / "FluentIcons.g.cs"
APPLE = ROOT / "apple" / "Packages" / "DmxKit" / "Sources" / "DmxKit" / "Design" / "SymbolNames.swift"
LINUX = ROOT / "linux" / "dmx-money-gtk" / "src" / "icon_names.rs"
LINUX_ICONS = ROOT / "linux" / "dmx-money-gtk" / "data" / "icons" / "hicolor" / "scalable" / "actions"
HEADER = "Généré par scripts/gen-native-icons.py depuis shared/icons/native.json : ne pas modifier à la main."
GLYPH_FONTS = {"fluent": "Fluent", "mdl2": "MDL2"}


def load() -> tuple[dict[str, dict[str, str]], str]:
    data = json.loads(SOURCE.read_text(encoding="utf-8"))
    return data["icons"], data["fallback"]


def gnome(entry: dict[str, str]) -> tuple[str, str]:
    source, _, icon = entry.get("gnome", "").partition(":")
    return source, icon


def vendored(source: str, icon: str) -> Path:
    return VENDORED[source] / (f"{icon}-symbolic.svg" if source == "adwaita" else f"{icon}.svg")


def adwaita_categories(listing: Path | None) -> dict[str, str]:
    if not listing:
        return {}
    categories = {}
    for line in listing.read_text().splitlines():
        if "/" in line:
            category, name = line.strip().rsplit("/", 1)
            categories[name.removesuffix("-symbolic")] = category
    return categories


def mdl2_codes(page: Path | None) -> set[str]:
    """Codes des glyphes listés dans les tableaux Markdown de la page Segoe MDL2 Assets."""
    if not page:
        return set()
    codes = set()
    for line in page.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("|"):
            codes |= {cell.strip().upper() for cell in line.split("|") if re.fullmatch(r"\s*[0-9A-Fa-f]{4}\s*", cell)}
    return codes


def validate(icons: dict[str, dict[str, str]], fallback: str, fluent: Path | None, mdl2: Path | None, categories: dict[str, str]) -> list[str]:
    errors = []
    catalogue = json.loads(CATALOGUE.read_text(encoding="utf-8"))["icons"]
    errors += [f"{name} : absent de native.json (catalogue shared/icons/icons.json)" for name in catalogue if name not in icons]
    if fallback not in icons:
        errors.append(f"icône de repli {fallback} absente")
    glyphs = {}
    if fluent:
        glyphs = {item["Code"]: item["Name"] for item in json.loads(fluent.read_text(encoding="utf-8-sig"))}
    windows10 = mdl2_codes(mdl2)
    errors += [f"aucun glyphe lu dans {page}" for page, codes in ((fluent, glyphs), (mdl2, windows10)) if page and not codes]
    for name, entry in icons.items():
        if not re.fullmatch(r"[a-z0-9]+(\.[a-z0-9]+)*", entry.get("sf", "")):
            errors.append(f"{name} : SF Symbol invalide {entry.get('sf')!r}")
        for key in [key for key in GLYPH_FONTS if key == "fluent" or key in entry]:
            match = re.fullmatch(r"([0-9A-F]{4}) (\w+)", entry.get(key, ""))
            if not match:
                errors.append(f"{name} : glyphe {GLYPH_FONTS[key]} invalide {entry.get(key)!r}")
            elif glyphs and glyphs.get(match.group(1)) != match.group(2):
                errors.append(f"{name} : {match.group(1)} est {glyphs.get(match.group(1), 'inconnu')} dans Segoe Fluent Icons, pas {match.group(2)}")
        if windows10:
            if "mdl2" in entry and entry["fluent"][:4] in windows10:
                errors.append(f"{name} : « mdl2 » inutile, Segoe MDL2 Assets a déjà {entry['fluent']}")
            elif entry.get("mdl2", entry.get("fluent", ""))[:4] not in windows10:
                errors.append(f"{name} : {entry.get('mdl2', entry.get('fluent'))} absent de Segoe MDL2 Assets (Windows 10) : ajouter un glyphe « mdl2 »")
        source, icon = gnome(entry)
        if source not in VENDORED or not re.fullmatch(r"[a-z0-9-]+", icon):
            errors.append(f"{name} : icône GNOME invalide {entry.get('gnome')!r}")
        elif source == "adwaita" and categories and icon not in categories:
            errors.append(f"{name} : {icon} absent du thème Adwaita")
        elif not vendored(source, icon).exists():
            errors.append(f"{name} : {vendored(source, icon).relative_to(ROOT)} absent (--fetch)")
    return errors


def fetch(icons: dict[str, dict[str, str]], categories: dict[str, str]) -> None:
    for source, icon in sorted({gnome(entry) for entry in icons.values()}):
        target = vendored(source, icon)
        if target.exists() or source not in VENDORED:
            continue
        if source == "adwaita" and icon not in categories:
            continue  # signalé par la validation ; la catégorie vient de la liste --adwaita
        target.parent.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(URLS[source].format(name=icon, category=categories.get(icon, ""))) as response:
            target.write_bytes(response.read())
        print(f"{source} : {target.name} téléchargée")


def linux_name(entry: dict[str, str]) -> str:
    source, icon = gnome(entry)
    return f"dmx-{icon}-symbolic" if source == "kit" else f"dmx-{source}-{icon}-symbolic"


def csharp_glyphs(icons: dict[str, dict[str, str]], key: str) -> list[str]:
    lines = []
    for name in sorted(icons):
        if key in icons[name]:
            code, label = icons[name][key].split(" ", 1)
            lines.append(f'        ["{name}"] = "\\u{code}", // {label}')
    return lines


def render(icons: dict[str, dict[str, str]], fallback: str) -> dict[Path, str]:
    names = sorted(icons)
    windows = [
        f"// {HEADER}",
        "#nullable enable",  # le compilateur désactive les annotations dans les fichiers .g.cs
        "namespace DmxMoney.App;",
        "",
        "/// <summary>Glyphes Segoe Fluent Icons des noms d'icônes stockés en base.</summary>",
        "internal static class FluentIcons",
        "{",
        "    private static readonly Dictionary<string, string> Glyphs = new()",
        "    {",
        *csharp_glyphs(icons, "fluent"),
        "    };",
        "",
        "    /// <summary>Remplaçants sous Windows 10 : sa police Segoe MDL2 Assets n'a pas ces glyphes.</summary>",
        "    private static readonly Dictionary<string, string> Windows10Glyphs = new()",
        "    {",
        *csharp_glyphs(icons, "mdl2"),
        "    };",
        "",
        "    /// <summary>Segoe Fluent Icons n'est fournie avec le système qu'à partir de Windows 11.</summary>",
        "    private static readonly bool HasFluentFont = OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000);",
        "",
        f"    /// <summary>Glyphe du nom donné, celui de « {fallback} » pour un nom inconnu.</summary>",
        "    public static string Glyph(string? name)",
        "    {",
        f'        var key = name is not null && Glyphs.ContainsKey(name) ? name : "{fallback}";',
        "        return !HasFluentFont && Windows10Glyphs.TryGetValue(key, out var glyph) ? glyph : Glyphs[key];",
        "    }",
        "}",
        "",
    ]
    apple = [f"// {HEADER}", "", "/// SF Symbols des noms d'icônes stockés en base.", "enum SymbolNames {", "    static let map: [String: String] = ["]
    apple += [f'        "{name}": "{icons[name]["sf"]}",' for name in names]
    apple += ["    ]", "}", ""]
    linux = [
        f"//! {HEADER}",
        "",
        "/// Icône symbolique GNOME d'un nom stocké en base, embarquée par l'app : thème Adwaita",
        "/// (préfixe « dmx-adwaita- ») ou GNOME Icon Development Kit (préfixe « dmx- »).",
        "pub fn native(name: &str) -> &'static str {",
        "    match name {",
    ]
    linux += [f'        "{name}" => "{linux_name(icons[name])}",' for name in names]
    linux += [f'        _ => "{linux_name(icons[fallback])}",', "    }", "}", ""]
    return {WINDOWS: "\n".join(windows), APPLE: "\n".join(apple), LINUX: "\n".join(linux)}


def linux_icons(icons: dict[str, dict[str, str]]) -> dict[str, bytes]:
    return {f"{linux_name(entry)}.svg": vendored(*gnome(entry)).read_bytes() for entry in icons.values()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--adwaita", type=Path)
    parser.add_argument("--fluent", type=Path)
    parser.add_argument("--mdl2", type=Path)
    arguments = parser.parse_args()

    icons, fallback = load()
    categories = adwaita_categories(arguments.adwaita)
    if arguments.fetch:
        fetch(icons, categories)
    errors = validate(icons, fallback, arguments.fluent, arguments.mdl2, categories)
    if errors:
        print("\n".join(f"!! {error}" for error in errors), file=sys.stderr)
        return 1

    outputs = render(icons, fallback)
    bundled = linux_icons(icons)
    stale = [path for path, content in outputs.items() if not path.exists() or path.read_text(encoding="utf-8") != content]
    existing = {path.name for path in LINUX_ICONS.glob("*.svg")} if LINUX_ICONS.exists() else set()
    stale_icons = [name for name, content in bundled.items() if name not in existing or (LINUX_ICONS / name).read_bytes() != content]
    extra_icons = sorted(existing - set(bundled))

    if arguments.check:
        problems = [str(path.relative_to(ROOT)) for path in stale] + stale_icons + [f"en trop : {name}" for name in extra_icons]
        if problems:
            print("!! icônes natives à régénérer (scripts/gen-native-icons.py) :\n  " + "\n  ".join(problems), file=sys.stderr)
            return 1
        print(f"icônes natives à jour ({len(icons)} noms, {len(bundled)} icônes GNOME embarquées)")
        return 0

    for path in stale:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(outputs[path], encoding="utf-8")
        print(f"écrit : {path.relative_to(ROOT)}")
    LINUX_ICONS.mkdir(parents=True, exist_ok=True)
    for name in extra_icons:
        (LINUX_ICONS / name).unlink()
    for name in stale_icons:
        (LINUX_ICONS / name).write_bytes(bundled[name])
    print(f"Linux : {len(bundled)} icônes GNOME embarquées ({len(stale_icons)} écrites, {len(extra_icons)} retirées)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
