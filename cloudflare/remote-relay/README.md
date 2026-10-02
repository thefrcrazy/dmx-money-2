# Compagnon distant DmxMoney

Le service commun est publié sur
`https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/mobile/`.
Chaque installation de bureau génère sa propre identité aléatoire. Aucun domaine,
enregistrement DNS, port entrant ou compte Cloudflare n’est demandé à l’utilisateur.
L’ancien `managed-bridge` reste séparé pour les installations existantes.

## Fonctionnement

Le bureau ouvre une connexion WSS sortante. Un Durable Object par installation
achemine les demandes HTTPS du téléphone vers cette connexion et renvoie la réponse.
La base reste sur le bureau ; le relais ne conserve aucune donnée financière.
Il conserve les empreintes des identifiants de transport et les compteurs de débit.
Les données en transit sont chiffrées avec AES-256-GCM ; HKDF-SHA256 produit des clés
distinctes pour les requêtes et les réponses. La clé maîtresse ne passe pas au Worker.

Le QR transmet la clé par fragment d’URL, avec un jeton d’appairage valable dix minutes
et consommable une seule fois. Le fragment est retiré après lecture. Le bureau conserve
ses secrets dans le trousseau système ; la PWA conserve des CryptoKey non extractables
dans IndexedDB. Les cookies de session restent en mémoire et passent uniquement dans
le message chiffré. Les passkeys sont vérifiées par le noyau avec vérification utilisateur
obligatoire : Face ID, Touch ID ou code selon l’appareil. Le QR doit rester privé.

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
  JSON UTF-8. La PWA masque les données avant une authentification passkey et après
  45 minutes ; cette garde ne chiffre pas les fichiers. Après fermeture, la reconnexion au bureau est requise
  pour déverrouiller. Un onglet déjà déverrouillé peut continuer hors ligne.
- SQLite sur le bureau et les sauvegardes `.dmx` restent dans leur format existant,
  sans chiffrement applicatif au repos. La protection du système et du disque s’applique.
- La migration depuis le pont local change l’origine WebAuthn : les anciens mobiles
  doivent être réappairés. Les anciennes files hors ligne restent dans leur ancien scope ;
  les synchroniser avant la migration évite d’y laisser des saisies en attente.

## Développement et publication

```bash
cd pwa
bun install --frozen-lockfile
bun run build
cd ../cloudflare/remote-relay
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
Un éditeur peut définir `DMXMONEY_REMOTE_RELAY_URL` au **build Rust** pour choisir une
autre origine HTTPS commune. La PWA prend l’origine du QR ; aucun secret commun
d’enregistrement n’est embarqué dans l’application.

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
