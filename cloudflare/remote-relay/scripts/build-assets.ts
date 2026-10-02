import { cp, mkdir, readdir, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { basename } from 'node:path';

const source = fileURLToPath(new URL('../../../pwa/dist/', import.meta.url));
const destination = fileURLToPath(new URL('../.assets/', import.meta.url));
// Routing/security headers are controlled by the relay Worker. Pages redirect files
// are inappropriate for Workers Assets and can create redirect loops.
await rm(destination, { recursive: true, force: true });
await mkdir(destination, { recursive: true });
for (const entry of await readdir(source)) {
  if (entry === '_redirects' || entry === '_headers' || entry === '.DS_Store') continue;
  await cp(`${source}/${entry}`, `${destination}/${entry}`, { recursive: true, filter: path => basename(path) !== '.DS_Store' });
}
