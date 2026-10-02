# Compagnon distant DmxMoney

La PWA commune est hébergée sur Cloudflare Pages à
`https://dmxmoney-companion.pages.dev/mobile/`.
L'API et le relais WebSocket sont fournis par le Worker séparé
`https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev`.
Chaque installation de bureau génère sa propre identité aléatoire. Aucun domaine,
enregistrement DNS, port entrant ou compte Cloudflare n’est demandé à l’utilisateur.
Dans les paramètres du bureau, activer le compagnon puis scanner le QR sur le téléphone.
L'adresse Pages et le relais sont configurés automatiquement ; une passkey termine
l'appairage. Le même accès fonctionne sur le réseau du domicile et sur les réseaux mobiles.

## Fonctionnement

Le bureau ouvre une connexion WSS sortante. Un Durable Object par installation
achemine les demandes HTTPS du téléphone vers cette connexion et renvoie la réponse.
Les POST/PUT/PATCH/DELETE sont déchiffrés et autorisés sur le bureau avant mutation de
sa base SQLite ; chaque écriture acceptée notifie l'interface native immédiatement.
La base reste sur le bureau ; le relais ne conserve aucune donnée financière.
Il conserve les empreintes des identifiants de transport et les compteurs de débit.
Les données en transit sont chiffrées avec AES-256-GCM ; HKDF-SHA256 produit des clés
distinctes pour les requêtes et les réponses. La clé maîtresse ne passe pas au Worker.

Le QR transmet la clé par fragment d’URL, avec un jeton d’appairage valable dix minutes
et consommable une seule fois. Le fragment est retiré après lecture. Le bureau conserve
ses secrets dans le trousseau système ; la PWA conserve des CryptoKey non extractables
dans IndexedDB. La session finalisée y est conservée sous forme chiffrée AES-GCM.
Ses cookies passent uniquement dans les messages chiffrés, jamais dans les cookies
HTTP du relais. Au lancement et au retour au premier plan, la PWA reprend cette session
par `POST /auth/session` sans demander une nouvelle passkey tant qu'elle reste valide.
Une session expire après sept jours d'inactivité ou trente jours depuis sa création.
Révoquer l'appareil ou sa passkey sur le bureau invalide toutes ses sessions immédiatement.
Le bouton « Verrouiller » retire la session et demande une nouvelle authentification,
en conservant la clé du relais, le cache et les modifications hors ligne.

Les passkeys sont vérifiées par le noyau avec `userVerification=required`. Face ID,
Touch ID ou le code de l'appareil sont choisis par le système ; l'application ne peut
pas imposer Face ID à Safari. L'appairage inachevé expire après dix minutes et n'est
pas persisté comme une session finalisée. Le QR doit rester privé.

Le bureau doit rester allumé, connecté à Internet et DmxMoney ouvert. La PWA active
vérifie les changements toutes les 2,5 secondes ; les écritures déclenchent aussi le
rafraîchissement natif. iOS peut suspendre une PWA en arrière-plan : aucune garantie de
synchronisation pendant cette suspension. Les modifications hors ligne restent en file
sur le téléphone et sont rejouées à la reconnexion.

## Limites de confidentialité

- Le relais connaît l’adresse IP, les identifiants de routage, les horaires et les tailles.
  Les URLs existent et restent observables ; masquer un lien dans l’interface n’est pas
  une protection cryptographique.
- Un opérateur qui modifierait le JavaScript livré à la PWA pourrait lire ses données.
  Le chiffrement protège le transport et le relais honnête, pas un client compromis.
- Le cache financier IndexedDB et les corps des mutations en attente sont obfusqués
  en JSON UTF-8/base64, avec migration atomique du stockage existant. Ce format masque
  la lecture directe, mais reste facilement décodable sans clé : il ne protège pas la
  confidentialité des fichiers récupérés. Le base64 ajoute environ 33 % à la taille du
  JSON UTF-8. La PWA masque les données en l'absence de session authentifiée ou après
  verrouillage ; cette garde ne chiffre pas les fichiers financiers. Le chiffrement
  de la session ne transforme pas le cache financier en stockage chiffré.
  Un téléphone déverrouillé peut réutiliser une session encore valide : le verrouillage
  du système et l'action « Verrouiller » protègent cet accès entre deux usages.
- SQLite sur le bureau et les sauvegardes `.dmx` restent dans leur format existant,
  sans chiffrement applicatif au repos. La protection du système et du disque s’applique.

## Développement et publication

Pour construire et publier la PWA sur le projet Pages commun, depuis la racine :

```bash
./scripts/deploy-pwa.sh
```

Le script construit la PWA, prépare `/mobile/` et publie sur la branche `main` du projet
Pages `dmxmoney-companion`. La session Wrangler ou le jeton Cloudflare de l'éditeur
doit autoriser ce projet. Aucun accès DNS ni namespace KV n'est utilisé.

Pour vérifier et publier le Worker du relais séparément :

```bash
./scripts/build-pwa.sh
cd cloudflare/remote-relay
bun install --frozen-lockfile
bun run assets
bun run types
bun run check
bun run test
bun run dry-run
bun run deploy
```

`bun run assets` copie le build sans les fichiers de routage propres à Pages. Le Worker
sert les assets sous `/mobile/` et applique ses propres en-têtes de sécurité. La date
de compatibilité est celle prise en charge par le moteur de tests verrouillé.
`bun run pages:assets` prépare le dossier statique Pages sous `/mobile/`, avec CSP,
un 404 explicite et chemins de service worker cohérents. Le Worker autorise en CORS uniquement cette
origine additionnelle, sur la route de messages chiffrés ; aucun cookie HTTP n'est exposé.
Un éditeur peut définir `DMXMONEY_REMOTE_RELAY_URL` au **build Rust** pour choisir une
autre origine HTTPS commune. La PWA prend l’origine du QR ; aucun secret commun
d’enregistrement n’est embarqué dans l’application.
`DMXMONEY_COMPANION_URL` peut choisir l'adresse HTTPS de la PWA au build Rust. Pour
un hébergement personnalisé, adapter aussi `COMPANION_ORIGIN` et la CSP de Pages.

Les overrides `sharp 0.35.4` et `undici 7.29.1` actualisent les dépendances de Miniflare
du moteur de tests verrouillé. Ils corrigent les avis de sécurité connus sans changer
la version de ce moteur ; les tests Worker et `bun audit` couvrent ce choix.

Le test Rust ignoré vérifie le transport réel, l’appairage chiffré et le rejet des renvois :

```bash
# Dans un terminal, depuis ce dossier :
bun run dev -- --port 8789 --ip 127.0.0.1
# Dans un autre, depuis la racine du dépôt :
cargo test -p dmx-bridge local_cloudflare_worker_round_trip -- --ignored
```

Avec `DMXMONEY_RELAY_SMOKE_URL` pointant sur une origine de test HTTPS, le même test
vérifie HTTPS/WSS. Il génère une identité éphémère et la supprime après succès.

## Routes et limites

| Route | Protection | Rôle |
| --- | --- | --- |
| `GET /relay/capabilities` | Publique | Découverte du protocole |
| `POST /relay/:id/enroll` | Possession du secret bureau ; rate limit IP | Création atomique, impossible d’écraser une autre identité |
| `DELETE /relay/:id/enroll` | Secret bureau | Supprime uniquement les identifiants du relais |
| `GET /relay/:id/connect` | Secret bureau | Connexion WSS sortante du bureau |
| `POST /relay/:id/request` | Identifiant de transport mobile | Transmet une enveloppe chiffrée ; passkey/session toujours requise par le bureau |

Huit demandes simultanées par installation, délai de réponse de 25 secondes,
300 demandes par minute et enveloppes de 12 MiB maximum. Le noyau accepte des demandes
récentes de deux minutes maximum et garde les identifiants anti-renvoi cinq minutes en
SQLite. Les transactions sont lues par pages de 2 000 avec une version bancaire stable.
Les autres collections et les écritures restent bornées par la taille d’enveloppe.
Le service requiert un compte Cloudflare permettant les Durable Objects SQLite ;
surveiller consommation et quotas dans le tableau de bord de l’éditeur.

Documentation : [WebSockets et hibernation](https://developers.cloudflare.com/durable-objects/best-practices/websockets/),
[limitation de débit Workers](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/).
