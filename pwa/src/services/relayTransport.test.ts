import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { indexedDB } from 'fake-indexeddb';
import { bytesToBase64Url, clearMobileRelay, configureMobileRelay, isMobileRelayUnlocked, markMobileRelayAuthenticated, mobileTransportFetch, MOBILE_RELAY_LOCK_STORAGE_KEY, MOBILE_RELAY_SESSION_EPOCH_KEY, suspendMobileRelayRuntime } from './relayTransport';
import { applyMobileCompanionPairingUrl, getMobileApiBaseUrl, getMobileCsrfToken, getMobilePreviousApiBaseUrls, hasMobileCompanionSetup, hasMobilePasskeySetup, setMobileApiBaseUrl, setMobileCsrfToken } from '../utils/runtime';
import { DatabaseService } from './db';
import { offlineStore } from './offlineStore';

const previous = { indexedDB: globalThis.indexedDB, window: globalThis.window, navigator: globalThis.navigator, localStorage: globalThis.localStorage, fetch: globalThis.fetch };
const endpoint = 'https://sync.example.test/relay/device-id-test';
const rawKey = Uint8Array.from({ length: 32 }, (_, index) => index);
const encodedKey = bytesToBase64Url(rawKey);
const encode = new TextEncoder();
const decode = new TextDecoder();
const fromBase64 = (value: string) => Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), char => char.charCodeAt(0));

beforeEach(() => {
    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { pathname: '/mobile', origin: 'https://sync.example.test' }, isSecureContext: true, PublicKeyCredential: class {}, setTimeout, clearTimeout, dispatchEvent: () => true } });
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { credentials: { get: mock(async () => { throw new Error('WebAuthn must be user initiated'); }), create: mock(async () => { throw new Error('WebAuthn must be user initiated'); }) }, userAgent: 'Fixture', platform: 'Fixture' } });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, value),
        removeItem: (key: string) => storage.delete(key),
    } });
});

afterEach(async () => {
    await offlineStore.clearAll();
    await clearMobileRelay();
    globalThis.fetch = previous.fetch;
    for (const key of ['indexedDB', 'window', 'navigator', 'localStorage'] as const) Object.defineProperty(globalThis, key, { configurable: true, value: previous[key] });
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

interface StoredKeysFixture {
    requestKey: CryptoKey;
    responseKey: CryptoKey;
    authorization: string;
    session?: { nonce: string; ciphertext: string; expiresAt: number; epoch: string };
}

const readStoredKeys = async (): Promise<StoredKeysFixture> => {
    const db = await new Promise<IDBDatabase>(resolve => { const request = indexedDB.open('dmxmoney-relay-keys', 1); request.onsuccess = () => resolve(request.result); });
    try {
        return await new Promise<StoredKeysFixture>(resolve => { const request = db.transaction('keys').objectStore('keys').get(endpoint); request.onsuccess = () => resolve(request.result); });
    } finally { db.close(); }
};

interface PeerRequest { id: string; method: string; path: string; headers: Record<string, string> }
const decodePeerRequest = async (init: RequestInit): Promise<PeerRequest> => {
    const envelope = JSON.parse(init.body as string);
    const clear = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: fromBase64(envelope.nonce) }, await peerKey('request'), fromBase64(envelope.ciphertext));
    return JSON.parse(decode.decode(clear));
};

const sessionExpiresAt = () => new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();
const seedFinalizedSession = async (cookie = 'preserved-secret', csrf = 'previous-csrf') => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    const expiry = sessionExpiresAt();
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const request = await decodePeerRequest(init);
        return respond(request.id, { id: request.id, status: 200, headers: { 'set-cookie': `dmxmoney_session=${cookie}; Secure; HttpOnly` }, body: '{}' });
    }) as never;
    await mobileTransportFetch('/auth/passkey/login/verify', endpoint, { method: 'POST', body: '{}' });
    setMobileCsrfToken(csrf);
    await markMobileRelayAuthenticated(expiry, undefined, undefined, true);
};

const installResumingPeer = (paths: string[]) => {
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const request = await decodePeerRequest(init);
        paths.push(request.path);
        expect(request.headers.cookie).toBe('dmxmoney_session=preserved-secret');
        return respond(request.id, { id: request.id, status: 200, headers: {}, body: JSON.stringify(request.path === '/auth/session'
            ? { ok: true, csrfToken: 'resumed-csrf', expiresAt: sessionExpiresAt(), passkeyRequired: false }
            : { ok: true, dataVersion: 8 }) });
    }) as never;
};

test('refresh and background dismissal resume an encrypted finalized session without WebAuthn', async () => {
    await seedFinalizedSession();
    const keys = await readStoredKeys();
    expect(keys.session).toBeDefined();
    expect(JSON.stringify(keys)).not.toContain('preserved-secret');
    expect(keys.responseKey.extractable).toBe(false);
    for (let restart = 0; restart < 2; restart += 1) {
        suspendMobileRelayRuntime();
        expect(isMobileRelayUnlocked()).toBe(false);
        const paths: string[] = [];
        installResumingPeer(paths);
        expect((await new DatabaseService().getSyncStatus()).dataVersion).toBe(8);
        expect(paths).toEqual(['/auth/session', '/api/status', '/api/status']);
        expect(getMobileCsrfToken()).toBe('resumed-csrf');
        expect(isMobileRelayUnlocked()).toBe(true);
    }
    expect(navigator.credentials.get).not.toHaveBeenCalled();
    expect(navigator.credentials.create).not.toHaveBeenCalled();
});

test('temporary network failure preserves the resumed cookie, pairing and pending edits', async () => {
    await seedFinalizedSession();
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"pending-account"}');
    const original = await readStoredKeys();
    suspendMobileRelayRuntime();
    globalThis.fetch = mock(async () => { throw new TypeError('offline'); }) as never;
    const db = new DatabaseService();
    await expect(db.getSyncStatus()).rejects.toThrow('offline');
    expect((await readStoredKeys()).session).toEqual(original.session);
    expect(getMobileApiBaseUrl()).toBe(endpoint);
    expect(await offlineStore.listMutations()).toHaveLength(1);
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('the desktop decides expiry when the locally remembered expiration is older', async () => {
    await seedFinalizedSession();
    const db = await new Promise<IDBDatabase>(resolve => { const request = indexedDB.open('dmxmoney-relay-keys', 1); request.onsuccess = () => resolve(request.result); });
    await new Promise<void>((resolve, reject) => {
        const transaction = db.transaction('keys', 'readwrite');
        const store = transaction.objectStore('keys');
        const request = store.get(endpoint);
        request.onsuccess = () => { request.result.session.expiresAt = Date.now() - 1000; store.put(request.result); };
        transaction.oncomplete = () => resolve();
        transaction.onabort = () => reject(transaction.error);
    });
    db.close();
    suspendMobileRelayRuntime();
    const paths: string[] = [];
    installResumingPeer(paths);
    expect((await new DatabaseService().getSyncStatus()).dataVersion).toBe(8);
    expect(paths[0]).toBe('/auth/session');
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('server revocation invalidates only the session and preserves keys and pending edits', async () => {
    await seedFinalizedSession();
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"pending-account"}');
    const original = await readStoredKeys();
    suspendMobileRelayRuntime();
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const request = await decodePeerRequest(init);
        expect(request.path).toBe('/auth/session');
        return respond(request.id, { id: request.id, status: 401, headers: {}, body: '{"error":"revoked"}' });
    }) as never;
    const db = new DatabaseService();
    try { await db.getSyncStatus(); throw new Error('Expected revocation'); }
    catch (error) { expect(db.isAuthenticationRequired(error)).toBe(true); }
    expect(isMobileRelayUnlocked()).toBe(false);
    expect((await readStoredKeys()).session).toBeUndefined();
    expect((await readStoredKeys()).authorization).toBe(original.authorization);
    expect(await offlineStore.listMutations()).toHaveLength(1);
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('a late session resume cannot reopen the PWA after voluntary lock', async () => {
    await seedFinalizedSession();
    suspendMobileRelayRuntime();
    let release!: () => void;
    const released = new Promise<void>(resolve => { release = resolve; });
    let entered!: () => void;
    const requestEntered = new Promise<void>(resolve => { entered = resolve; });
    let logoutEntered!: () => void;
    const logoutReceived = new Promise<void>(resolve => { logoutEntered = resolve; });
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const request = await decodePeerRequest(init);
        if (request.path === '/auth/session') { entered(); await released; }
        if (request.path === '/auth/logout') {
            expect(request.headers.cookie).toBe('dmxmoney_session=preserved-secret');
            logoutEntered();
        }
        return respond(request.id, { id: request.id, status: 200, headers: {}, body: JSON.stringify({ ok: true, csrfToken: 'late-csrf', expiresAt: sessionExpiresAt() }) });
    }) as never;
    const db = new DatabaseService();
    const resuming = db.getSyncStatus();
    const rejected = resuming.catch(error => error);
    await requestEntered;
    await db.lockMobileCompanion();
    await logoutReceived;
    release();
    expect((await rejected as Error).message).toContain('verrouillée');
    expect(isMobileRelayUnlocked()).toBe(false);
    expect(getMobileCsrfToken()).toBeNull();
    expect((await readStoredKeys()).session).toBeUndefined();
    suspendMobileRelayRuntime();
    await expect(new DatabaseService().getSyncStatus()).rejects.toThrow('clé d’accès');
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('a late external HTTP 401 cannot invalidate a newer finalized session', async () => {
    for (const delayBodyOnly of [false, true]) {
        await seedFinalizedSession();
        let release!: () => void;
        const released = new Promise<void>(resolve => { release = resolve; });
        let entered!: () => void;
        const requestEntered = new Promise<void>(resolve => { entered = resolve; });
        globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
            const request = await decodePeerRequest(init);
            expect(request.path).toBe('/api/status');
            expect(request.headers.cookie).toBe('dmxmoney_session=preserved-secret');
            if (delayBodyOnly) return new Response(new ReadableStream({
                async pull(controller) {
                    entered();
                    await released;
                    controller.enqueue(encode.encode('{"error":"old relay authorization"}'));
                    controller.close();
                },
            }), { status: 401 });
            entered();
            await released;
            return new Response('{"error":"old relay authorization"}', { status: 401 });
        }) as never;
        const rejected = new DatabaseService().getSyncStatus().catch(error => error);
        await requestEntered;
        await seedFinalizedSession('new-session-secret', 'new-session-csrf');
        const current = await readStoredKeys();
        release();
        expect((await rejected as Error).message).toContain('verrouillée');
        expect(isMobileRelayUnlocked()).toBe(true);
        expect(getMobileCsrfToken()).toBe('new-session-csrf');
        expect((await readStoredKeys()).session).toEqual(current.session);
        expect(localStorage.getItem(MOBILE_RELAY_LOCK_STORAGE_KEY)).toBeNull();
    }
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('silent resume honors another document lock even before a storage event arrives', async () => {
    for (const changeEpoch of [false, true]) {
        await seedFinalizedSession();
        suspendMobileRelayRuntime();
        let release!: () => void;
        const released = new Promise<void>(resolve => { release = resolve; });
        let entered!: () => void;
        const requestEntered = new Promise<void>(resolve => { entered = resolve; });
        globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
            const request = await decodePeerRequest(init);
            expect(request.path).toBe('/auth/session');
            entered();
            await released;
            return respond(request.id, { id: request.id, status: 200, headers: {}, body: JSON.stringify({
                ok: true, csrfToken: 'late-csrf', expiresAt: sessionExpiresAt(), passkeyRequired: false,
            }) });
        }) as never;
        const db = new DatabaseService();
        const rejected = db.getSyncStatus().catch(error => error);
        await requestEntered;
        if (changeEpoch) localStorage.setItem(MOBILE_RELAY_SESSION_EPOCH_KEY, crypto.randomUUID());
        localStorage.setItem(MOBILE_RELAY_LOCK_STORAGE_KEY, '1');
        const locked = await readStoredKeys();
        release();
        expect((await rejected as Error).message).toContain('verrouillée');
        expect(isMobileRelayUnlocked()).toBe(false);
        expect(getMobileCsrfToken()).toBeNull();
        expect(localStorage.getItem(MOBILE_RELAY_LOCK_STORAGE_KEY)).toBe('1');
        expect((await readStoredKeys()).session).toEqual(locked.session);
        await expect(db.getSyncStatus()).rejects.toThrow('clé d’accès');
    }
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

test('another document cannot replace the CSRF token associated with this cookie', async () => {
    await seedFinalizedSession();
    // A different tab, or an older PWA, may still publish its own token here.
    localStorage.setItem('dmxmoney.secureCsrfToken', 'other-document-csrf');
    const mutations: PeerRequest[] = [];
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const request = await decodePeerRequest(init);
        if (request.method !== 'GET') mutations.push(request);
        return respond(request.id, { id: request.id, status: 200, headers: {}, body: JSON.stringify({ ok: true, dataVersion: 8 }) });
    }) as never;
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"new-account"}');
    await new DatabaseService().getSyncStatus();
    expect(mutations).toHaveLength(1);
    expect(mutations[0].headers.cookie).toBe('dmxmoney_session=preserved-secret');
    expect(mutations[0].headers['x-dmx-csrf']).toBe('previous-csrf');
    expect(getMobileCsrfToken()).toBe('previous-csrf');
    expect(await offlineStore.listMutations()).toHaveLength(0);
    expect(navigator.credentials.get).not.toHaveBeenCalled();
});

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
    await markMobileRelayAuthenticated(sessionExpiresAt());
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
    setMobileApiBaseUrl('https://relay.test/relay/desktop-one');
    setMobileApiBaseUrl('https://relay.test/relay/desktop-two');
    expect(getMobilePreviousApiBaseUrls()).toEqual([]);
    setMobileApiBaseUrl('https://other-relay.test/relay/desktop-one');
    expect(getMobilePreviousApiBaseUrls()).toEqual([]);
    expect(applyMobileCompanionPairingUrl(`#relay=${encodeURIComponent(endpoint)}&key=${encodedKey}&pairing=token`).ok).toBe(true);
    expect(getMobileApiBaseUrl()).toBe(endpoint);
    expect(localStorage.getItem('dmxmoney.remoteRelayEndpoint')).toBe(endpoint);
    expect(localStorage.getItem('key')).toBeNull();
});

test('new pairing requires an encrypted Internet relay and never adopts a local API QR', () => {
    for (const fragment of [
        '#api=https://desktop.test:8443&pairing=token',
        `#relay=${encodeURIComponent(endpoint)}&pairing=token`,
        `#relay=${encodeURIComponent(endpoint)}&key=${encodedKey}`,
        `#relay=${encodeURIComponent(endpoint)}&key=${encodedKey}&pairing=token&api=https://desktop.test:8443`,
    ]) {
        expect(applyMobileCompanionPairingUrl(fragment).ok).toBe(false);
        expect(getMobileApiBaseUrl()).toBeNull();
        expect(localStorage.getItem('dmxmoney.remoteRelayEndpoint')).toBeNull();
        expect(localStorage.getItem('dmxmoney.securePairingToken')).toBeNull();
    }
});

test('stale local configuration requires normal pairing without erasing stored metadata', () => {
    localStorage.setItem('dmxmoney.secureApiBaseUrl', 'https://desktop.test:8443');
    localStorage.setItem('dmxmoney.securePairingToken', 'previous-token');
    localStorage.setItem('dmxmoney.securePasskeyReady', '1');
    expect(getMobileApiBaseUrl()).toBeNull();
    expect(hasMobileCompanionSetup()).toBe(false);
    expect(hasMobilePasskeySetup()).toBe(false);
    expect(isMobileRelayUnlocked()).toBe(false);
    expect(localStorage.getItem('dmxmoney.secureApiBaseUrl')).toBe('https://desktop.test:8443');
    expect(localStorage.getItem('dmxmoney.securePairingToken')).toBe('previous-token');
});

test('transport never sends financial data directly to an API without the matching encrypted relay', async () => {
    globalThis.fetch = mock(async () => new Response('must not be contacted')) as never;
    await expect(mobileTransportFetch('/api/accounts', 'https://desktop.test:8443', { method: 'POST', body: 'private data' })).rejects.toThrow();
    await expect(mobileTransportFetch('/api/accounts', endpoint, {})).rejects.toThrow();
    await configureMobileRelay(endpoint, encodedKey);
    await expect(mobileTransportFetch('/api/accounts', 'https://other.test/relay/different-device', {})).rejects.toThrow();
    expect(globalThis.fetch).not.toHaveBeenCalled();
});

test('relay cached financial data stays hidden until a passkey session is finalized', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    await Promise.all((['accounts', 'transactions', 'categories', 'scheduled', 'budgets'] as const).map(key => offlineStore.setData(key, [])));
    globalThis.fetch = mock(() => { throw new Error('Network must not be used'); }) as never;
    const db = new DatabaseService();
    expect(await db.getCachedBankData()).toBeNull();
    await markMobileRelayAuthenticated(sessionExpiresAt());
    expect(await db.getCachedBankData()).not.toBeNull();
    expect(globalThis.fetch).not.toHaveBeenCalled();
    await offlineStore.clearAll();
});

test('paginated journals restart on version conflict and cache only a complete snapshot', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    await markMobileRelayAuthenticated(sessionExpiresAt());
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
    await markMobileRelayAuthenticated(sessionExpiresAt());
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

test('oversized pages reduce their limit without dropping or duplicating the completed journal', async () => {
    await configureMobileRelay(endpoint, encodedKey);
    setMobileApiBaseUrl(endpoint);
    await markMobileRelayAuthenticated(sessionExpiresAt());
    const paths: string[] = [];
    globalThis.fetch = mock(async (_url: string, init: RequestInit) => {
        const envelope = JSON.parse(init.body as string);
        const clear = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: fromBase64(envelope.nonce) }, await peerKey('request'), fromBase64(envelope.ciphertext));
        const request = JSON.parse(decode.decode(clear));
        paths.push(request.path);
        if (paths.length === 1) return respond(envelope.id, { id: envelope.id, status: 413, headers: {}, body: '{"error":"response_too_large"}' });
        return respond(envelope.id, { id: envelope.id, status: 200, headers: {}, body: JSON.stringify({ transactions: [{ id: 'retained' }], dataVersion: 7, nextOffset: null }) });
    }) as never;
    expect(await new DatabaseService().getTransactions()).toEqual([{ id: 'retained' }] as never);
    expect(paths).toEqual(['/api/transactions/page?offset=0&limit=2000', '/api/transactions/page?offset=0&limit=1000']);
});
