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

* Rust stable (1.94+)
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

Les tests d'import et de coalescence des notifications ne nécessitent aucune base utilisateur :

```bash
cargo test -p dmx-money-gtk
# Objets GTK réels : fermeture des formulaires et libération des contrôles.
xvfb-run cargo test -p dmx-money-gtk --features ui-tests --test ui-lifecycle
```

Le second test nécessite un affichage GTK disponible (`xvfb-run` sur une CI Linux). Il vérifie
les références après fermeture, sans ouvrir de base ni simuler une session financière réelle.

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

Les icônes sont celles de GNOME : icônes symboliques du thème Adwaita quand il en a un
équivalent, sinon du GNOME Icon Development Kit (licence CC0). Toutes sont embarquées dans
`dmx-money-gtk/data/icons/hicolor/scalable/actions` sous un nom préfixé (`dmx-adwaita-…`,
`dmx-…`) : elles s'affichent même sans thème Adwaita installé (autre bureau, AppImage).
`scripts/gen-native-icons.py` génère la correspondance (`src/icon_names.rs`) depuis
`shared/icons/native.json` et copie les icônes ; GTK les recolore avec la couleur du thème
comme avec les 120 couleurs de comptes et de catégories.

Les noms d'icônes stockés en base restent les noms Lucide, identiques sur les trois
plateformes.

## Empaquetage

```bash
# Flatpak (build local, dépendances Cargo téléchargées pendant le build)
flatpak install -y flathub org.gnome.Platform//50 org.gnome.Sdk//50 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08
flatpak-builder --user --install --force-clean build linux/flatpak/com.dmxmoney.app.yml
flatpak run com.dmxmoney.app

# AppImage
./scripts/build-linux-appimage.sh        # target/appimage/DmxMoney-<version>-x86_64.AppImage
```

L'AppImage publiée est construite sous Ubuntu 24.04. La CI utilise `appimagetool` sans
`linuxdeploy` : GTK 4.12+ et libadwaita 1.5+ doivent donc être présents sur la machine.
Les distributions plus anciennes ne sont pas validées ; le Flatpak fournit le runtime GNOME 50.
Le SDK GNOME 50 sélectionne l'extension Rust stable sur la branche freedesktop 25.08.

Le Flatpak demande : réseau (connexion sortante au relais Internet), trousseau
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

Dans les paramètres, activer le compagnon puis scanner le QR sur le téléphone. Le relais
chiffré (`dmx-bridge`) utilise le même Worker commun que macOS et Windows, avec une
connexion WebSocket sortante. La PWA est hébergée sur
[Cloudflare Pages](https://dmxmoney-companion.pages.dev/mobile/). Aucun DNS personnel,
certificat local ou port entrant n'est à configurer. Le téléphone fonctionne en Wi-Fi,
4G ou 5G tant que l'ordinateur est allumé, connecté à Internet et DmxMoney ouvert.

Les paquets AppImage et Flatpak n'embarquent pas de PWA. Le client mobile est publié
séparément avec `scripts/deploy-pwa.sh` ; voir [les garanties de synchronisation](../docs/mobile-sync.md).
