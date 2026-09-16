import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const source = readFileSync(new URL('../public/sw.js', import.meta.url), 'utf8');
test('navigation falls back to the saved shell when the LAN server times out', async () => {
    const handlers: Record<string, (event: unknown) => void> = {};
    const cached = new Response('saved app');
    runInNewContext(source, {
        self: { addEventListener: (name: string, handler: (event: unknown) => void) => { handlers[name] = handler; } },
        caches: { open: async () => ({ match: async () => cached }) },
        fetch: (_: unknown, { signal }: RequestInit) => new Promise((_, reject) => signal?.addEventListener('abort', () => reject(new Error('timeout')))),
        AbortController, URL, Response,
        setTimeout: (fn: () => void) => setTimeout(fn, 1), clearTimeout,
    });
    let result: Promise<Response> | undefined;
    handlers.fetch({ request: { url: 'https://desktop.test/mobile/', method: 'GET', mode: 'navigate' }, respondWith: (value: Promise<Response>) => { result = value; } });
    expect(await (await result!).text()).toBe('saved app');
});
test('activation removes only obsolete application shell caches', async () => {
    const handlers: Record<string, (event: unknown) => void> = {};
    const removed: string[] = [];
    const current = source.match(/const CACHE_NAME = "([^"]+)"/)![1];
    runInNewContext(source, {
        self: { addEventListener: (name: string, handler: (event: unknown) => void) => { handlers[name] = handler; }, clients: { claim: async () => {} } },
        caches: { keys: async () => ['dmxmoney-shell-old', current, 'other-app'], delete: async (key: string) => { removed.push(key); } },
    });
    let done: Promise<void> | undefined;
    handlers.activate({ waitUntil: (value: Promise<void>) => { done = value; } });
    await done;
    expect(removed).toEqual(['dmxmoney-shell-old']);
});
