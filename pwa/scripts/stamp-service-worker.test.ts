import { expect, test } from 'bun:test';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { stampServiceWorker } from './stamp-service-worker';

test('cache stamp is repeatable and changes when a bundle changes', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'dmx-sw-'));
    try {
        await mkdir(join(dir, 'assets'));
        await writeFile(join(dir, 'sw.js'), 'const CACHE_NAME = "old";\n');
        await writeFile(join(dir, 'assets/app.js'), 'first');
        await stampServiceWorker(dir, '2.0.4');
        const first = await readFile(join(dir, 'sw.js'), 'utf8');
        await stampServiceWorker(dir, '2.0.4');
        expect(await readFile(join(dir, 'sw.js'), 'utf8')).toBe(first);
        await writeFile(join(dir, 'assets/app.js'), 'second');
        await stampServiceWorker(dir, '2.0.4');
        expect(await readFile(join(dir, 'sw.js'), 'utf8')).not.toBe(first);
    } finally { await rm(dir, { recursive: true, force: true }); }
});
