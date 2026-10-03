# Correctifs de l’audit DmxMoney V2

3 octobre 2026. Référence avant correction : `402d993062de05f9b5c9a5ccac9e728b23f23700`, version 2.1.0-rc.2. Seul `dmx-money-2` est modifié. Aucun nouveau tag ni déploiement de production dans cette intervention.

Les défauts d’intégrité, de disponibilité et d’interface identifiés ont reçu les corrections ci-dessous. Les risques conditionnels ont été durcis ou explicités. **A6 reste une limite de confidentialité : CloudKit privé ne reçoit pas une nouvelle enveloppe E2EE applicative.** Les validations exécutées sont distinguées des contrôles de plateforme encore nécessaires.

## Noyau, synchronisation et sécurité

| Constat | Correction | Vérification |
| --- | --- | --- |
| B01 — Brouillon obsolète | État initial immuable dans les formulaires et APIs FFI WithBase ; fusion des seuls champs modifiés sous transaction, conflit explicite. SwiftUI, AppKit, WinUI et GTK utilisent le contrat. | Tests Rust de concurrence/rollback ; formulaire Mac fictif ouvert à 47,32 €, montant concurrent 48,32 €, édition du libellé conservant 48,32 €. |
| B02 — Achats identiques fusionnés | Multiplicité CSV/QIF ; identité banque/compte/FITID OFX persistée, UUIDv5 déterministe compatible entre noyau et PWA. Réimport après déplacement manuel reconnu par ID ; mapping final OFX conservant l’identité et solde initial calculé après dédoublonnage. | FITID distincts sur mêmes valeurs conservés ; réimport identique sans nouvelle ligne ; backup/restauration et note locale préservés. |
| B03 — Références orphelines | FK toujours actives ; staging durable des dépendances et contreparties, application atomique des paires et conversions de virements. | Parents reçus après enfants, paires et suppressions en lots séparés, conversions dans les deux ordres, contrôle des FK. |
| B04 — Compteur passkey | Compteur SQL relu et écrit conditionnellement ; challenge, compteur, enrollment et création de session finalisés atomiquement. | Régression 0→5→1 refusée ; compteur zéro synchronisé compatible ; courses challenge/enrollment et rollback. |
| B05 — Création anonyme de stockage | Méthode, format du bearer et quotas contrôlés avant routage DO ; schéma applicatif créé après enrollment valide. | Tests Worker/Miniflare sur IDs inconnus, méthodes et credentials invalides. |
| B06 — Lecteurs/réponses non bornés | Réservation avant lecture ; deux slots fixes de 12 Mio, maintenus pendant lecture, attente et consommation du flux de réponse ; délais 10 s/25 s ; saturation 503 réessayable. PWA limitée à deux appels simultanés. | Saturation, EOF/cancel/error, lecture lente, réponse sans resérialisation et délai réel 25 s→504 puis réutilisation. CORS conservé dans les sorties d’erreur. |
| B07 / P12 — Page trop grosse | Pagination plafonnée à 3 Mio de JSON et 2 000 lignes ; offsets effectifs, réduction adaptative du lot client lors d’un 413, garde dataVersion. | Descriptions fortement échappées, IDs présents exactement une fois ; retry au même offset ; cache conservé en cas d’échec. |
| B08 / P11 — Calcul non borné | 1 826 jours inclusifs et 500 000 points avant allocation ; validation moteur/settings/API/UI ; rattrapage limité à 1 000 occurrences par échéance et appel. | Millions de jours refusés, ancienne récurrence avancée sans itération journalière exhaustive ; aucune écriture partielle de réglages invalides. |
| B09 — Permissions Unix | Dossier 0700, DB/WAL/SHM 0600 ; propriétaire et liens symboliques contrôlés. | Dossier et fichiers fictifs existants durcis depuis 755/644. |
| B10 — Débordement d’agrégats | Intermédiaires en centimes i128 pour soldes, journal, analyses, prévisions, budgets et tableau de bord. | 1 025 valeurs extrêmes admises ne produisent plus de total négatif par débordement i64. |
| Durcissements complémentaires | Compte et groupe atomiques ; nettoyage borné des sessions/challenges/QR ; receipts horodatés conservés 90 jours, anciens receipts sans échéance conservés. | Échec SQL injecté sans écriture partielle ; mutations trop anciennes refusées explicitement sans effacement de la file mobile. |

Sources principales : `core/crates/dmx-core`, `core/crates/dmx-bridge`, `core/crates/dmx-ffi`, `cloudflare/remote-relay/src`. Les régressions métier sont dans `audit_integrity.rs`, les tests auth/companion et les tests Worker.

## PWA

| Constat | Correction | Vérification |
| --- | --- | --- |
| P01 — Refus permanent bloquant toute la file | Quarantaine persistante ; dépendances retenues, changements indépendants poursuivis ; interface d’examen, réessai et abandon explicite de la chaîne restante après lecture serveur valide. | Rejet de compte + opération dépendante bloqués, autre compte transmis ; refus réseau/quota ne supprime ni cache ni file. |
| P02 — Conflits de réglages ignorés | Conflits conservés et champs affichés ; cache et outbox lus dans une même transaction ; générations/revision locale empêchant un vieux refresh d’écraser une édition. | Conflit retenu ; édition concurrente du thème conserve la nouvelle valeur et sa mutation. |
| P03 — Transfert côté revenu | Contrepartie capturée à l’ouverture ; PATCH `/api/transfers` pour les deux côtés avec leurs bases, source/destination distinctes. | Écritures/cache/outbox atomiques ; montant concurrent préservé, conflit sans écriture partielle. |
| P04 — Reload sans échéance | Rechargement conditionné au nombre d’échéances effectivement traitées. | Branche zéro sans relecture complète des collections. |
| P05 — Snapshot complet à chaque édition | IndexedDB stocke les opérations par ID ; cache, révision et outbox modifiés dans une transaction. | Journal fictif de 30 000 lignes : édition = une écriture de ligne, aucune lecture de toutes les lignes ; rollback quota atomique. |
| P06 — Budget recalculé à chaque frappe | Bornes mensuelles mémorisées à partir de la date locale. | Dépendances des calculs indépendantes du texte recherché. |
| P07 — Toutes les cartes d’échéances montées | Un layout actif selon largeur ; fenêtre de cartes, ancrage et maintien de la ligne ayant le focus clavier. | Chromium fictif 390×844, 1 000 échéances : cinq cartes au début, sept à la fin, dernier élément atteint ; nom de compte lisible et aucune carte débordante. Pas de mesure FPS. |
| P08 — Focus hors dialogue | Dialogues natifs nommés, fond inerte, boucle Tab, restauration de focus, popovers dans le dialogue ; scroll interne à toutes largeurs. Fermeture/soumission bloquées pendant écriture et animation de sortie. | TypeScript/build ; contrôles navigateur sur fixture, y compris Escape des popovers. Pas de certification WCAG globale. |
| P09 — Comptes de même nom | Séries et valeurs indexées par ID ; nom utilisé seulement pour l’affichage. | Deux comptes de même nom gardent des valeurs distinctes. |
| P10 — Dates UTC/locales mélangées | Dates bancaires locales explicites et calcul de jours calendaires ; solde d’ouverture excluant correctement le premier jour. | Fuseau négatif, DST, date invalide et borne du premier jour. |
| P11 — Plages personnalisées | Validation partagée des dates/dimensions avant création de séries et message d’erreur visible. | Plages énormes rejetées avant allocation. |
| P12 — Retry pagination | Voir B07 ; réduction du lot sans avancer l’offset après un 413. | Page initiale refusée puis lot réduit complet, sans perte d’ID. |
| Compléments de revue | Imports utilisant les mêmes dialogues, double lancement empêché ; échéance attendue avant fermeture et erreur conservant le brouillon ; libellé de fusion exact. Corps de mutation nuls/primitifs/malformés conservés comme illisibles ; récupération explicite utilise les IDs bruts. | Régressions de corruptions, garde d’IDs lors d’abandon, snapshot cohérent et quota ; libellés d’issues défensifs. Tests de verrouillage/session maintenus après introduction de la concurrence limitée. |

Sources : `pwa/src/services`, `context`, `pages`, `features/import`, `components/ui`, `components/scheduled` et `utils`.

## Apple

| Constat | Correction / traitement | Vérification et portée |
| --- | --- | --- |
| A1 — Suppression iCloud redatée | Tombstone réservé portant le timestamp de l’auteur ; anciennes suppressions physiques suspendent la sync au lieu de fabriquer une date. | Tests CKRecord/outbox, sans compte iCloud physique. Tous les clients doivent être mis à jour. |
| A2 — Updater supprimant aussi le backup | Bundle précédent retenu jusqu’au marqueur de lancement ; rollback possible même si le nouveau bundle demeure ; backup gardé si restauration échoue. | Six scénarios du script de production sur applications/commandes fictives. |
| A3 — Import sur thread UI | Lectures/parsing/preview hors UI ; relevés 16 Mio, backups 64 Mio ; accès security-scoped maintenu durant lecture. | Fichiers fictifs aux limites et limites+1, worker hors thread principal. |
| A4 — Annulation/fermeture tardive | Génération de présentation ; résultat attaché au formulaire ; fermeture et remplacement bloqués pendant écriture commencée. | Régression busy/remplacement/résultat tardif. L’écriture commencée se termine ; elle n’est pas prétendue annulable. |
| A5 — Zone effacée republiée | Zone disparue ou compte changé ⇒ suspension et conservation locale, reprise volontaire. | Branches revues et build ; aucune zone réelle supprimée. |
| A6 — Confidentialité CloudKit | Réglages décrivant explicitement le cloud privé Apple et sa différence avec l’E2EE du compagnon. | **Limite conservée : aucune nouvelle enveloppe fournisseur-opaque CloudKit.** |
| A7 — Mauvaise release sélectionnée | Filtrage canal, architecture, OS et sélection de la plus haute SemVer compatible. | Flux non trié, RC/stable, asset absent, OS incompatible. |
| A8 — Pages cachées/recherche | Modèles paresseux, requêtes hors UI, debounce 150 ms, générations annulant publications périmées. | 100 demandes regroupées en une lecture ; activation/masquage et publication sur UI. |
| A9 — Listes compactes | Conteneurs paresseux sur OS compatibles ; journaux natifs conservés. | Parse/build ; repli Catalina maintenu, sans nouvelle mesure FPS. |
| A10 — État iCloud non publié | État observable et erreurs visibles dans les deux réglages. | Compilation et isolation fictive ; callbacks CloudKit physiques non exécutés. |
| A11 — Entitlements perdus | Résolution des entitlements effectifs ; configuration iCloud refusant la signature ad hoc ; métadonnées cloud neutralisées sans droits. | Trois fixtures de configuration et syntaxe shell. |
| A12 — Divergences modernes | Politique RC cohérente, synchronisation iCloud explicite ; brouillons chargés hors body et erreurs visibles. | Typecheck/build et régressions. |
| H1 — AppIntents | Authentification locale requise ; notifications génériques. | Compilation ; Siri sur appareil verrouillé reste un test physique. |
| H2 — Identité de l’updater | URLs épinglées au dépôt V2 ; signature et TeamID de l’installation exigés pour remplacement automatique ; ad hoc = téléchargement manuel. | Tests de provenance/signature et rollback. Pas de nouveau manifeste signé ni garantie contre une équipe de publication compromise. |
| H3 — Accessibilité | Couleurs/icônes nommées et sélection accessible ; cibles 44 pt sur iOS, action Pointer/Dépointer nommée. | Source/build ; modificateurs d’accessibilité typés pour Intel/Catalina 10.15 ; pas de parcours VoiceOver/Dynamic Type physique. |
| H4 — Index AppKit obsolète | ID et révision capturés au début, édition invalidée lors d’un reload, APIs avec base. | Test métier noyau ; variante AppKit typée sur ARM, runtime Intel à confirmer en CI. |

Sources : `apple/Packages/DmxKit`, variantes iOS/macOS, `scripts/build-macos.sh`, `scripts/tests/test-macos-updater.py`.

## Windows / Linux

| Constat | Correction | Vérification |
| --- | --- | --- |
| WL-01 | Inspection de sauvegarde acceptant timestamp absent avec affichage adapté. | Fixture et tests .NET. |
| WL-02 | Erreurs de préparation/import/restauration visibles ; dialogue et brouillon conservés, état busy cohérent. | Tests ViewModels et contrôles de dialogue. |
| WL-03 | Avertissement Fusionner décrivant les collisions d’IDs et le traitement réel des réglages. | Sources des interfaces alignées au noyau. |
| WL-04 | Filtres de catégories reconstruits depuis les catégories actuelles tout en conservant la sélection valide. | Régressions ViewModels/source UI. |
| WL-05 | Décodage Windows-1252 effectif, UTF-8/UTF-16 et erreur explicite si illisible. | Tests euro/apostrophes et BOM. |
| WL-06 | GIO HANDLES_OPEN et traitement de fichiers de sauvegarde au démarrage/instance existante. | Compilation/source ; association Linux physique à confirmer. |
| WL-07 | Références faibles et déconnexion des signaux à la fermeture des formulaires GTK. | 20 cycles réels GTK/libadwaita sur Mac sans base financière, objets libérés. Test ajouté à CI Linux sous Xvfb. |
| WL-08 | Événements de statut séparés des changements de données, canal borné et coalescence. | Revue/couverture événementielle et compilation. |
| WL-09 | Graphiques réagissant à la publication de la vue plutôt qu’à chaque notification dérivée. | Tests ViewModels ; rendu WinUI final confié à CI. |
| WL-10 | Préparation des imports hors UI, aperçu limité et formats/tailles contrôlés ; confirmation await et fermeture bloquée durant écriture. | 100 000 lignes fictives : constructeur ~0,03 ms/2 128 octets, préparation ~119–120 ms en worker. Baseline ~214,89 ms/~8,98 Mo dans le constructeur. Aucun gain FPS déduit. |
| WL-11 | Windows App SDK 2.5.1 et BuildTools 10.0.26100.4654 compatibles ; toolchain Rust alignée 1.94.0. | Tests .NET locaux ; compilation WinUI x64/ARM64 en CI requise. |
| WL-C01 | Identité d’édition capturée ; réaffectation/reload annule un brouillon périmé. | 11 scénarios utilisant EditableCell réel avec stubs d’événements WinUI ; test ajouté en CI. |
| WL-O01 | Noms et états sélectionnés accessibles des boutons d’accent Windows. | Source ; Narrator/Orca physiques non exécutés. |

Sources : `windows/src`, `windows/tests`, `linux/dmx-money-gtk/src`, `linux/dmx-money-gtk/tests`.

## Validations exécutées

- Rust noyau/bridge/FFI : **166 tests réussis**, un smoke réseau ignoré ; format et Clippy all-targets `-D warnings` réussis.
- PWA : **119 tests réussis**, 572 assertions ; TypeScript, ESLint et build de production réussis.
- Worker : **16 tests réussis** sous Miniflare, dont délai réel de 25 s ; typage réussi.
- .NET : **73 tests réussis** avec la FFI host fraîche ; 11 scénarios EditableCell séparés.
- Apple : **30 XCTest réussis**, build moderne ARM/ad hoc vérifié ; AppKit typé sur ARM et tests du script updater/entitlements réussis.
- GTK : quatre tests unitaires, 20 cycles de libération de contrôles réels ; vérification host du tray ksni 0.3.6.
- Interface Mac pilotée avec `DMXMONEY_DATA_DIR` et bundle fictif distinct : édition concurrente, recherche, budget, échéancier, analyses. iCloud, compagnon et updater désactivés dans ce dossier. À la fermeture, l’outil a repris la fenêtre installée ; le pilotage a été arrêté sans modification de cette fenêtre.

La CI de cette pull request vérifie Windows x64, Mac moderne/Intel et iOS, ainsi que Linux. Windows ARM64 est construit par le workflow de release ; cette architecture n’est pas exécutée dans la CI de pull request. Son état est publié sur la pull request ; les résultats locaux ci-dessus ne remplacent pas ces builds.

## Limites de sécurité et compatibilité

Le cache PWA conserve l’obfuscation réversible demandée. La base native et les backups restent non chiffrés ; permissions OS ≠ chiffrement. Un code PWA remplacé à l’origine peut accéder aux données côté client. CloudKit privé conserve la limite A6 explicitée ci-dessus.

SQLx 0.9.0 embarque SQLite **3.51.3** via libsqlite3-sys 0.37.0 ; ce n’est pas la dernière version globale de SQLite. Le mode DEFENSIVE et `trusted_schema=OFF` sont activés/vérifiés. Les préconditions SQL arbitraire/FTS5/DEFENSIVE désactivé des avis récents ne sont pas exposées par ces parcours ; cela ne remplace pas un patch du code C. La contrainte SQLx `<0.38` empêche une simple mise à jour vers les bindings plus récents. [Documentation SQLx](https://docs.rs/sqlx/0.9.0/sqlx/), [avis SQLite officiels](https://sqlite.org/cves.html).

Les quotas du relais sont locaux à chaque lieu Cloudflare et éventuellement cohérents ; aucun plafond global exact ni pic heap réel mesuré n’est annoncé. Le budget de 24 Mio réserve les frames, il ne représente pas toute la mémoire de l’isolate. [Quotas Cloudflare](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/), [limites Workers](https://developers.cloudflare.com/workers/platform/limits/#memory).

Rust/MSRV 1.94 ; macOS moderne 26, Intel legacy 10.15, iOS 17. La lib ARM host marque bien un minimum 11.0, sans imposer macOS 27. Le SDK Rust Flatpak externe doit également fournir Rust ≥1.94 au build. VoiceOver, Narrator, Siri verrouillé, iCloud entre appareils et Safari mobile physique restent des validations distinctes ; aucun chiffre FPS/4G réel ni certification de sécurité n’est revendiqué.

Les nouvelles identités OFX ne peuvent pas reconstituer avec certitude les FITID des anciennes opérations. Une première réimportation enrichie peut conserver une apparente duplication : les données existantes ne sont pas supprimées par heuristique.
