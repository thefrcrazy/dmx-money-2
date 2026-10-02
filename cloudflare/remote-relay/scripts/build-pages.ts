import { cp, mkdir, readdir, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { basename } from 'node:path';

const source = fileURLToPath(new URL('../../../pwa/dist/', import.meta.url));
const destination = fileURLToPath(new URL('../../companion-pages/.pages/', import.meta.url));
const mobile = `${destination}/mobile`;
await rm(destination, { recursive: true, force: true });
await mkdir(mobile, { recursive: true });
for (const entry of await readdir(source)) {
  if (entry === '_redirects' || entry === '_headers' || entry === '.DS_Store') continue;
  await cp(`${source}/${entry}`, `${mobile}/${entry}`, { recursive: true, filter: path => basename(path) !== '.DS_Store' });
}
// Keep the companion under /mobile/ so its relative assets and SW scope agree.
// No catch-all rewrite: it would return HTML for missing JavaScript assets.
await writeFile(`${destination}/_redirects`, '/ /mobile/ 302\n/mobile /mobile/ 301\n');
// A root 404 disables Pages' implicit SPA fallback for missing JS/assets.
await writeFile(`${destination}/404.html`, '<!doctype html><html lang="fr"><meta charset="utf-8"><title>Page introuvable</title><body><p>Page introuvable.</p><a href="/mobile/">Ouvrir DmxMoney</a></body></html>');
await writeFile(`${destination}/_headers`, `/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: no-referrer
  Permissions-Policy: camera=(self), microphone=(self), geolocation=()
  Cross-Origin-Opener-Policy: same-origin
  Content-Security-Policy: default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self' https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev; worker-src 'self'; manifest-src 'self'; form-action 'none'

/mobile/sw.js
  Cache-Control: no-store
  Service-Worker-Allowed: /mobile/

/mobile/
  Cache-Control: no-store

/mobile/index.html
  Cache-Control: no-store

/mobile/manifest.webmanifest
  Cache-Control: no-cache
`);
console.log(`Pages companion prepared at ${mobile}`);
