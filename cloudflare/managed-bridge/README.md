# DmxMoney Managed Bridge

Service Cloudflare Worker qui rend le pont HTTPS automatique pour les utilisateurs DmxMoney et sert la PWA statique.

L’application desktop appelle uniquement `https://dmxmoney.develop-max.com`. Le Worker garde le token Cloudflare côté serveur, crée un appareil, retourne un sous-domaine local dédié, met à jour le DNS vers l’IP privée du Mac et pose/supprime les TXT ACME nécessaires à Let’s Encrypt. Il sert aussi les fichiers de `dist/` depuis KV sous `/mobile`. Aucune donnée financière ne transite par ce service.

## Secrets et bindings

- `DEVICES`: KV namespace contenant uniquement les appareils, secrets hashés et IDs DNS.
- `ASSETS`: KV namespace contenant les fichiers statiques de la PWA.
- `CLOUDFLARE_API_TOKEN`: token limité à la zone DNS.
- `CLOUDFLARE_ZONE_ID`: zone du domaine `BRIDGE_DOMAIN`.
- `REGISTRATION_SECRET`: secret serveur obligatoire pour créer un nouvel appareil depuis l’app desktop.
- `BRIDGE_DOMAIN`: domaine public du pont, par défaut `develop-max.com`.
- `PWA_URL`: URL HTTPS de la PWA statique, par défaut `https://dmxmoney.develop-max.com/mobile`.

Le token Cloudflare n’est jamais stocké dans SQLite ni renvoyé à l’application desktop. La route publique `/v1/devices/register` échoue fermée sans `REGISTRATION_SECRET`, ce qui évite qu’un clone open source ou la PWA statique publique provisionne des sous-domaines sur `develop-max.com`.

## Endpoints utilisés par DmxMoney

- `GET /mobile` et fichiers statiques PWA.
- `POST /v1/devices/register`
- `POST /v1/devices/:deviceId/dns`
- `POST /v1/devices/:deviceId/acme/txt`
- `POST /v1/devices/:deviceId/acme/txt/delete`
