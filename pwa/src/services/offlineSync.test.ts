import { afterEach, beforeEach, expect, mock, spyOn, test } from 'bun:test';
import { DatabaseService } from './db';
import { offlineStore } from './offlineStore';
import { indexedDB, IDBObjectStore } from 'fake-indexeddb';
import { encodeOfflinePayload } from './offlinePayload';
import type { Settings } from '../types';
import { bytesToBase64Url, clearMobileRelay, configureMobileRelay, markMobileRelayAuthenticated } from './relayTransport';
import { setMobileCsrfToken } from '../utils/runtime';

const originalWindow = globalThis.window;
const originalStorage = globalThis.localStorage;
const originalFetch = globalThis.fetch;
const originalIndexedDB = globalThis.indexedDB;
const endpoint = 'https://relay.example.test/relay/offline-test-device';
const secret = new Uint8Array(32).fill(11);
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let requestHandler: (path: string, init: RequestInit) => Promise<Response>;
const fromBase64 = (value: string) => Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), char => char.charCodeAt(0));
let data: Record<string, unknown>;
let queue: { id: string; path: string; method: string; body?: string; failure?: { status: number; message: string; conflicts?: string[] } }[];
let storeMocks: { mockRestore: () => void }[];
const fixtureSettings: Settings = {
    theme: 'light', primaryColor: 'default', componentSpacing: 6, componentPadding: 6,
    windowPosition: null, windowSize: null, settingsRevision: 1,
};
beforeEach(async () => {
    const storage = new Map([['dmxmoney.secureApiBaseUrl', endpoint], ['dmxmoney.secureCsrfToken', 'test']]);
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { pathname: '/mobile' }, setTimeout, clearTimeout, dispatchEvent: () => true } });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
    } });
    await configureMobileRelay(endpoint, bytesToBase64Url(secret));
    await markMobileRelayAuthenticated(new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString());
    setMobileCsrfToken('test');
    const material = await crypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveKey']);
    const derive = (direction: 'request' | 'response') => crypto.subtle.deriveKey({
        name: 'HKDF', hash: 'SHA-256', salt: encoder.encode('dmx-relay-v1'), info: encoder.encode(direction),
    }, material, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
    const [requestKey, responseKey] = await Promise.all([derive('request'), derive('response')]);
    requestHandler = async () => { throw new Error('Unexpected request'); };
    globalThis.fetch = mock(async (input: string, init: RequestInit) => {
        expect(input).toBe(`${endpoint}/request`);
        expect(init.credentials).toBe('omit');
        const envelope = JSON.parse(init.body as string);
        const clear = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: fromBase64(envelope.nonce) }, requestKey, fromBase64(envelope.ciphertext));
        const request = JSON.parse(decoder.decode(clear));
        const response = await requestHandler(request.path, request);
        const nonce = crypto.getRandomValues(new Uint8Array(12));
        const body = encoder.encode(JSON.stringify({
            id: envelope.id, status: response.status, headers: Object.fromEntries(response.headers.entries()), body: await response.text(),
        }));
        const encrypted = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, responseKey, body);
        return Response.json({ id: envelope.id, nonce: bytesToBase64Url(nonce), ciphertext: bytesToBase64Url(new Uint8Array(encrypted)) });
    }) as never;
    data = { accounts: [], transactions: [], categories: [], scheduled: [], budgets: [] };
    queue = [];
    storeMocks = [
        spyOn(offlineStore, 'getData').mockImplementation(async key => data[key] as never ?? null),
        spyOn(offlineStore, 'getRevision').mockResolvedValue('revision'),
        spyOn(offlineStore, 'acceptRemoteData').mockImplementation(async (key, value) => {
            if (queue.length) return data[key] as never;
            data[key] = value;
            return value;
        }),
        spyOn(offlineStore, 'setData').mockImplementation(async (key, value) => { data[key] = value; }),
        spyOn(offlineStore, 'listMutations').mockImplementation(async () => queue as never),
        spyOn(offlineStore, 'getSettingsSnapshot').mockImplementation(async () => ({ settings: data.settings as Settings ?? null, mutations: queue as never })),
        spyOn(offlineStore, 'listMutationIds').mockImplementation(async () => queue.map(item => item.id)),
        spyOn(offlineStore, 'setMutationFailure').mockImplementation(async (id, failure) => {
            const item = queue.find(item => item.id === id);
            if (item) item.failure = failure;
        }),
        spyOn(offlineStore, 'removeMutation').mockImplementation(async id => { queue = queue.filter(item => item.id !== id); }),
    ];
});
afterEach(async () => {
    await clearMobileRelay();
    mock.restore();
    globalThis.fetch = originalFetch;
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: originalIndexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: originalWindow });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: originalStorage });
});
test('cached startup accepts empty collections and never contacts the desktop', async () => {
    requestHandler = mock(async () => { throw new Error('Network must not be used'); });
    const db = new DatabaseService();
    expect(await db.getCachedBankData()).not.toBeNull();
    expect(await db.getAccounts()).toEqual([]);
    expect(globalThis.fetch).not.toHaveBeenCalled();
});
test('incomplete cache cannot masquerade as an empty bank', async () => {
    delete data.transactions;
    expect(await new DatabaseService().getCachedBankData()).toBeNull();
});
test('HTTP 503 preserves cached data and unacknowledged edits', async () => {
    queue = [{ id: 'queued', path: '/api/accounts/a', method: 'DELETE' }];
    requestHandler = mock(async () => new Response('unavailable', { status: 503 }));
    const db = new DatabaseService();
    expect(await db.getAccounts()).toEqual([]);
    expect(queue).toHaveLength(1);
    try { await db.getSyncStatus(); throw new Error('Expected offline error'); }
    catch (error) { expect(db.isOfflineError(error)).toBe(true); }
});
test('reconnection acknowledges queued edits before loading fresh data', async () => {
    queue = [{ id: 'queued', path: '/api/accounts/a', method: 'DELETE' }];
    const calls: string[] = [];
    requestHandler = mock(async (path: string, init: RequestInit) => {
        calls.push(`${init.method ?? 'GET'} ${path}`);
        return Response.json(path === '/api/status' ? { ok: true, dataVersion: 2 } : []);
    });
    const db = new DatabaseService();
    await db.getCachedBankData();
    await db.getSyncStatus();
    await db.getAccounts();
    expect(queue).toHaveLength(0);
    expect(calls).toEqual(['GET /api/status', 'DELETE /api/accounts/a', 'GET /api/status', 'GET /api/accounts']);
});
test('a concurrent local edit is not overwritten by an older GET response', async () => {
    requestHandler = mock(async () => {
        data.accounts = [{ id: 'local' }];
        queue = [{ id: 'queued', path: '/api/accounts', method: 'POST' }];
        return Response.json([{ id: 'server' }]);
    });
    expect(await new DatabaseService().getAccounts()).toEqual([{ id: 'local' }] as never);
    expect(data.accounts).toEqual([{ id: 'local' }]);
});

test('legacy queued settings JSON fields are decoded and transmitted exactly once', async () => {
    queue = [{ id: 'groups', path: '/api/settings', method: 'PATCH', body: JSON.stringify({
        schemaVersion: 2, baseRevision: 1, values: { accountGroups: JSON.stringify({ a: 'group' }) },
        expectedValues: { accountGroups: JSON.stringify({}) },
    }) }];
    let transmitted: { values: { accountGroups: string }; expectedValues: { accountGroups: string } } | undefined;
    requestHandler = mock(async (path: string, init: RequestInit) => {
        if (path === '/api/settings') transmitted = JSON.parse(init.body as string);
        return Response.json({ ok: true, dataVersion: 3, revision: 2, conflicts: [] });
    });
    Object.assign(window, { dispatchEvent: () => true });
    await new DatabaseService().getSyncStatus();
    expect(transmitted?.values.accountGroups).toBe('{"a":"group"}');
    expect(transmitted?.expectedValues.accountGroups).toBe('{}');
    expect(queue).toHaveLength(0);
});

test('unreadable settings mutations remain in the offline queue', async () => {
    queue = [{ id: 'broken', path: '/api/settings', method: 'PATCH', body: '{invalid' }];
    requestHandler = mock(async () => Response.json({ ok: true, dataVersion: 1 }));
    await expect(new DatabaseService().getSyncStatus()).rejects.toThrow('illisible');
    expect(queue).toHaveLength(1);
});

test('a permanent rejection stays visible, blocks its dependent edit and lets independent work through', async () => {
    queue = [
        { id: 'bad', path: '/api/accounts', method: 'POST', body: '{"id":"a","name":"bad"}' },
        { id: 'dependent', path: '/api/transactions', method: 'POST', body: '{"id":"t","accountId":"a"}' },
        { id: 'independent', path: '/api/accounts', method: 'POST', body: '{"id":"b","name":"good"}' },
    ];
    const sent: string[] = [];
    requestHandler = mock(async (path, init) => {
        if (init.method === 'POST') {
            const body = JSON.parse(init.body as string);
            sent.push(body.id);
            if (body.id === 'a') return Response.json({ error: 'Invalid account' }, { status: 400 });
        }
        return Response.json({ ok: true, dataVersion: 3 });
    });
    await new DatabaseService().getSyncStatus();
    expect(sent).toEqual(['a', 'b']);
    expect(queue.map(item => item.id)).toEqual(['bad', 'dependent']);
    expect(queue[0].failure?.status).toBe(400);
});

test('settings conflicts are retained and reported instead of acknowledged', async () => {
    queue = [{ id: 'settings-conflict', path: '/api/settings', method: 'PATCH', body: JSON.stringify({
        schemaVersion: 2, baseRevision: 1, values: { theme: 'dark' }, expectedValues: { theme: 'light' },
    }) }];
    requestHandler = mock(async path => Response.json(path === '/api/settings'
        ? { ok: true, revision: 3, conflicts: ['theme'] } : { ok: true, dataVersion: 3 }));
    await new DatabaseService().getSyncStatus();
    expect(queue).toHaveLength(1);
    expect(queue[0].failure?.conflicts).toEqual(['theme']);
});

// These regressions exercise the real cache and outbox; only the encrypted relay's remote responses are fixtures.
async function useActualStore() {
    storeMocks.forEach(item => item.mockRestore());
    await offlineStore.clearAll();
    for (const key of ['accounts', 'transactions', 'categories', 'budgets', 'scheduled'] as const) await offlineStore.setData(key, []);
    await offlineStore.setData('settings', fixtureSettings);
}
function deferred() {
    let resolve!: () => void;
    const promise = new Promise<void>(done => { resolve = done; });
    return { promise, resolve };
}
async function corruptActualQueue() {
    // The legacy unreadable body is retained inside the current opaque format, exactly as after an upgrade.
    const request = indexedDB.open('dmxmoney-mobile-offline');
    const db = await new Promise<IDBDatabase>((resolve, reject) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });
    try {
        const transaction = db.transaction('mutations', 'readwrite');
        const done = new Promise<void>((resolve, reject) => {
            transaction.oncomplete = () => resolve();
            transaction.onabort = transaction.onerror = () => reject(transaction.error);
        });
        transaction.objectStore('mutations').put({ id: 'broken', scope: endpoint, path: '/api/accounts', method: 'POST', createdAt: 1,
            payload: encodeOfflinePayload({ body: '{invalid' }) });
        await done;
    } finally { db.close(); }
}
function successfulSnapshot(path: string) {
    if (path === '/api/status') return Response.json({ ok: true, dataVersion: 10 });
    if (path.startsWith('/api/transactions/page?')) return Response.json({ transactions: [], dataVersion: 10, nextOffset: null });
    if (path === '/api/settings') return Response.json(fixtureSettings);
    return Response.json([]);
}

test('a settings read suspended before its final snapshot returns the concurrent local choice without rewriting cache', async () => {
    await useActualStore();
    requestHandler = async path => path === '/api/settings' ? Response.json(fixtureSettings) : Response.json({ ok: true, dataVersion: 1 });
    const reached = deferred();
    const gate = deferred();
    const snapshot = offlineStore.getSettingsSnapshot.bind(offlineStore);
    spyOn(offlineStore, 'getSettingsSnapshot').mockImplementation(async () => {
        reached.resolve(); await gate.promise; return snapshot();
    });
    const reading = new DatabaseService().getSettings();
    await reached.promise;
    await offlineStore.commitBankMutation('/api/settings', 'PATCH', JSON.stringify({
        schemaVersion: 2, baseRevision: 1, values: { theme: 'dark' }, expectedValues: { theme: 'light' },
    }), { ...fixtureSettings, theme: 'dark' });
    gate.resolve();
    expect((await reading)?.theme).toBe('dark');
    expect((await offlineStore.getData('settings'))?.theme).toBe('dark');
    const pending = await offlineStore.listMutations();
    expect(pending).toHaveLength(1);
    expect(JSON.parse(pending[0].body!).values.theme).toBe('dark');
});

test('explicit discard recovers an unreadable body after a complete stable server snapshot', async () => {
    await useActualStore();
    await offlineStore.setData('accounts', [{ id: 'local', name: 'Fixture', type: 'checking', initialBalance: 0, color: '', icon: '' }]);
    await corruptActualQueue();
    await expect(offlineStore.listMutations()).rejects.toThrow('illisibles');
    const paths: string[] = [];
    requestHandler = async path => { paths.push(path); return successfulSnapshot(path); };
    await new DatabaseService().reloadServerAndDiscardPending();
    expect(paths.filter(path => path === '/api/status')).toHaveLength(2);
    expect(paths.some(path => path.startsWith('/api/transactions/page?'))).toBe(true);
    expect(await offlineStore.listMutations()).toEqual([]);
    expect(await offlineStore.getData('accounts')).toEqual([]);
    expect((await offlineStore.getData('settings'))?.theme).toBe('light');
});

test('explicit discard preserves an unreadable queue when any server collection fails', async () => {
    await useActualStore(); await corruptActualQueue();
    requestHandler = async path => path === '/api/categories' ? new Response('Fixture unavailable', { status: 503 }) : successfulSnapshot(path);
    await expect(new DatabaseService().reloadServerAndDiscardPending()).rejects.toThrow();
    expect(await offlineStore.listMutationIds()).toEqual(['broken']);
    expect((await offlineStore.getData('settings'))?.theme).toBe('light');
});

test('explicit discard refuses new local work added while reading the server snapshot', async () => {
    await useActualStore();
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"first"}');
    const reached = deferred(); const gate = deferred();
    requestHandler = async path => {
        if (path === '/api/accounts') { reached.resolve(); await gate.promise; }
        return successfulSnapshot(path);
    };
    const restoring = new DatabaseService().reloadServerAndDiscardPending();
    await reached.promise;
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"second"}');
    gate.resolve();
    await expect(restoring).rejects.toThrow('changé');
    expect((await offlineStore.getData('accounts'))?.map(item => item.id)).toEqual(['first', 'second']);
    expect(await offlineStore.listMutationIds()).toHaveLength(2);
});

test('explicit discard quota failure preserves all cached rows and the unreadable queue', async () => {
    await useActualStore(); await corruptActualQueue();
    await offlineStore.setData('accounts', [{ id: 'local', name: 'Fixture', type: 'checking', initialBalance: 0, color: '', icon: '' }]);
    requestHandler = async path => successfulSnapshot(path);
    const put = IDBObjectStore.prototype.put;
    spyOn(IDBObjectStore.prototype, 'put').mockImplementation(function(value: unknown, key?: IDBValidKey) {
        if (this.name === 'data' && (value as { dataKey?: string }).dataKey === 'settings') throw new DOMException('Fixture quota', 'QuotaExceededError');
        return put.call(this, value, key);
    });
    await expect(new DatabaseService().reloadServerAndDiscardPending()).rejects.toThrow('Fixture quota');
    expect((await offlineStore.getData('accounts'))?.map(item => item.id)).toEqual(['local']);
    expect(await offlineStore.listMutationIds()).toEqual(['broken']);
});
