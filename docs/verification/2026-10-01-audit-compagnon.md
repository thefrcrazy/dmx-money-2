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
`3ae96b65-2692-4033-b844-cd056ad8709f`, cache PWA `2.0.6-2278b075f072404e`.
L’ancien Worker `managed-bridge` reste séparé.
**Aucune nouvelle release desktop ni aucun installeur Windows signé n’a été publié.**
Les applications déjà installées nécessitent une nouvelle build pour utiliser ce relais.

La [CI du code `a2b2e1e`](https://github.com/thefrcrazy/dmx-money-2/actions/runs/36890917651)
a validé Rust, PWA/Workers, Linux, Apple et Windows. Elle reconstruit les bindings et les
binaires à partir des nouvelles sources : GTK en release avec installation et métadonnées,
WinUI x64 avec tests et démarrage/navigation, macOS moderne et Intel ciblant 10.15,
captures AppKit et iOS simulateur arm64. Le job Windows a aussi construit et vérifié
un installeur autosigné de test et ses 277 binaires : 46 signatures de développement
épinglées (Setup inclus) et 232 signatures fournisseurs publiques. Les 54 tests .NET,
dont le verrou de fichier Windows réel, et le démarrage de toutes les pages ont réussi.
Ce passage ne publie pas de release desktop.

## Défauts et corrections

| Zone | Problème constaté | Correction |
| --- | --- | --- |
| Accès distant | Le pont historique publiait une adresse DNS locale et un certificat ; il ne transportait pas les demandes depuis la 4G/5G jusqu’au bureau | Connexion WSS sortante du bureau vers un Durable Object par installation et PWA commune, sans domaine, port entrant ou compte Cloudflare individuel |
| Transport | Nécessité de protéger les données contre la lecture par le relais | AES-256-GCM, clés de requête/réponse dérivées par HKDF-SHA256, clé maîtresse uniquement dans le QR et le trousseau du bureau ; CryptoKey non extractables sur le mobile |
| Appairage/session | Course de consommation du QR et sessions conservées après révocation d’une passkey | Consommation conditionnelle atomique du jeton unique, révocation des sessions avec la passkey, migration d’origine invalidant les anciens appairages |
| Relais | Rejeu après reconnexion et flux non bornés | Identifiants anti-rejeu en SQLite, demandes récentes, limites de taille, débit et concurrence ; arrêt immédiat et reconnexion avec temporisation |
| Compatibilité Rust | La CI avec le nouveau stable refusait une API atomique dépréciée ; son nouveau nom n’existe pas sur le MSRV 1.88 | Compteur de connexions exprimé avec `compare_exchange_weak`, compatible avec les deux toolchains ; contrôles de capacité conservés |
| Cycle de vie Worker | Supprimer une inscription supprimait aussi la table SQL de débit ; réinscrire le même objet provoquait une erreur | Suppression sérialisée puis recréation du schéma vide ; test reproduisant l’erreur avant correction |
| Journal Rust | Recalculs répétés des comptes/catégories/budgets et allocations inutiles | Index de recherche et de calcul, réduction des parcours répétés |
| Journal natif | GTK faisait des parcours quadratiques ; WinUI remplaçait inutilement sa source et perdait la sélection ; Apple répétait tris/filtres | Mise à jour GTK en lot, sélection WinUI conservée et rafraîchissements réduits, invalidation explicite des caches Apple ; virtualisation native conservée |
| Journal mobile | Toutes les lignes et groupes étaient construits immédiatement | Affichage progressif de 80 lignes, regroupement linéaire, index des budgets et formatteurs réutilisés |
| Lecture mobile | Téléchargement de toutes les opérations en une seule enveloppe | Pages de 2 000, version bancaire contrôlée, reprise bornée après changement concurrent, cache remplacé après réception complète |
| Cache mobile | Valeurs financières et corps des saisies hors ligne directement lisibles dans IndexedDB | Enveloppes JSON UTF-8/base64 versionnées, migration transactionnelle conservant messages, ordre, scopes et révisions ; obfuscation sans chiffrement |
| Imports | Dates/montants invalides transformés ou partiellement écrits ; transactions commencées après certaines lectures ; faux succès mobile | Validation stricte, transactions IMMEDIATE incluant lectures et contrôles, erreurs propagées ; deux achats identiques restent deux achats distincts |
| Sauvegarde | Une restauration pouvait remplacer des données avant validation complète ou séparer paramètres et données | Validation avant remplacement, restauration atomique des données, groupes et paramètres ; rejet des versions futures |
| Mutations synchronisées | Identifiants/timestamps/montants distants insuffisamment vérifiés ; liens de virements pouvant désigner une opération sans rapport | Identité enveloppe/payload contrôlée, dates comparées comme instants, liens réciproques et contreparties cohérentes exigés |
| Cohérence réseau | Export, snapshot et file de sync pouvaient lire des états différents | Une transaction de lecture par snapshot/export/outbox |
| CloudKit | Un accusé de réception pouvait supprimer une édition plus récente ; file de plus de 2 000 bloquée ; changement d’Apple ID non isolé | ACK limité aux séquences envoyées, modification plus récente remise en attente, remplissage après ACK ; suspension et réactivation explicite après changement de compte |
| File PWA | Double sérialisation des mutations banque/paramètres, changement de destination risquant de retargeter des saisies | Sérialisation corrigée, files malformées conservées, destination vérifiée sans utiliser un HTTP 401 comme preuve d’identité |
| PWA publiée | Chemins absolus de manifest, service worker et assets incompatibles avec `/mobile/` | Chemins relatifs, scope et cache de build vérifiés depuis l’adresse publique |
| Windows | EXE/DLL/Setup distribués sans confiance Authenticode publique | Pipeline Azure Artifact Signing Public Trust, OIDC, vérification des signatures/horodatages et du paquet ; publication bloquée si configuration ou signature manque |
| Développement Windows | Besoin de builds autosignés sans abonnement ; confusion entre élévation administrateur et confiance du code | Mode de signature locale distinct de la publication publique, clé privée non exportable et vérification d’intégrité ; aucun contournement Smart App Control |
| Updater Windows | Source insuffisamment restreinte, installation peu explicite et cache de paquet réutilisé sans recontrôle par Velopack | HTTPS, politique stable/préversion, paquet complet et contrôle final taille/SHA-256 même en cache, verrou Windows jusqu'au lancement ; confirmation avant redémarrage |

## Vérifications réalisées

| Contrôle | Résultat local |
| --- | --- |
| Rust core/bridge/FFI | 134 tests ordinaires réussis ; format et Clippy sans avertissements |
| Transport réel | Test supplémentaire HTTPS/WSS réussi sur le Worker publié : inscription éphémère, appairage chiffré, cookie dans la réponse chiffrée, rejet du même message à 409 et suppression de l’inscription |
| PWA | 76 tests, TypeScript strict et build de production réussis ; migration/rollback du cache, écriture concurrente, Unicode, ancien onglet bloquant et conservation des données corrompues couverts |
| Workers | 6 tests du relais dans le moteur Cloudflare réel ; 5 contrats du pont historique ; typage et empaquetage à blanc réussis |
| Swift | 13 tests DmxKit, dont 3 courses d’ACK CloudKit ; builds macOS modern et AppKit réussis |
| Windows | 54 tests .NET de noyau/modèles réussis, dont les paquets en cache altérés et les diagnostics des erreurs de politique Windows ; scripts PowerShell et vérification de signature détaillés dans le rapport Windows |
| GTK | Compilation vérifiée ; pas d’exécution du bureau Linux pendant cette passe |
| Navigateur public | Nouvelle PWA à 390 × 844 sans débordement ; bundle `index-Bo9DIAf8.js`, service worker actif sous `/mobile/` et nouveau cache reçus ; aucune erreur JavaScript, CSP ou réseau constatée sur l’écran non appairé |
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

L’obfuscation du cache a été mesurée séparément sous Bun sur ce Mac avec **100 000 opérations** :
JSON UTF-8 **17 579 781 octets**, base64 **23 439 708 caractères**, enveloppe complète
**23 439 761 octets**, soit environ **33,3 %** de plus que le JSON. Ce ratio ne mesure pas
le quota IndexedDB du stockage précédent. Les méthodes base64 natives, détectées à
l’exécution, donnent **40–52 ms** d’encodage et **58–66 ms** de décodage ; le repli pour les
anciens navigateurs donne **213–239 ms** et **61–77 ms**. Ces mesures incluent la conversion
JSON et ne garantissent ni latence ni consommation mémoire sur téléphone. La migration
ajoute un travail linéaire sur les données existantes ; la transaction conserve l’ancien
stockage si elle échoue. Référence des API :
[Uint8Array.toBase64](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8Array/toBase64).

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

- **Windows** : l’utilisateur préfère actuellement une signature autosignée de développement,
  sans abonnement. Elle ne donne pas de confiance publique et le mode administrateur ne
  contourne pas Smart App Control. Pour distribuer avec une signature reconnue, configurer
  réellement le compte Azure, faire valider l’identité et créer le profil Public Trust,
  ou utiliser un autre fournisseur reconnu. Le code ne peut pas fabriquer cette identité. Puis tester une
  installation et une mise à jour x64/arm64 avec Smart App Control actif. Une signature
  valide ne garantit pas l’absence de tout avertissement SmartScreen lié à la réputation.
  Voir [la procédure de signature](../release.md#signature-windows-et-smart-app-control).
- **Compilation native** : les builds Swift locaux utilisent le XCFramework préexistant ;
  les nouvelles sources Rust sont testées séparément en local. La CI a ensuite reconstruit
  noyau, bindings et applications Apple/Windows/Linux, avec démarrage WinUI. Une release
  signée et des essais d’installation restent nécessaires avant distribution publique.
- **Catalina** : Xcode 27 installé refuse la cible 10.15. La même interface AppKit compile
  avec une cible 12.0 temporaire en ligne de commande ; la cible projet 10.15 reste conservée.
  La CI avec son Xcode compatible a réussi la compilation Intel ciblant 10.15. Pas de
  validation sur macOS Catalina réel. Pas de test iCloud réel avec changement d’Apple ID.
- **Confidentialité** : le Worker ne reçoit pas la clé maîtresse ou les données financières
  en clair, mais voit IP, routage, horaires et tailles. Un opérateur qui modifie le JavaScript
  de la PWA peut compromettre le client ; une URL cachée n’est pas une barrière de sécurité.
- **Stockage** : cache financier et corps des messages IndexedDB désormais obfusqués
  en base64, facilement décodables sans clé. Cet encodage ajoute environ 33 % au JSON UTF-8
  et ne chiffre pas le stockage. SQLite bureau et sauvegardes `.dmx` restent sans chiffrement
  applicatif au repos. La garde passkey de 45 minutes masque l’interface ; elle ne chiffre
  pas les fichiers. Le format `.dmx` utilise également base64, qui n’est pas du chiffrement.
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
