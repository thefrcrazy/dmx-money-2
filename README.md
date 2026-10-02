# DmxMoney 2

DmxMoney est une application de gestion financière personnelle, locale et privée : comptes, journal de transactions, budgets, échéancier, analyses et prédictions de trésorerie.

La version 2 est une réécriture **native sur chaque plateforme** autour d'un **noyau Rust partagé**.

| Plateforme | Interface | Minimum |
| --- | --- | --- |
| macOS Apple Silicon (« modern ») | SwiftUI natif (Table, Form, Swift Charts, SF Symbols, Liquid Glass) | macOS 26 Tahoe |
| macOS Intel (« legacy ») | AppKit + SwiftUI compatible | macOS 10.15 Catalina |
| iOS / iPadOS | SwiftUI | iOS 17 |
| Windows (x64, arm64) | WinUI 3 + C# | Windows 10 1809 |
| Linux (x64) | GTK4 + libadwaita (Rust) | GTK 4.12 / libadwaita 1.5 |

## Architecture

```text
.
├── core/crates/dmx-core/        # modèles, SQLite, règles métier, imports, .dmx, sync
├── core/crates/dmx-bridge/      # pont compagnon mobile (HTTPS local, passkeys, ACME)
├── core/crates/dmx-ffi/         # façade UniFFI (Swift, C#)
├── core/crates/uniffi-bindgen/  # génération des bindings Swift
├── core/fixtures/               # jeux de données de test
├── apple/                       # apps macOS, iOS et iPadOS (XcodeGen)
├── windows/                     # app WinUI 3 (.NET)
├── linux/dmx-money-gtk/         # app GTK4/libadwaita
├── pwa/                         # client web du compagnon mobile, embarqué par les apps de bureau
├── cloudflare/managed-bridge/   # Worker du pont HTTPS managé
├── cloudflare/remote-relay/     # relais Internet chiffré, PWA commune et connexion sortante
├── shared/                      # icônes (catalogue, correspondances natives, kit GNOME), logos
├── tools/seed-demo/             # jeu de données de démonstration (captures, essais)
└── scripts/                     # builds et génération des bindings
```

Principe directeur : **les interfaces n'effectuent aucun calcul métier**. Soldes, budgets, projections et séries de graphiques sont calculés par `dmx-core`, ce qui garantit des résultats identiques sur toutes les plateformes.

## Données

- Base SQLite `dmxmoney2025.db`, schéma compatible avec DmxMoney 1.x.
- Au premier lancement, la base de DmxMoney 1.x (`com.dmxmoney.desktop`) est copiée si elle existe. L'originale n'est jamais modifiée.
- Sauvegardes `.dmx` compatibles 1.x (JSON encodé en base64) : export, restauration complète ou fusion.

## Synchronisation mobile

Deux options indépendantes, activables dans Paramètres :

- **iCloud** : synchronisation entre Mac, iPhone et iPad (macOS 14+ / iOS 17+).
- **Compagnon PWA distant** : le desktop ouvre une connexion sortante au relais commun
  Cloudflare. La [PWA hébergée sur Cloudflare Pages](https://dmxmoney-companion.pages.dev/mobile/)
  transmet les modifications au bureau par le Worker et sa connexion WebSocket.
  Le téléphone fonctionne en Wi-Fi, 4G ou 5G, avec appairage QR, passkey et
  transport chiffré entre appareils. Le PC doit rester allumé et DmxMoney ouvert.
  Aucun DNS n’est à configurer par l’utilisateur. Les installations du pont local
  existant restent disponibles en mode « Accès local hérité » ; sélectionner
  « Passer à l’accès Internet » après synchronisation des anciennes saisies mobiles.
  Les appairages du précédent relais nécessitent « Mettre à jour le compagnon » pour
  changer d'origine vers Pages. Un nouvel appairage est nécessaire dans les deux cas.

Les garanties, le cache hors ligne et les limites de confidentialité sont détaillés dans
[le fonctionnement du relais](cloudflare/remote-relay/README.md) et [la synchronisation mobile](docs/mobile-sync.md).
Le cache mobile est obfusqué en base64 pour masquer la lecture directe ; cet encodage
reste décodable et ne remplace pas le chiffrement ni la protection du téléphone.
Les corrections, mesures et limites de validation sont recensées dans
[l’audit du 1 octobre 2026](docs/verification/2026-10-01-audit-compagnon.md).

## Assistant (Siri, compagnon mobile)

`dmx-core::assistant` comprend une demande courte en français et y répond avec **ses** chiffres :
« ajoute 12,50 € en alimentation », « quel est mon solde ? », « combien me reste-t-il en
carburant ? », « prochaines échéances », « résumé du mois », « traiter les échéances dues ».
L'analyse est déterministe et testée ; aucun montant ne sort d'un modèle.

* **macOS** : sept intentions App Intents (`DmxIntents.swift`) et leurs phrases Siri, plus les
  Raccourcis. Chaque intention appelle le noyau, qui résout aussi les noms de compte et de
  catégorie — Siri peut dire « en alimentation » sans connaître d'identifiant.
* **Compagnon mobile** : la PWA envoie la phrase à `POST /api/assistant` du pont local. L'app de
  bureau la fait d'abord normaliser par son **modèle sur l'appareil** (Apple Intelligence sur
  macOS 26, `AssistantRewriter`) quand il est disponible, puis le noyau l'interprète et calcule la
  réponse. Sans modèle — Mac Intel, Apple Intelligence absente, Linux, Windows — la même analyse
  déterministe s'applique et la réponse est identique.
* Le modèle ne touche jamais aux données : il réécrit la phrase, rien d'autre. La même mécanique
  s'ouvre aux hôtes Windows et Linux via le rappel `rephrase_assistant_request`.

## Développement

Prérequis communs : Rust stable (1.88+).

```bash
cargo test --workspace
```

Les instructions propres à chaque plateforme sont dans [apple/README.md](apple/README.md), [windows/README.md](windows/README.md) et [linux/README.md](linux/README.md) ; la publication est décrite dans [docs/release.md](docs/release.md).

Sous Windows, les builds de développement peuvent être autosignés localement sans abonnement.
Cette signature ne garantit pas l’acceptation par Smart App Control ; les mises à jour restent
soumises aux règles d’exécution de Windows. Voir [la procédure Windows](docs/release.md#windows).

Pour essayer l'application sans toucher à vos données :

```bash
cargo run -p seed-demo -- /tmp/dmx-demo
DMXMONEY_DATA_DIR=/tmp/dmx-demo cargo run -p dmx-money-gtk    # ou l'app macOS / Windows
```

## Licence

MIT, voir [`LICENSE`](LICENSE). Chaque app affiche les icônes de son système : SF Symbols sur Apple, Segoe Fluent Icons sous Windows, icônes symboliques GNOME sous Linux : thème [Adwaita](https://gitlab.gnome.org/GNOME/adwaita-icon-theme) (LGPL v3 ou CC BY-SA 3.0, voir `shared/icons/adwaita`) et [GNOME Icon Development Kit](https://gitlab.gnome.org/Teams/Design/icon-development-kit) (CC0, voir `shared/icons/gnome-kit`). La PWA et macOS 10.15 utilisent [Lucide](https://lucide.dev) (licence ISC, voir `shared/icons/lucide/LICENSE`).
