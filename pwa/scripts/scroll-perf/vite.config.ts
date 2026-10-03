import { defineConfig } from 'vite';
import base from '../../vite.config.ts';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
export default defineConfig({ ...base, root: new URL('.', import.meta.url).pathname,
 resolve: { alias: [{find: /.*\/context\/BankContext$/, replacement: new URL('./bank.ts', import.meta.url).pathname}] },
 build: { ...base.build, outDir: process.env.SCROLL_PERF_OUT_DIR ?? join(tmpdir(), 'dmx-scroll-perf-dist'), emptyOutDir: true },
});
