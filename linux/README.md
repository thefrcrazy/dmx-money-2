# DmxMoney — Linux (GTK4 / libadwaita)

Application GTK4 native écrite en Rust. Elle utilise **directement** `dmx-core` et `dmx-bridge`
(pas de FFI) : l'interface n'affiche que ce que le noyau calcule.

## Arborescence

| Chemin | Rôle |
|---|---|
| `dmx-money-gtk/src/store.rs` | état partagé : moteur, réglages, comptes, filtre global, abonnements |
| `dmx-money-gtk/src/window.rs` | fenêtre : barre latérale, en-tête (filtre de comptes, soldes), messages |
| `dmx-money-gtk/src/pages/` | une page par écran, reconstruite depuis les vues du noyau |
| `dmx-money-gtk/src/forms/` | formulaires en `AdwDialog` : contrôles, entités, données (import, `.dmx`) |
| `dmx-money-gtk/src/charts.rs` | graphiques dessinés avec cairo (mêmes formes que macOS et Windows) |
| `dmx-money-gtk/src/icons.rs` | icônes Lucide symboliques + couleurs de comptes appliquées en CSS |
| `dmx-money-gtk/src/tray.rs` | icône de zone de notification (StatusNotifierItem, via `ksni`) |
| `dmx-money-gtk/src/bridge.rs` | démarrage et pilotage du pont compagnon mobile |
| `dmx-money-gtk/data/` | feuille de style, icônes générées, fichiers `.desktop`, AppStream et MIME |
| `flatpak/com.dmxmoney.app.yml` | manifeste Flatpak |

Le journal et l'échéancier utilisent `GtkColumnView` avec édition en ligne
(`GtkEditableLabel`) ; les formulaires sont des `AdwDialog` ; les réglages présentent le pont
PWA, le QR d'appairage et les mobiles appairés.

## Prérequis

* Rust stable (1.88+)
* GTK 4.12+ et libadwaita 1.5+ avec leurs fichiers de développement

```bash
# Debian / Ubuntu
sudo apt install libgtk-4-dev libadwaita-1-dev libdbus-1-dev pkg-config build-essential
# Fedora
sudo dnf install gtk4-devel libadwaita-devel dbus-devel
# Arch
sudo pacman -S gtk4 libadwaita dbus
```

## Compiler et lancer

```bash
./scripts/build-linux.sh                 # icônes + build release
./scripts/install-linux.sh ~/.local      # installation dans un préfixe
~/.local/bin/dmxmoney
```

Pendant le développement :

```bash
cargo run -p dmx-money-gtk
```

Sur macOS, `scripts/build-linux.sh` sert uniquement de contrôle de compilation
(`brew install gtk4 libadwaita`) ; l'icône de zone de notification n'est compilée que sous Linux.

## Captures de vérification

L'app sait rendre chaque page et chaque formulaire en PNG sans qu'on pilote l'écran, comme le
`SnapshotRunner` de l'app macOS :

```bash
cargo run -p seed-demo -- /tmp/dmx-demo          # jeu de données déterministe
DMXMONEY_SNAPSHOT_DIR=/tmp/dmx-snapshots DMXMONEY_DATA_DIR=/tmp/dmx-demo \
  cargo run -p dmx-money-gtk                     # 18 captures, puis l'app quitte
```

`RUST_LOG=info` journalise chaque capture ; les avertissements GTK apparaissent sur la sortie
d'erreur, ce qui permet de repérer un arbre de widgets incorrect.

## Icônes

`scripts/gen-linux-icons.sh` convertit les SVG Lucide de `shared/icons/lucide` en icônes
symboliques GTK dans `dmx-money-gtk/data/icons/hicolor/scalable/actions`
(`Wallet` → `dmx-wallet-symbolic`). GTK recolore une icône symbolique en forçant `fill` :
les traits Lucide sont donc transformés en surfaces fermées par `tools/gen-symbolic-icons`,
ce qui rend les icônes correctes sur toutes les versions de GTK, avec la couleur du thème
comme avec les 120 couleurs de comptes et de catégories.

Les noms d'icônes stockés en base restent les noms Lucide, identiques sur les trois
plateformes.

## Empaquetage

```bash
# Flatpak (build local, dépendances Cargo téléchargées pendant le build)
flatpak-builder --user --install --force-clean build linux/flatpak/com.dmxmoney.app.yml
flatpak run com.dmxmoney.app

# AppImage
./scripts/build-linux-appimage.sh        # target/appimage/DmxMoney-<version>-x86_64.AppImage
```

Le Flatpak demande : réseau (pont HTTPS local), trousseau
(`org.freedesktop.secrets`, service « DmxMoney Secure Bridge »), `StatusNotifierWatcher`
pour l'icône de zone de notification. Les imports et exports passent par le portail de
fichiers, sans accès disque supplémentaire.

## Données

* Base : `~/.local/share/com.dmxmoney.app/dmxmoney2025.db`
* Reprise automatique de DmxMoney 1.x (`~/.local/share/com.dmxmoney.desktop`) au premier
  lancement ; la base d'origine n'est jamais modifiée. Depuis un Flatpak, le bac à sable ne
  voit pas ce dossier : exportez un `.dmx` depuis la 1.x et importez-le dans Paramètres.
* `DMXMONEY_DATA_DIR` permet de travailler sur un dossier de test (le pont PWA est alors désactivé).
* Sauvegardes `.dmx` compatibles 1.x, type MIME `application/x-dmxmoney-backup`.

## Compagnon mobile

Le pont HTTPS local est le même que sur macOS et Windows (`dmx-bridge`). La PWA est servie
depuis `$PREFIX/share/dmx-money/pwa` quand `pwa/dist` a été construit avant l'installation :

```bash
cd pwa && bun install && bun run build
```

Sans ce dossier, le pont expose l'API locale et la PWA peut être ouverte depuis l'URL publique
du pont managé.
