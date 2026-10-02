# Artefacts macOS et Linux 2.0.9 — 2 octobre 2026

Les quatre paquets publics macOS/Linux de la [release v2.0.9](https://github.com/thefrcrazy/dmx-money-2/releases/tag/v2.0.9) ont été téléchargés et inspectés sans installation ni lancement. Leurs tailles et SHA-256 correspondent aux métadonnées des assets GitHub. La publication date du `2026-10-02T13:03:53Z` et cible le commit `afe2676009c8b287bf2a2d77f39d24ccd8cff178`, construit par le [workflow 37008470476](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37008470476).

## Paquets publics

| Asset | Octets | Version | Architecture | Minimum / runtime vérifié |
| --- | ---: | --- | --- | --- |
| `DmxMoney-2.0.9-apple-silicon.dmg` | 11 337 891 | 2.0.9, build 6 | arm64 | macOS 26.0 |
| `DmxMoney-2.0.9-intel-catalina.dmg` | 11 733 354 | 2.0.9, build 6 | x86_64 | macOS 10.15 |
| `DmxMoney-2.0.9-x86_64.AppImage` | 8 976 888 | AppStream 2.0.9 | ELF x86_64 | Dépendances GTK/libadwaita du système hôte |
| `com.dmxmoney.app.flatpak` | 6 275 512 | AppStream 2.0.9 | ELF x86_64 | `org.gnome.Platform/x86_64/50`, SDK `org.gnome.Sdk/x86_64/50` |

| Asset | SHA-256 conforme à GitHub |
| --- | --- |
| Apple Silicon DMG | `af4b380777591c30b6908c0ea8e5a6678004e587cbd6026357833734e8a8c1c2` |
| Intel/Catalina DMG | `e94f00d9432418abc98e2cd6f8e702b81f99f768fae9f0cf63a55f6667188859` |
| AppImage | `725df8abf9db560b122932a1e56ff452a7dd7322e414e13392c034aa171024c9` |
| Flatpak | `7ab00712f0a3be3aa47ae9e0b9d1d924a0dc3a0bc996fa6cc8ba0ac7b1add839` |

## macOS et flux de mise à jour

Les `Info.plist` des deux applications indiquent `CFBundleShortVersionString=2.0.9` et `CFBundleVersion=6`. `lipo` confirme une seule architecture par paquet. Les minima de `LSMinimumSystemVersion` concordent avec les commandes de chargement Mach-O : 26.0 pour Apple Silicon et 10.15 pour Intel.

`codesign --verify --deep --strict --verbose=2` réussit sur les deux applications. Leur identité est `Signature=adhoc`, `TeamIdentifier=not set`. Cela valide l’intégrité structurelle de la signature ; cela ne fournit pas une identité Apple Developer ID ni une notarisation.

Le champ `DmxUpdateFeedURL` embarqué dans les deux applications pointe vers `https://api.github.com/repos/thefrcrazy/dmx-money-2/releases`. Le [updates.json publié](https://github.com/thefrcrazy/dmx-money-2/releases/download/v2.0.9/updates.json) annonce 2.0.9 et associe `darwin-arm64` au DMG Apple Silicon, minimum 26.0, et `darwin-x86_64` au DMG Intel, minimum 10.15. Les URL correspondent aux assets publics de cette release.

## Nouveau compagnon Pages dans les binaires

Le défaut `https://dmxmoney-companion.pages.dev/mobile/` est présent dans le Mach-O arm64 et les deux ELF Linux. Dans le Mach-O Intel, LLVM construit cette chaîne par six écritures immédiates dans `dmx_bridge::companion::relay::companion_url` : cinq blocs de 8 octets aux offsets 0, 8, 16, 24 et 32, puis 4 octets à l’offset 40. La reconstruction statique produit exactement l’URL de 44 octets, même si elle n’existe pas sous forme de constante contiguë dans le fichier.

Le nouveau message public « aucun DNS individuel n’est nécessaire » est également présent dans les deux Mach-O. Cette vérification confirme que la logique Pages récente est embarquée ; elle n’exécute pas l’activation d’un compagnon utilisateur.

## PWA embarquée

Les quatre paquets contiennent les mêmes 23 fichiers PWA, comparés fichier par fichier par SHA-256 à la référence locale 2.0.9 conservée avant publication. Aucun fichier distribué ne diffère. La métadonnée Finder locale `.DS_Store`, absente des quatre paquets, a été conservée séparément et exclue de la référence applicative.
Ces 23 fichiers incluent les métadonnées d'hébergement `_headers` et `_redirects`.
Le déploiement Pages exclut ces deux fichiers de `/mobile/` et définit ses règles
à la racine : les 21 fichiers applicatifs publics restent identiques.

- Cache : `dmxmoney-shell-2.0.9-a9ced5e45d71a6f4`.
- Bundle principal : `assets/index-Gsv97oo8.js`, SHA-256 `a418a65c4502f222d87a7d6e0da313a6b7059723c1102d3bfe6de6d45f64f66b`.
- 15 entrées `BUILD_ASSETS`, 5 entrées `APP_SHELL` et 10 références HTML contrôlées : aucune référence manquante ni sortie de l’arborescence attendue.

La PWA se trouve dans `Contents/Resources/dist` sur macOS, `usr/share/dmx-money/pwa` dans l’AppImage et `files/share/dmx-money/pwa` dans le Flatpak.

## Méthode, preuves et limites

Les DMG ont été montés en lecture seule avec `-nobrowse -noautoopen`, puis détachés ; aucun montage de cette inspection ne subsiste. Le SquashFS de l’AppImage a été lu avec [PySquashfsImage 0.9.0](https://github.com/matteomattei/PySquashfsImage), depuis une wheel isolée dans `/private/tmp`, sans exécuter son runtime. Le Flatpak a été inspecté par lecture du delta OSTree/GVariant et décompression des objets, sans installation. Seuls les fichiers PWA, métadonnées et binaires nécessaires à ces contrôles ont été extraits dans `/private/tmp`.

Les preuves locales sont conservées dans `/private/tmp/dmx-release-2.0.9-desktop-artifacts/` : `release-metadata.json`, `desktop-proof-summary.json`, `inspection.json` pour macOS, `intel-pages-code-proof.json`, `appimage-static/inspection.json`, `flatpak-inspection.json` et `flatpak-static-report.jsonl`.

Aucune application publiée n’a été lancée, aucune application utilisateur n’a été remplacée et aucune base de données utilisateur n’a été ouverte. Ces contrôles ne constituent pas un essai de lancement sur Catalina, macOS 26 ou Linux, ni une vérification matérielle de Face ID ou d’un téléphone en 4G/5G. La preuve du trajet chiffré de mutation distante et du déploiement Pages est documentée séparément dans [le rapport du compagnon 2.0.9](2026-10-02-pages-companion-2.0.9.md).
