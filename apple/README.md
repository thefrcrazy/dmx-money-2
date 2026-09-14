# DmxMoney — Apple (macOS, iOS, iPadOS)

Une seule application par plateforme, sur le noyau Rust partagé exposé en Swift par UniFFI.
L'interface n'effectue aucun calcul métier : soldes, budgets, projections et séries de
graphiques viennent de `dmx-core`.

| Cible | Interface | Minimum | Architecture |
|---|---|---|---|
| `DmxMoney-macOS-Modern` | SwiftUI natif macOS 26 : `NavigationSplitView`, barre d'outils unifiée, `Table`, `Form`, Swift Charts (survol et bulles), SF Symbols, Liquid Glass | macOS 26 Tahoe | arm64 (Apple Silicon) |
| `DmxMoney-macOS` | AppKit + SwiftUI compatible Catalina (contrôles et graphiques maison) | macOS 10.15 Catalina | x86_64 (Intel) |
| `DmxMoney-iOS` | SwiftUI (`TabView` sur iPhone, `NavigationSplitView` sur iPad) | iOS / iPadOS 17 | arm64 |

Comme en DmxMoney 1.x, macOS a **deux builds séparés** (et non un binaire universel). Ici les
deux ont en plus leur propre interface : la variante *modern* est une vraie app macOS 26 (les
API modernes n'existent pas sous Catalina), la variante *legacy* garde l'interface compatible.
Les deux partagent DmxKit (store, modèles de page, formulaires) et le noyau Rust, donc les
montants et les règles restent identiques.

| Dossier | Contenu |
|---|---|
| `DmxMoney-macOS-Modern` | app SwiftUI : `DmxMoneyModernApp`, `ModernShell`, pages natives, tous les formulaires en `Form` groupés (assistant d'import, restauration `.dmx`, suggestions, nouveautés comprises), compagnon mobile natif (QR CoreImage), page Paramètres dans la fenêtre, correspondance Lucide → SF Symbols |
| `DmxMoney-macOS` | app AppKit : `AppDelegate`, `MainWindowController`, `JournalViewController`, menus |
| `DmxMoney-macOS-Shared` | code commun aux deux : recherche de mise à jour, panneaux de fichiers, démarrage du pont, reprise d'une base 1.x, mode capture |

## Interface de la variante moderne

* **Filtre de comptes et soldes dans la barre d'outils**, après le titre (`AccountSelector`,
  `ToolbarBalances`). Le détail s'ouvre en popover : une case par compte, avec son icône, sa
  couleur et son solde. Sans filtre, tous les comptes comptent — les cases sont cochées — et
  cocher un seul compte l'isole. Le sélecteur n'apparaît que sur les pages qu'il filtre : la vue
  d'ensemble, « Mes Comptes », les catégories et les réglages s'en passent (il revient si un
  filtre est actif, pour expliquer les montants).
* **Recherche repliée en loupe** (`ToolbarSearch`) qui s'ouvre au clic ou par ⌘F, et se referme
  avec Échap : `searchToolbarBehavior(.minimize)` est réservé à iOS, et le champ système de la
  barre d'outils ne se replie que lorsque la place manque.
* **Sélecteurs en popover** (`FilterSelector`) pour les catégories, types, états, budgets et
  fréquences : un `Menu` SwiftUI n'affiche pas les images de ses `Toggle` sur macOS, les icônes
  demandées disparaissaient. Les lignes (`SelectorRow`) dessinent elles-mêmes leur case à cocher :
  un `Toggle` teinte tout son libellé avec la couleur d'accentuation, et les icônes de comptes ou
  de catégories ressortaient toutes en bleu au lieu de garder leur couleur.
* **Largeurs fixes dans la barre d'outils** : le nom du compte est tronqué (128 pt) et chaque
  solde occupe 104 pt, pour que la barre ne respire pas au fil des montants.
* **Édition en ligne du journal** (`InlineTextCell`) : la saisie reste locale et n'est envoyée au
  noyau qu'à la validation ou en quittant le champ. Écrire à chaque frappe rechargeait la table
  sous le curseur, ce qui faisait perdre le focus et déplaçait la ligne en cours de frappe.
* **Dates** : champ à incréments plus un calendrier complet en popover (`DayField`).
* **Paramètres dans la fenêtre** (`ModernSettings`, quatre onglets : Général, Compagnon mobile,
  Données, À propos), en colonne alignée à gauche et une icône par ligne. ⌘, sélectionne cette
  page ; il n'y a pas de fenêtre Réglages séparée.
* **Pied de barre latérale ancré en bas** comme en 1.x : Catégories, Paramètres, recherche de
  mise à jour, Quitter (en rouge, sans confirmation). C'est une seconde liste de barre latérale,
  pour que les lignes gardent la géométrie de celles du dessus.
* **Montants toujours avec les centimes** : aucune vue n'arrondit à l'euro.
* **Barre d'outils par page** : pas de « + », de rechargement ni de recherche sur la vue
  d'ensemble, qui est une page de lecture.
* **Filtres alignés à droite**, actions et suggestions à gauche, sur toutes les pages.
* **Tables** (journal, échéancier, catégories) : sélection, colonne d'actions
  pointer / modifier / supprimer, menu contextuel, Échap pour désélectionner.
* **Graphiques** : survol avec bande translucide et trait de repère (`ChartHoverMark`), identique
  sur les analyses et les prédictions, bulle opaque (`HoverBubble`) et montants
  colorés (rouge sous zéro, orange sous le seuil d'alerte). Toute série tracée doit figurer dans
  `chartForegroundStyleScale` — une série absente du domaine fait échouer Swift Charts (c'était
  le cas du point bas journalier).
* **Prédictions** : le point bas journalier est affiché par défaut ; sur une base neuve, la
  projection démarre au jour courant et non au 1er du mois (le défaut de colonne SQLite reste
  celui de la 1.x, seule la ligne créée à l'initialisation diffère).

## Arborescence

| Chemin | Rôle |
|---|---|
| `Packages/DmxKit/Frameworks/DmxCoreFFI.xcframework` | noyau Rust (généré) |
| `Packages/DmxKit/Sources/DmxCore/Generated` | bindings Swift UniFFI (générés) |
| `Packages/DmxKit/Sources/DmxKit/Store` | `AppStore`, modèles de page, navigation |
| `Packages/DmxKit/Sources/DmxKit/Pages` | les neuf écrans, partagés macOS / iOS |
| `Packages/DmxKit/Sources/DmxKit/Forms` | formulaires (entités, données, assistant d'import) |
| `Packages/DmxKit/Sources/DmxKit/Charts` | graphiques en `Path` (compatibles 10.15) |
| `Packages/DmxKit/Sources/DmxKit/Design` | couleurs, icônes Lucide (`DmxIcon`) |
| `Packages/DmxKit/Sources/DmxKit/Sync` | synchronisation iCloud (`CKSyncEngine`) |
| `DmxMoney-macOS` | coquille AppKit : menus, fenêtre, journal `NSTableView`, icône de la barre de menus |
| `DmxMoney-iOS` | application SwiftUI iPhone / iPad |
| `project.yml` | définition XcodeGen du projet |

Contraintes Catalina respectées dans DmxKit : pas de `@Observable`, `LazyVGrid`, `Label`,
`Menu` ni `ProgressView`, et `objectWillChange.send()` à la place de `@Published` dans les
sous-classes d'`ObservableObject`.

## Prérequis

```bash
brew install xcodegen
rustup target add aarch64-apple-darwin x86_64-apple-darwin \
                  aarch64-apple-ios aarch64-apple-ios-sim
xcodebuild -downloadPlatform iOS       # runtime du simulateur
```

## Construire

```bash
# App macOS complète (noyau, icônes, projet, build) dans dist/<variante>/DmxMoney.app
./scripts/build-macos.sh               # modern : SwiftUI natif, Apple Silicon, macOS 26+
./scripts/build-macos.sh legacy        # legacy : AppKit, Intel, macOS 10.15 Catalina
./scripts/build-macos.sh universal     # interface legacy, deux architectures (essai local)

# Étapes séparées, pour itérer
./scripts/build-apple-xcframework.sh   # noyau Rust + bindings Swift (PROFILE=debug pour itérer)
./scripts/sync-apple-icons.sh          # icônes Lucide → catalogue d'assets
./scripts/sync-apple-app-icons.sh      # icône d'application (macOS + iOS)
cd apple && xcodegen generate

# iOS sur simulateur (tranche arm64 ; IOS_SIM_X86=1 ajoute le simulateur Intel)
xcodebuild -project apple/DmxMoney.xcodeproj -scheme DmxMoney-iOS \
    -destination 'generic/platform=iOS Simulator' ARCHS=arm64 ONLY_ACTIVE_ARCH=NO build
```

Tests du noyau vus depuis Swift :

```bash
swift test --package-path apple/Packages/DmxKit
```

## Signature et iCloud

`Config/Base.xcconfig` ne contient aucune identité. Pour signer et activer la synchronisation
iCloud, créer `apple/Config/Signing.local.xcconfig` (non versionné) :

```
DEVELOPMENT_TEAM = XXXXXXXXXX
DMX_ENTITLEMENTS_SUFFIX = -iCloud
DMX_ICLOUD_CONTAINER = iCloud.com.dmxmoney.app
DMX_UPDATE_FEED_URL = https://github.com/<owner>/<repo>/releases/latest/download/updates.json
```

Sans ce fichier, les cibles utilisent `DmxMoney.entitlements` (distribution directe, sans
CloudKit) et l'interrupteur iCloud reste masqué — comme sous macOS 13 et antérieur, où
`CKSyncEngine` n'existe pas. Le pont compagnon PWA, lui, fonctionne dès Catalina.

## Mises à jour

`UpdateChecker` lit le JSON publié à `DMX_UPDATE_FEED_URL` (au plus une vérification par jour,
plus l'entrée de menu « Rechercher les mises à jour… »), annonce la nouvelle version et ouvre
la page de téléchargement. Le flux contient une entrée par architecture
(`darwin-arm64`, `darwin-x86_64`) : un Mac Intel sous Catalina reçoit le DMG legacy, jamais
celui d'Apple Silicon. Sparkle 2 n'est pas utilisé : ses binaires officiels sont compilés
pour macOS 11 et empêcheraient le lancement sous Catalina. Voir `docs/release.md`.

## Captures de vérification

Les deux apps savent rendre chaque page et chaque formulaire en PNG (revue visuelle, CI), comme
l'app GTK :

```bash
cargo run -p seed-demo -- /tmp/dmx-demo   # jeu de données déterministe (optionnel)
DMXMONEY_SNAPSHOT_DIR=/tmp/dmx-snapshots DMXMONEY_DATA_DIR=/tmp/dmx-demo \
  /path/to/DmxMoney.app/Contents/MacOS/DmxMoney
```

`SnapshotRunner` rend chaque page hors écran (`cacheDisplay`) puis quitte, en deux images :
`<page>.png` (le contenu) et `<page>-chrome.png` (la fenêtre entière, barre d'outils et barre
latérale comprises). Pour la variante
modern, lancer l'app par LaunchServices (`open`), seule voie qui crée la fenêtre d'une app
SwiftUI ; les variables passent alors par `launchctl setenv`. Les matériaux (barre latérale,
barre d'outils, Liquid Glass) ne se rendent pas hors écran : ces zones apparaissent en noir
dans les captures, l'app à l'écran est correcte.

Sur le simulateur iOS, les variables passent par `SIMCTL_CHILD_` :

```bash
xcrun simctl install booted <chemin>/DmxMoney.app
SIMCTL_CHILD_DMXMONEY_DATA_DIR=/tmp/dmx-demo \
  SIMCTL_CHILD_DMXMONEY_SNAPSHOT_DIR=/tmp/dmx-ios-snapshots \
  xcrun simctl launch booted com.dmxmoney.app
```

L'app iOS rend la fenêtre clé (feuilles comprises) puis quitte.

## Siri et Raccourcis (variante moderne)

`DmxIntents.swift` déclare sept intentions App Intents — ajouter une opération, solde, budget
restant, prochaines échéances, résumé du mois, traiter les échéances dues, demande libre — et
`DmxShortcuts` leurs phrases Siri. Elles tournent dans le processus de l'app, appellent
`dmx-core` et lisent la phrase qu'il renvoie : aucune intention ne calcule de montant.

`AssistantRewriter` branche le modèle sur l'appareil (FoundationModels, macOS 26, Apple Silicon)
sur `AppStore.assistantRewriter`. Le pont s'en sert pour normaliser les demandes libres de la PWA
avant de les passer au noyau ; sans Apple Intelligence, le hook vaut `nil` et le noyau analyse la
phrase telle quelle. L'état est affiché dans Paramètres → À propos.

**Deux conditions pour que Raccourcis et Siri joignent l'app** (sinon : « couldn't communicate
with the app ») :

1. *Signature scellée, avec un Team ID.* Sans équipe de développement, l'éditeur de liens ne
   signe que l'exécutable (`adhoc,linker-signed`, aucune ressource scellée) : `codesign --verify`
   échoue. Et même scellée, une signature ad hoc n'a pas de Team ID : l'index des intentions
   (`linkd`) ignore l'app. `build-macos.sh` signe donc avec la première identité « Apple
   Development » du trousseau (un compte Apple gratuit suffit, `DMX_SIGN_IDENTITY` pour en forcer
   une) ; l'ad hoc n'est qu'un repli, sans Siri ni Raccourcis.
2. *Une seule copie connue de LaunchServices* pour `com.dmxmoney.app`. Chaque build Xcode
   enregistre son produit intermédiaire, et la variante legacy porte le même identifiant : le
   système peut lancer une copie sans intentions. Le script désenregistre le produit intermédiaire
   et n'enregistre que la variante native de la machine.

3. *Aucune autre copie visible de Spotlight.* `linkd` redécouvre les apps par Spotlight : des
   builds laissées dans `target/` ou une variante legacy dans `dist/` suffisent à rendre l'index
   ambigu, et Raccourcis obtient `LNMetadataProviderErrorDomain 9004 « Empty result »`. `target/`
   et `dist/legacy/` contiennent donc un `.metadata_never_index`.

Après une nouvelle build, **quitter l'app déjà ouverte** : les intentions sont servies par le
processus en cours, pas par le binaire sur disque. Si l'index reste vide, `killall linkd`
(l'agent est relancé à la demande) puis rouvrir l'app, qui republie ses raccourcis
(`updateAppShortcutParameters`).

**Retour visible.** Chaque intention renvoie sa phrase comme valeur (utilisable par « Afficher
le résultat »), une vue sous la réponse, et publie une notification : lancée depuis Raccourcis
sans interface, une intention ne montrait rien même quand l'opération était enregistrée. Les
comptes et catégories sont des `AppEntity` : Raccourcis et Siri proposent la liste au lieu d'un
texte libre.

**Langue de Siri.** Les phrases sont déclarées en français et traduites en anglais dans
`AppShortcuts.xcstrings` : Siri réglé en anglais comprend « What's my balance in DmxMoney »,
en français « Quel est mon solde dans DmxMoney ». Titres, descriptions et paramètres des
intentions sont eux aussi traduits (`Localizable.xcstrings`) : Siri s'en sert pour relier une
demande libre à la bonne intention. Les réponses, produites par le noyau, restent en français.

Siri n'oriente vers l'app que si la phrase **contient son nom** : « Quel est mon solde dans
DmxMoney », « Solde DmxMoney », « Combien il me reste sur DmxMoney ». « Quel est mon solde »
seul reste une question pour Siri.

Vérifier que les intentions sont bien exposées :

```bash
python3 -c "import json;print(list(json.load(open('dist/modern/DmxMoney.app/Contents/Resources/Metadata.appintents/extract.actionsdata'))['actions']))"
```

## Compagnon mobile et PWA

> **Les deux versions partagent le pont.** DmxMoney 1.x et 2.x utilisent la même entrée de
> trousseau (`DmxMoney Secure Bridge`), donc le même sous-domaine et le même certificat. Si la
> 1.x tourne, elle garde le port habituel et un mobile déjà appairé continue de lui parler — avec
> l'ancien client PWA. Pour tester la 2.x : quitter la 1.x, puis réappairer le mobile avec le QR
> de la 2.x (le port change, donc le lien d'appairage aussi). La page Compagnon mobile affiche
> l'adresse réellement servie et rappelle ce point.


Les apps de bureau **embarquent le client PWA** (`pwa/dist`, construit par
`scripts/build-pwa.sh` et copié dans `Resources`). Le pont local le sert lui-même et le QR
d'appairage pointe vers le pont : le mobile charge donc la PWA de *cette* version, et non celle
déployée sur le Worker Cloudflare. Sans ce dossier, le pont redirige vers la PWA publique
(comportement 1.x).

```bash
./scripts/build-pwa.sh          # pwa/dist
./scripts/build-macos.sh        # embarque pwa/dist dans l'app
```

## Reprise d'une base DmxMoney 1.x

Au lancement, si le dossier de données contient déjà une base **moins complète** qu'une base 1.x
trouvée à côté (cas d'un dossier `com.dmxmoney.app` laissé par un ancien build 1.x), l'app le
détecte et propose la reprise : la base actuelle est exportée en `.dmx` dans le dossier de
données, la base 1.x est copiée en lecture seule (`VACUUM INTO`), et « Ne plus demander » écrit
un marqueur `.legacy-ignored`.

La proposition vient de la fenêtre, jamais du démarrage : la variante SwiftUI l'affiche en
alerte système (`ModernLegacyAdoption`, appelée depuis la vue), la variante AppKit en `NSAlert`
une fois la fenêtre à l'écran. Ouvrir un panneau modal pendant la construction des scènes
SwiftUI relance le rendu au milieu d'une transaction, et AttributeGraph arrête l'application.
La proposition attend une fenêtre visible, et « Plus tard » est le bouton par défaut : ni la
touche Entrée ni une alerte fermée automatiquement ne peuvent déclencher la reprise.

Pour éprouver le parcours sans toucher aux données réelles :

```bash
open -n -g --fresh -a dist/modern/DmxMoney.app \
  --env DMXMONEY_DATA_DIR=/tmp/dmx-essai \
  --env DMXMONEY_LEGACY_DB=/tmp/dmx-essai/v1.db
```

## Données

* Base : `~/Library/Application Support/com.dmxmoney.app/dmxmoney2025.db`
* Reprise automatique de DmxMoney 1.x (`com.dmxmoney.desktop`) au premier lancement ;
  la base d'origine n'est jamais modifiée.
* `DMXMONEY_DATA_DIR` permet de travailler sur un dossier de test (pont PWA désactivé),
  `DMXMONEY_LEGACY_DB` (chemins séparés par `:`) y ajoute des bases 1.x à reprendre.
* Sauvegardes `.dmx` compatibles 1.x, déclarées comme type de document de l'app macOS.
