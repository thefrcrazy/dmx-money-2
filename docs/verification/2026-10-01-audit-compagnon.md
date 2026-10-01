# Audit et compagnon Internet — 1 octobre 2026

## Périmètre et livraison

Revue du noyau Rust, imports/restauration, snapshots et synchronisation, pont mobile et
authentification, PWA, Cloudflare, journal SwiftUI/AppKit/WinUI/GTK et distribution Windows.
Les ressources générées ne sont pas assimilées à du code relu ligne par ligne. Les tests
ne constituent pas une garantie d’absence de tout défaut.

Seul **dmx-money-2** a reçu des modifications de code. **Dmx-Money** est archivé sur GitHub,
sans renommage, sans dernier commit et reste public à la demande de l’utilisateur.
L’archivage conserve l’historique ; un nouveau commit ne masque pas les anciens commits.

Le nouveau Worker et sa PWA sont publiés sur
[le service commun](https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/mobile/), version Worker
`7a88524a-13e4-437a-8b78-b52a5eeb3c71`. L’ancien Worker `managed-bridge` reste séparé.
**Aucune nouvelle release desktop ni aucun installeur Windows signé n’a été publié.**
Les applications déjà installées nécessitent une nouvelle build pour utiliser ce relais.

## Défauts et corrections

| Zone | Problème constaté | Correction |
| --- | --- | --- |
| Accès distant | Le pont historique publiait une adresse DNS locale et un certificat ; il ne transportait pas les demandes depuis la 4G/5G jusqu’au bureau | Connexion WSS sortante du bureau vers un Durable Object par installation et PWA commune, sans domaine, port entrant ou compte Cloudflare individuel |
| Transport | Nécessité de protéger les données contre la lecture par le relais | AES-256-GCM, clés de requête/réponse dérivées par HKDF-SHA256, clé maîtresse uniquement dans le QR et le trousseau du bureau ; CryptoKey non extractables sur le mobile |
| Appairage/session | Course de consommation du QR et sessions conservées après révocation d’une passkey | Consommation conditionnelle atomique du jeton unique, révocation des sessions avec la passkey, migration d’origine invalidant les anciens appairages |
| Relais | Rejeu après reconnexion et flux non bornés | Identifiants anti-rejeu en SQLite, demandes récentes, limites de taille, débit et concurrence ; arrêt immédiat et reconnexion avec temporisation |
| Cycle de vie Worker | Supprimer une inscription supprimait aussi la table SQL de débit ; réinscrire le même objet provoquait une erreur | Suppression sérialisée puis recréation du schéma vide ; test reproduisant l’erreur avant correction |
| Journal Rust | Recalculs répétés des comptes/catégories/budgets et allocations inutiles | Index de recherche et de calcul, réduction des parcours répétés |
| Journal natif | GTK faisait des parcours quadratiques ; WinUI remplaçait inutilement sa source et perdait la sélection ; Apple répétait tris/filtres | Mise à jour GTK en lot, sélection WinUI conservée et rafraîchissements réduits, invalidation explicite des caches Apple ; virtualisation native conservée |
| Journal mobile | Toutes les lignes et groupes étaient construits immédiatement | Affichage progressif de 80 lignes, regroupement linéaire, index des budgets et formatteurs réutilisés |
| Lecture mobile | Téléchargement de toutes les opérations en une seule enveloppe | Pages de 2 000, version bancaire contrôlée, reprise bornée après changement concurrent, cache remplacé après réception complète |
| Imports | Dates/montants invalides transformés ou partiellement écrits ; transactions commencées après certaines lectures ; faux succès mobile | Validation stricte, transactions IMMEDIATE incluant lectures et contrôles, erreurs propagées ; deux achats identiques restent deux achats distincts |
| Sauvegarde | Une restauration pouvait remplacer des données avant validation complète ou séparer paramètres et données | Validation avant remplacement, restauration atomique des données, groupes et paramètres ; rejet des versions futures |
| Mutations synchronisées | Identifiants/timestamps/montants distants insuffisamment vérifiés ; liens de virements pouvant désigner une opération sans rapport | Identité enveloppe/payload contrôlée, dates comparées comme instants, liens réciproques et contreparties cohérentes exigés |
| Cohérence réseau | Export, snapshot et file de sync pouvaient lire des états différents | Une transaction de lecture par snapshot/export/outbox |
| CloudKit | Un accusé de réception pouvait supprimer une édition plus récente ; file de plus de 2 000 bloquée ; changement d’Apple ID non isolé | ACK limité aux séquences envoyées, modification plus récente remise en attente, remplissage après ACK ; suspension et réactivation explicite après changement de compte |
| File PWA | Double sérialisation des mutations banque/paramètres, changement de destination risquant de retargeter des saisies | Sérialisation corrigée, files malformées conservées, destination vérifiée sans utiliser un HTTP 401 comme preuve d’identité |
| PWA publiée | Chemins absolus de manifest, service worker et assets incompatibles avec `/mobile/` | Chemins relatifs, scope et cache de build vérifiés depuis l’adresse publique |
| Windows | EXE/DLL/Setup distribués sans confiance Authenticode publique | Pipeline Azure Artifact Signing Public Trust, OIDC, vérification des signatures/horodatages et du paquet ; publication bloquée si configuration ou signature manque |
| Updater Windows | Source personnalisée insuffisamment restreinte et installation peu explicite | Source HTTPS validée, politique stable/préversion, contrôle SHA-256, vérification et installation séparées avec confirmation avant redémarrage |

## Vérifications réalisées

| Contrôle | Résultat local |
| --- | --- |
| Rust core/bridge/FFI | 134 tests ordinaires réussis ; format et Clippy sans avertissements |
| Transport réel | Test supplémentaire HTTPS/WSS réussi sur le Worker publié : inscription éphémère, appairage chiffré, cookie dans la réponse chiffrée, rejet du même message à 409 et suppression de l’inscription |
| PWA | 64 tests, TypeScript strict et build de production réussis |
| Workers | 6 tests du relais dans le moteur Cloudflare réel ; 5 contrats du pont historique ; typage et empaquetage à blanc réussis |
| Swift | 13 tests DmxKit, dont 3 courses d’ACK CloudKit ; builds macOS modern et AppKit réussis |
| Windows | 34 tests .NET de noyau/modèles réussis ; scripts PowerShell parsés et 5 cas de refus de publication exécutés |
| GTK | Compilation vérifiée ; pas d’exécution du bureau Linux pendant cette passe |
| Navigateur public | PWA à 390 × 844 sans débordement ; service worker actif sous `/mobile/`, manifest/icônes/assets reçus ; aucune erreur JavaScript, CSP ou réseau constatée sur l’écran non appairé |
| Dépendances npm | PWA : aucun avis sur 478 paquets ; relais : aucun avis sur 159 paquets après overrides de deux dépendances de test |
| RustSec | 512 paquets, aucune entrée `vulnerabilities` ; 3 avis conservés détaillés ci-dessous |

Le contrôle NuGet des 25 versions directes/transitives Windows ne relève aucune version
dans les plages vulnérables du flux officiel ; restauration WinUI avec audit transitif
réussie sans avertissement NU190x. La commande de listing échoue sur ce Mac ; elle ne sert
pas de preuve d’absence d’avis. Voir [la vérification Windows](2026-10-01-windows-release.md).

Le noyau du journal a été mesuré en release sur une base synthétique de **100 000 opérations**,
30 comptes et 100 catégories/budgets : médiane **236 ms avant → 180 ms après**, soit environ
24 % de réduction. Cela mesure le calcul du noyau, pas le temps d’ouverture de l’interface.
Les collections natives sont encore chargées en mémoire ; l’affichage virtualisé ne rend
pas toutes les opérations sur la collection indépendantes de sa taille.

## Dépendances et avis conservés

Les outils de test Cloudflare verrouillés tiraient `sharp 0.35.2` et `undici 7.29.0`.
Les overrides imposent `sharp 0.35.4` et `undici 7.29.1`, versions corrigées ; les tests et
l’audit npm ont été rejoués après ce changement. Voir les avis mainteneur
[sharp](https://github.com/advisories/GHSA-rgj7-g3m4-5g8c) et
[undici/TLS](https://github.com/advisories/GHSA-w293-vg96-wgc3).

RustSec a été actualisé le 1 octobre 2026, commit
`3461c0d8f85d084552dd999c58d97c7123a9e0fd`, avec cargo-audit 0.22.2 installé hors du projet.
Les avis restants concernent `clap 2` dans UniFFI CLI et `ksni → dbus-codegen` à la compilation :

- `ansi_term 0.12.1` : [non-maintenance](https://rustsec.org/advisories/RUSTSEC-2021-0139.html).
- `atty 0.2.14` : [non-maintenance](https://rustsec.org/advisories/RUSTSEC-2024-0375.html).
- `atty` : [lecture potentiellement non alignée sous Windows](https://rustsec.org/advisories/RUSTSEC-2021-0145.html), catégorie `unsound`, condition liée à un allocateur personnalisé ; aucun allocateur de ce type trouvé dans le projet.

Le build distribué de `dmx-ffi` n’active pas UniFFI CLI. Ces avis ne sont ni masqués ni
assimilés à une preuve de vulnérabilité exploitable dans l’application.

## Limites et travail restant avant release

- **Windows** : configurer réellement le compte Azure, faire valider l’identité et créer
  le profil Public Trust. Le code ne peut pas fabriquer cette identité. Puis tester une
  installation et une mise à jour x64/arm64 avec Smart App Control actif. Une signature
  valide ne garantit pas l’absence de tout avertissement SmartScreen lié à la réputation.
  Voir [la procédure de signature](../release.md#signature-windows-et-smart-app-control).
- **Compilation native** : les builds Swift locaux utilisent le XCFramework préexistant ;
  les nouvelles sources Rust sont testées séparément. Reconstruire noyau, bindings et binaires
  de distribution avant livraison. WinUI complet doit être compilé/exécuté sur Windows.
- **Catalina** : Xcode 27 installé refuse la cible 10.15. La même interface AppKit compile
  avec une cible 12.0 temporaire en ligne de commande ; la cible projet 10.15 reste conservée.
  Pas de validation sur macOS Catalina réel. Pas de test iCloud réel avec changement d’Apple ID.
- **Confidentialité** : le Worker ne reçoit pas la clé maîtresse ou les données financières
  en clair, mais voit IP, routage, horaires et tailles. Un opérateur qui modifie le JavaScript
  de la PWA peut compromettre le client ; une URL cachée n’est pas une barrière de sécurité.
- **Stockage** : cache financier IndexedDB, SQLite bureau et sauvegardes `.dmx` restent sans
  chiffrement applicatif au repos. La garde passkey de 45 minutes masque l’interface ; elle
  ne chiffre pas les fichiers. Le format `.dmx` utilise base64, qui n’est pas du chiffrement.
- **Disponibilité** : bureau allumé, connecté et application ouverte ; contrôle actif toutes
  les 2,5 secondes, reprise à la réouverture si iOS suspend la PWA. Aucun accès direct à la
  base quand le bureau dort. Les saisies hors ligne sont conservées jusqu’à reconnexion.
- **Migration** : synchroniser les anciennes saisies mobiles avant d’activer le nouveau
  relais ; changement d’origine WebAuthn et nouvel appairage requis. Les anciennes files
  restent dans leur ancien scope au lieu d’être envoyées silencieusement ailleurs.
- **Téléphone réel** : le test réseau Rust/Worker valide appairage et transport chiffré,
  pas le parcours WebAuthn complet sur iPhone/Android ni une édition en 4G après suspension
  et reprise de la PWA. Ces parcours restent à tester sur les appareils concernés.
- **Volumes** : autres collections toujours transmises en bloc dans les limites d’enveloppe,
  journal natif en mémoire, bundle principal PWA d’environ 346 Kio gzip. Mesurer avec une
  base représentative et un vrai téléphone avant de promettre une latence pour tous volumes.

## Références

- [GitHub : archivage et historique](https://docs.github.com/en/repositories/archiving-a-github-repository/archiving-repositories).
- [Cloudflare : WebSockets et hibernation](https://developers.cloudflare.com/durable-objects/best-practices/websockets/).
- [Microsoft : Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview).
- [Microsoft : réputation SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).
- [Protocole et déploiement du relais](../../cloudflare/remote-relay/README.md).
