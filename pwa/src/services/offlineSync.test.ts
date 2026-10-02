import { afterEach, beforeEach, expect, mock, spyOn, test } from 'bun:test';
import { DatabaseService } from './db';
import { offlineStore } from './offlineStore';
import { indexedDB } from 'fake-indexeddb';
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
let queue: { id: string; path: string; method: string; body?: string }[];
beforeEach(async () => {
    const storage = new Map([['dmxmoney.secureApiBaseUrl', endpoint], ['dmxmoney.secureCsrfToken', 'test']]);
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { pathname: '/mobile' }, setTimeout, clearTimeout } });
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
    spyOn(offlineStore, 'getData').mockImplementation(async key => data[key] as never ?? null);
    spyOn(offlineStore, 'getRevision').mockResolvedValue('revision');
    spyOn(offlineStore, 'acceptRemoteData').mockImplementation(async (key, value) => {
        if (queue.length) return data[key] as never;
        data[key] = value;
        return value;
    });
    spyOn(offlineStore, 'setData').mockImplementation(async (key, value) => { data[key] = value; });
    spyOn(offlineStore, 'listMutations').mockImplementation(async () => queue as never);
    spyOn(offlineStore, 'removeMutation').mockImplementation(async id => { queue = queue.filter(item => item.id !== id); });
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
