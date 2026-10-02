# Compagnon Cloudflare Pages 2.0.9 — 2 octobre 2026

## État de la livraison

La [PWA publique Cloudflare Pages](https://dmxmoney-companion.pages.dev/mobile/)
et son Worker de relais sont déployés et vérifiés. Les sources du correctif sont
au commit `afe2676009c8b287bf2a2d77f39d24ccd8cff178`, fusion de la
[PR #3](https://github.com/thefrcrazy/dmx-money-2/pull/3). Son arbre est identique
au commit `7bdd249ad7aea589164ef67e2db73bb088ee1eaa` validé en CI.

La [CI multiplateforme](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37006945200)
est réussie sur ses cinq contrôles : Rust, PWA/Worker, Apple, Windows et Linux.
La [release 2.0.9](https://github.com/thefrcrazy/dmx-money-2/releases/tag/v2.0.9)
est publiée depuis le 2 octobre 2026 à 15:03:53 Europe/Paris (13:03:53 UTC),
avec 23 assets non vides, chacun muni d'une empreinte SHA-256 GitHub.
La [construction de publication](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37008470476)
est réussie sur ses sept jobs : version, deux DMG macOS, Windows x64 et ARM64,
AppImage, Flatpak et publication. Aucun fichier de clé privée n'est publié.
Le flux macOS public `updates.json` annonce 2.0.9 et les deux DMG correspondant
à macOS 26 Apple Silicon et 10.15 Intel. Les contrôles détaillés des artefacts sont
recensés dans les rapports Windows et desktop liés ci-dessous. Le
[rapport de la livraison 2.0.8](2026-10-02-release-2.0.8.md) décrit son état historique.

Seul le dépôt V2 a été modifié. Le dépôt V1 reste archivé et public.
Le diagnostic des 57,3 Go locaux et le nettoyage préparé sont détaillés dans
[le rapport de stockage](2026-10-02-build-storage.md) ; aucun cache utilisateur
n'a été supprimé pendant cette passe.

Le [rapport Windows 2.0.9](2026-10-02-windows-2.0.9.md) confirme la réutilisation
du certificat stable Developmax / Collignon Maxim, les signatures Authenticode
de Setup et du paquet complet sur les runners, et les 13 petits assets publics
téléchargés avec tailles et SHA-256 conformes. Les deux flux Windows annoncent
uniquement leur paquet complet 2.0.9. L'autosignature reste sans garantie
d'acceptation par Smart App Control.
Le [rapport des artefacts macOS/Linux](2026-10-02-desktop-artifacts-2.0.9.md)
confirme les versions 2.0.9/build 6, architectures et minima des DMG,
les signatures ad hoc valides, le runtime Flatpak GNOME 50 et la PWA embarquée
identique sur les quatre paquets. Le défaut Pages du noyau Intel est confirmé
par reconstruction statique de l'adresse dans ses instructions machine.

## Diagnostic de la migration 2.0.8

L'installation existante inspectée conservait une identité du pont DNS historique
et ne possédait aucune configuration dans `remote_relay`. Le comportement livré
en 2.0.8 pouvait conserver ce pont local lors de l'activation : un compagnon
« actif » n'attestait donc pas d'une connexion au relais Internet. Le téléphone
restait tributaire de l'adresse HTTPS locale et du même réseau que le bureau.
L'existence du Worker public ne suffisait pas à rendre cette installation accessible
depuis la 4G ou la 5G.

Le correctif fait de l'activation une action explicite vers le relais Internet,
y compris pour une installation avec identité DNS héritée. Un échec réseau conserve
la configuration et les appairages précédents ; il ne se rabat plus sur le pont DNS
en annonçant une activation Internet réussie. Une nouvelle installation ne demande
aucun DNS individuel, port entrant ni compte Cloudflare à son utilisateur.

Le simple démarrage ou la lecture de l'état préserve les appairages existants.
SwiftUI moderne, l'interface AppKit/SwiftUI compatible Catalina, WinUI et GTK proposent :

- **« Passer à l'accès Internet »** pour un pont local hérité, avec une explication
  explicite de sa limite au même réseau.
- **« Mettre à jour le compagnon »** pour l'ancien relais officiel dont la PWA
  utilise encore l'origine Worker. Cette détection se limite au relais officiel,
  afin de préserver les hébergements personnalisés.

La confirmation demande de synchroniser d'abord les anciennes saisies en attente.
Le changement d'identité ou d'origine WebAuthn révoque atomiquement les passkeys,
sessions et QR inutilisés précédents. Le QR conservé en mémoire par le bureau est
également effacé après activation : il ne peut plus être rendu avec un ancien jeton
sur la nouvelle origine Pages. Une activation répétée à la même origine préserve
le nouveau QR valide. Le switch WinUI est recalé sur l'état réel pendant la confirmation,
y compris si celle-ci est annulée.

## Flux réellement implémenté

```text
PWA Cloudflare Pages
  → POST HTTPS contenant une enveloppe chiffrée
  → Worker / Durable Object de l'installation
  → connexion WSS sortante déjà ouverte par le bureau
  → déchiffrement, session/CSRF et règles métier du noyau
  → mutation SQLite et notification de l'interface native
```

La réponse suit le chemin inverse. Le navigateur utilise HTTPS ; il n'ouvre pas
un WebSocket direct vers le bureau. La connexion WSS est établie par l'application
de bureau vers le Worker. Pages héberge la PWA ; le Worker et ses Durable Objects
acheminent les messages. La base de référence reste sur le bureau.

Une modification téléphone → bureau est appliquée et notifie l'interface native
dès réception et validation, sans attendre le polling du téléphone. Dans l'autre
sens, la PWA active vérifie `dataVersion` toutes les **2,5 secondes**, puis recharge
les données si cette version change (`pwa/src/context/BankContext.tsx`). Le délai
réel comprend le réseau et le chargement ; aucune latence fixe n'est garantie.

AES-256-GCM protège les enveloppes ; HKDF-SHA256 sépare les clés requête/réponse.
Le Worker reçoit les identifiants de transport et le contenu chiffré, sans clé maîtresse.
Les cookies de session et le CSRF passent dans les enveloppes chiffrées, sans devenir
des cookies de l'origine du relais. Le bureau reste responsable de l'autorisation
et de la validation des mutations.

## Déploiements publics vérifiés

| Élément | Valeur |
| --- | --- |
| PWA de production | `https://dmxmoney-companion.pages.dev/mobile/` |
| Déploiement Pages | `45ef9209-26c2-404e-8f11-c54fc10c8cbd` |
| URL propre au déploiement | `https://45ef9209.dmxmoney-companion.pages.dev` |
| Fin du déploiement Pages | 2 octobre 2026 à 14:26:05, Europe/Paris (12:26:05 UTC) |
| API du relais | `https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev` |
| Version Worker vérifiée | `c5e0b11c-ec36-48bf-b3eb-da05100f6c93` |
| Cache du service worker | `dmxmoney-shell-2.0.9-a9ced5e45d71a6f4` |
| Bundle principal | `assets/index-Gsv97oo8.js` |

Ce sont les déploiements finaux, effectués après exclusion des `.DS_Store` du build.
Le Worker inutilisé `dmxmoney-companion` a été supprimé après autorisation explicite.
Un contrôle après suppression confirme Pages en HTTP 200, le protocole du relais actif
`dmx-relay-v1` et l'ancien Worker inutilisé en HTTP 404. Le projet Pages du même nom
est conservé.
Le script Pages ajoute un `404.html` racine pour désactiver son fallback SPA implicite,
conformément au [comportement documenté de Pages](https://developers.cloudflare.com/pages/configuration/serving-pages/#single-page-application-spa-rendering).
Les pages et le service worker sont servis avec `Cache-Control: no-store`.

Les empreintes SHA-256 publiques correspondent au build local `pwa/dist` :

| Fichier | SHA-256 |
| --- | --- |
| `index.html` | `914d6f7b5fd03e3fb28742683d6812ab8a8c5d77c00fc0933df20bd1f9f5d183` |
| `sw.js` | `b2f698bb2f0449075a5ab5b4462af837732b4d68753af3cf01e51a40eab06f19` |
| `assets/index-Gsv97oo8.js` | `a418a65c4502f222d87a7d6e0da313a6b7059723c1102d3bfe6de6d45f64f66b` |

## Preuve de mutations sur le relais public

Le test
`companion::relay::tests::local_cloudflare_worker_round_trip_uses_encryption_auth_and_replay_protection`
dans `core/crates/dmx-bridge/src/companion/relay.rs` a réussi contre le Worker HTTPS/WSS
public, avec l'origine Pages.

Le test utilise le vrai noyau et une base SQLite **en mémoire**, une identité de relais
aléatoire, un QR éphémère et des données entièrement synthétiques. Il crée une session
finalisée comme fixture de test, représentant l'état après une authentification passkey.
Il ne réalise pas la cérémonie biométrique d'un téléphone réel.

Assertions vérifiées pendant ce passage réseau réel :

- Connexion WSS du bureau et préflight Pages 204 avec origine exacte.
- Identifiant de transport incorrect refusé en 401 avant transmission au bureau.
- Démarrage de l'appairage chiffré accepté ; rejeu de la même enveloppe refusé en 409.
- Session d'appairage non finalisée et CSRF incorrect refusés en 401, sans mutation
  ni notification native.
- Création d'un compte, création puis édition et suppression d'une opération par
  HTTPS → Worker → WSS → handlers du bureau → SQLite.
- Snapshots du noyau confirmant les écritures ; lecture distante confirmant l'édition.
- **Quatre callbacks `data_changed`**, un par mutation acceptée, avec des versions
  strictement croissantes et une dernière version égale à celle du snapshot SQLite.
- Session révoquée refusée en 401, sans cinquième callback.
- Suppression de l'inscription du relais en 204, puis nouvelle demande refusée en 404.

Le nettoyage de l'identité éphémère est prévu aussi en cas d'échec d'assertion.
Aucune donnée financière utilisateur n'a été utilisée ou modifiée.
Le log local de cette exécution est
`/private/tmp/dmx-money-pages-live-mutation-proof.log` : un test réussi, zéro échec.

## QA navigateur et tests

Le contrôle de l'adresse de production a été effectué dans Chromium via ego-browser
à **390 × 844**, sur l'écran non appairé :

- Largeur de document de 390 pixels, aucun débordement horizontal ; écran d'installation
  rendu et capture inspectée.
- Aucune exception JS, rejection non gérée ou violation CSP observée.
- Service worker activé et contrôleur `https://dmxmoney-companion.pages.dev/mobile/sw.js`,
  scope `/mobile/`, cache exact ci-dessus ; `Service-Worker-Allowed: /mobile/`.
- **21 fichiers PWA publics** répondant en 200 avec le MIME attendu et une empreinte
  SHA-256 identique au fichier local, dont tous les chunks JavaScript, le CSS, les images,
  le manifeste et le service worker.
- Une vraie `fetch` navigateur cross-origin POST, avec enveloppe opaque et identifiant
  aléatoire non enregistré, obtient un **404 JSON lisible** (`not_found`, réponse de type
  `cors`), sans erreur CORS. Ce test ne crée aucun appairage utilisateur.
- Préflight 204 pour l'origine Pages exacte, méthode POST et seuls en-têtes
  `Authorization, Content-Type` ; origine étrangère refusée en 403 sans autorisation CORS.
  Aucun `Access-Control-Allow-Credentials` n'est accordé.
- CSP limitant les connexions à l'origine Pages et au Worker officiel ; `nosniff`,
  `Referrer-Policy: no-referrer` et `Cross-Origin-Opener-Policy: same-origin` présents.
- `/mobile/assets/missing-test.js` répond en **404**, sans faux succès HTML 200.
  `/.DS_Store`, `/mobile/.DS_Store` et `/mobile/assets/.DS_Store` répondent également en 404.
- `/` répond en 302 et `/mobile` en 301 vers `/mobile/`.

Les preuves temporaires sont `/private/tmp/dmx-pages-209-qa.json` et
`/private/tmp/dmx-pages-209-mobile.png`. Le TaskSpace de test 59 a été fermé.

| Vérification locale de cette passe | Résultat |
| --- | --- |
| Tests du bridge Rust | 29 réussis ; le test réseau est ignoré par défaut |
| Test du relais public lancé explicitement | 1 réussi |
| Clippy du bridge | Réussi, avertissements traités comme erreurs |
| Tests PWA | 76 réussis |
| Tests Worker du relais | 7 réussis |
| Tests .NET | 63 réussis |
| Builds SwiftUI/AppKit et vérification GTK | Réussis |
| Préparation Pages, typage Worker et dry-run | Réussis |
| CI du commit | Cinq contrôles réussis, dont builds Apple Silicon/Intel Catalina et démarrage WinUI |
| Publication bureau 2.0.9 | Réussie, sept jobs et 23 assets publics non vides |

Les logs de CI confirment également 137 tests Rust du workspace réussis (dont les
29 du bridge), 13 tests DmxKit réussis, 63 tests .NET réussis et le build iOS arm64
simulateur. Le smoke réseau public, ignoré par défaut, a été lancé séparément.

Les tests de migration vérifient notamment la révocation lors du changement vers Pages,
la préservation du nouveau QR à la même origine et l'absence de modification des anciens
paramètres/appairages si l'activation Internet échoue. Le test unitaire de mutations
chiffrées vérifie également les snapshots et notifications hors réseau.

## Utilisation et limites

Pour migrer, synchroniser d'abord les saisies en attente dans l'ancienne PWA, installer
la version bureau 2.0.9, choisir l'action de migration affichée
dans le compagnon, puis générer et scanner un nouveau QR. Les files hors ligne de
l'ancienne origine ne sont pas transférées automatiquement vers Pages ; conserver
leur stockage tant qu'elles ne sont pas synchronisées.

- Le bureau doit rester **allumé, connecté à Internet et DmxMoney ouvert**. Le relais
  ne permet pas de modifier sa base tant que ce bureau est absent.
- Le transport n'impose pas le même réseau Wi-Fi : une connexion Internet mobile
  suffit en principe. Le parcours sur un téléphone physique en 4G/5G, Face ID ou
  Touch ID **n'a pas été testé** pendant cette passe.
- Les quatre callbacks du test prouvent la notification du noyau ; ils ne constituent
  pas une mesure de latence visuelle sur toutes les machines natives.
- La PWA active utilise un polling de 2,5 secondes pour les changements du bureau.
  iOS peut la suspendre en arrière-plan ; la reprise exige sa réouverture et un bureau
  disponible. Les écritures hors ligne restent en attente jusqu'à la reconnexion.
- Le cache mobile reste obfusqué en base64, facilement décodable. SQLite bureau
  et sauvegardes ne disposent pas d'un chiffrement applicatif au repos.
- Les URL et métadonnées réseau restent observables. Un opérateur modifiant le
  JavaScript livré pourrait compromettre le client ; le chiffrement ne protège pas
  un client compromis. Voir les [limites du relais](../../cloudflare/remote-relay/README.md).
- Aucun test d'installation avec Smart App Control, aucun Face ID matériel ni aucune
  nouvelle garantie de signature de confiance publique n'est ajouté par cette passe.

La [documentation du relais](../../cloudflare/remote-relay/README.md) et celle de
[la synchronisation mobile](../mobile-sync.md) décrivent le protocole, ses bornes et
les garanties de reprise des mutations.
