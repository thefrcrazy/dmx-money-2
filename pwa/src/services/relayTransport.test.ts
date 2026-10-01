import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { indexedDB } from 'fake-indexeddb';
import { bytesToBase64Url, clearMobileRelay, configureMobileRelay, isMobileRelayUnlocked, markMobileRelayAuthenticated, mobileTransportFetch } from './relayTransport';
import { applyMobileCompanionPairingUrl, getMobileApiBaseUrl, getMobilePreviousApiBaseUrls, setMobileApiBaseUrl } from '../utils/runtime';
import { DatabaseService } from './db';
import { offlineStore } from './offlineStore';

const previous = { indexedDB: globalThis.indexedDB, window: globalThis.window, localStorage: globalThis.localStorage, fetch: globalThis.fetch };
const endpoint = 'https://sync.example.test/relay/device-id-test';
const rawKey = Uint8Array.from({ length: 32 }, (_, index) => index);
const encodedKey = bytesToBase64Url(rawKey);
const encode = new TextEncoder();
const decode = new TextDecoder();
const fromBase64 = (value: string) => Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), char => char.charCodeAt(0));

beforeEach(() => {
    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { pathname: '/mobile', origin: 'https://sync.example.test' }, setTimeout, clearTimeout } });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
    } });
});

afterEach(async () => {
    await clearMobileRelay();
    globalThis.fetch = previous.fetch;
    for (const key of ['indexedDB', 'window', 'localStorage'] as const) Object.defineProperty(globalThis, key, { configurable: true, value: previous[key] });
});

const peerKey = async (direction: string) => {
    const material = await crypto.subtle.importKey('raw', rawKey, 'HKDF', false, ['deriveKey']);
    return crypto.subtle.deriveKey({ name: 'HKDF', hash: 'SHA-256', salt: encode.encode('dmx-relay-v1'), info: encode.encode(direction) }, material, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
};

const respond = async (id: string, payload: unknown, tamper = false) => {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const encrypted = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, await peerKey('response'), encode.encode(JSON.stringify(payload))));
    if (tamper) encrypted[0] ^= 1;
    return Response.json({ id, nonce: bytesToBase64Url(nonce), ciphertext: bytesToBase64Url(encrypted) });
};

test('financial API traffic and session cookies remain encrypted end to end', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    expect(isMobileRelayUnlocked()).toBe(false);
    const received: Record<string, unknown>[] = [];
    globalThis.fetch = mock(async (url: string, init: RequestInit) => {
        expect(url).toBe(`${endpoint}/request`);
        expect(init.credentials).toBe('omit');
        const envelope = JSON.parse(init.body as string);
        expect(Object.keys(envelope).sort()).toEqual(['ciphertext', 'id', 'nonce']);
        expect(init.body).not.toContain('Compte privé');
        expect(init.body).not.toContain(encodedKey);
        const expectedAuthorization = bytesToBase64Url(new Uint8Array(await crypto.subtle.digest('SHA-256', encode.encode(encodedKey))));
        expect(new Headers(init.headers).get('Authorization')).toBe(`Bearer ${expectedAuthorization}`);
        const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: fromBase64(envelope.nonce) }, await peerKey('request'), fromBase64(envelope.ciphertext));
        const payload = JSON.parse(decode.decode(plaintext));
        expect(payload.id).toBe(envelope.id);
        expect(Math.abs(Date.now() - payload.issuedAt)).toBeLessThan(1000);
        received.push(payload);
        return respond(envelope.id, { id: envelope.id, status: 200, headers: { 'set-cookie': 'dmxmoney_session=secret; Secure; HttpOnly', 'content-type': 'application/json' }, body: '{"ok":true}' });
    }) as never;
    const result = await mobileTransportFetch('/auth/pairing/start', endpoint, { method: 'POST', body: '{"label":"Compte privé"}' });
    expect(await result.json()).toEqual({ ok: true });
    expect(result.headers.get('set-cookie')).toBeNull();
    expect(isMobileRelayUnlocked()).toBe(false);
    await mobileTransportFetch('/api/accounts', endpoint, { headers: { 'X-Dmx-Csrf': 'csrf' } });
    expect(received[0].path).toBe('/auth/pairing/start');
    expect(received[1].headers).toEqual({ cookie: 'dmxmoney_session=secret', 'x-dmx-csrf': 'csrf' });
    markMobileRelayAuthenticated();
    expect(isMobileRelayUnlocked()).toBe(true);
});

test('altered ciphertext and substituted response IDs are rejected', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    for (const tamper of [true, false]) {
        globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
            const { id } = JSON.parse(init.body as string);
            return respond(id, { id: tamper ? id : 'different-request', status: 200, headers: {}, body: '[]' }, tamper);
        }) as never;
        await expect(mobileTransportFetch('/api/accounts', endpoint, {})).rejects.toThrow();
    }
});

test('relay secrets are stored as nonextractable directional keys', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    const db = await new Promise<IDBDatabase>(resolve => { const request = indexedDB.open('dmxmoney-relay-keys', 1); request.onsuccess = () => resolve(request.result); });
    const keys = await new Promise<{ requestKey: CryptoKey; responseKey: CryptoKey }>(resolve => { const request = db.transaction('keys').objectStore('keys').get(endpoint); request.onsuccess = () => resolve(request.result); });
    expect(keys.requestKey.extractable).toBe(false);
    expect(keys.responseKey.extractable).toBe(false);
    await expect(crypto.subtle.exportKey('raw', keys.requestKey)).rejects.toThrow();
    db.close();
    expect(() => configureMobileRelay('http://sync.example.test/relay/device-id-test', encodedKey)).toThrow();
    expect(() => configureMobileRelay(endpoint, 'short-key')).toThrow();
});

test('PWA encryption matches the Rust relay interoperability fixture', async () => {
    await configureMobileRelay(endpoint, bytesToBase64Url(new Uint8Array(32).fill(7)));
    const db = await new Promise<IDBDatabase>(resolve => { const request = indexedDB.open('dmxmoney-relay-keys', 1); request.onsuccess = () => resolve(request.result); });
    const keys = await new Promise<{ requestKey: CryptoKey }>(resolve => { const request = db.transaction('keys').objectStore('keys').get(endpoint); request.onsuccess = () => resolve(request.result); });
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: new Uint8Array(12).fill(3) }, keys.requestKey, encode.encode('financial data'));
    expect(bytesToBase64Url(new Uint8Array(ciphertext))).toBe('0XCKWd-tiNRKR9oBcNZiQKc63zee7z56JyadPQ9E');
    db.close();
});

test('QR setup refuses insecure endpoints and keeps other desktop queues isolated', async () => {
    expect(applyMobileCompanionPairingUrl('#api=http://desktop.test:8443&pairing=token').ok).toBe(false);
    expect(getMobileApiBaseUrl()).toBeNull();
    setMobileApiBaseUrl('https://desktop.test:8443');
    setMobileApiBaseUrl('https://desktop.test:8444');
    expect(getMobilePreviousApiBaseUrls()).toEqual(['https://desktop.test:8443']);
    setMobileApiBaseUrl('https://other-desktop.test:8443');
    expect(getMobilePreviousApiBaseUrls()).toEqual([]);
    expect(applyMobileCompanionPairingUrl(`#relay=${encodeURIComponent(endpoint)}&key=${encodedKey}&pairing=token`).ok).toBe(true);
    expect(getMobileApiBaseUrl()).toBe(endpoint);
    expect(localStorage.getItem('dmxmoney.remoteRelayEndpoint')).toBe(endpoint);
    expect(localStorage.getItem('key')).toBeNull();
});

test('relay cached financial data stays hidden until a passkey session is finalized', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    await Promise.all((['accounts', 'transactions', 'categories', 'scheduled', 'budgets'] as const).map(key => offlineStore.setData(key, [])));
    globalThis.fetch = mock(() => { throw new Error('Network must not be used'); }) as never;
    const db = new DatabaseService();
    expect(await db.getCachedBankData()).toBeNull();
    markMobileRelayAuthenticated();
    expect(await db.getCachedBankData()).not.toBeNull();
    expect(globalThis.fetch).not.toHaveBeenCalled();
    await offlineStore.clearAll();
});

test('paginated journals restart on version conflict and cache only a complete snapshot', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    markMobileRelayAuthenticated();
    const paths: string[] = [];
    let pageIndex = 0;
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const envelope = JSON.parse(init.body as string);
        const clear = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: fromBase64(envelope.nonce) }, await peerKey('request'), fromBase64(envelope.ciphertext));
        const request = JSON.parse(decode.decode(clear));
        paths.push(request.path);
        const pages = [
            { status: 200, body: { transactions: [{ id: 'old' }], dataVersion: 1, nextOffset: 1 } },
            { status: 409, body: { error: 'version_changed' } },
            { status: 200, body: { transactions: [{ id: 'updated' }], dataVersion: 2, nextOffset: 1 } },
            { status: 200, body: { transactions: [{ id: 'second' }], dataVersion: 2, nextOffset: null } },
        ];
        const page = pages[pageIndex++];
        return respond(envelope.id, { id: envelope.id, status: page.status, headers: {}, body: JSON.stringify(page.body) });
    }) as never;
    expect(await new DatabaseService().getTransactions()).toEqual([{ id: 'updated' }, { id: 'second' }] as never);
    expect(await offlineStore.getData('transactions')).toEqual([{ id: 'updated' }, { id: 'second' }] as never);
    expect(paths).toEqual([
        '/api/transactions/page?offset=0&limit=2000', '/api/transactions/page?offset=1&limit=2000&version=1',
        '/api/transactions/page?offset=0&limit=2000', '/api/transactions/page?offset=1&limit=2000&version=2',
    ]);
    await offlineStore.clearAll();
});

test('invalid pagination cannot loop forever or overwrite the existing cache', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    markMobileRelayAuthenticated();
    await offlineStore.setData('transactions', [{ id: 'saved' }] as never);
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const { id } = JSON.parse(init.body as string);
        return respond(id, { id, status: 200, headers: {}, body: JSON.stringify({ transactions: [{ id: 'incomplete' }], dataVersion: 2, nextOffset: 0 }) });
    }) as never;
    await expect(new DatabaseService().getTransactions()).rejects.toThrow('Pagination');
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    expect(await offlineStore.getData('transactions')).toEqual([{ id: 'saved' }] as never);
    await offlineStore.clearAll();
});
