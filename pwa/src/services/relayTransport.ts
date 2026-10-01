const RELAY_ENDPOINT_KEY = 'dmxmoney.remoteRelayEndpoint';
const DATABASE_NAME = 'dmxmoney-relay-keys';
const MAX_BODY_BYTES = 8 * 1024 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface RelayKeys {
    endpoint: string;
    requestKey: CryptoKey;
    responseKey: CryptoKey;
    authorization: string;
}

interface RelayEnvelope {
    id: string;
    nonce: string;
    ciphertext: string;
}

interface RelayResponse {
    id: string;
    status: number;
    headers: Record<string, string>;
    body: string;
}

const pendingConfigurations = new Map<string, Promise<void>>();
// Authentication cookies travel only inside encrypted envelopes. They never
// become cookies of the relay operator's origin, and disappear when this tab closes.
const sessionCookies = new Map<string, string>();
let unlockedUntil = 0;
let unlockedEndpoint: string | null = null;
let lockTimer: ReturnType<typeof setTimeout> | null = null;
export const MOBILE_RELAY_LOCK_EVENT = 'dmxmoney-mobile-relay-lock';

const lockRelay = () => {
    if (unlockedEndpoint) sessionCookies.delete(unlockedEndpoint);
    unlockedUntil = 0;
    unlockedEndpoint = null;
    if (lockTimer) clearTimeout(lockTimer);
    lockTimer = null;
    if (typeof window !== 'undefined' && typeof window.dispatchEvent === 'function') window.dispatchEvent(new Event(MOBILE_RELAY_LOCK_EVENT));
};

export const markMobileRelayAuthenticated = () => {
    const endpoint = getMobileRelayEndpoint();
    if (!endpoint) return;
    if (lockTimer) clearTimeout(lockTimer);
    unlockedEndpoint = endpoint;
    unlockedUntil = Date.now() + 45 * 60 * 1000;
    lockTimer = setTimeout(lockRelay, 45 * 60 * 1000);
};

export const isMobileRelayUnlocked = () => {
    const endpoint = getMobileRelayEndpoint();
    if (!endpoint) return true;
    return unlockedEndpoint === endpoint && unlockedUntil > Date.now();
};

export const bytesToBase64Url = (bytes: Uint8Array): string => {
    let value = '';
    for (let offset = 0; offset < bytes.length; offset += 8192) {
        value += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
    }
    return btoa(value).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

const base64UrlToBytes = (value: string): Uint8Array<ArrayBuffer> => {
    if (!/^[A-Za-z0-9_-]+$/.test(value)) throw new Error('Clé ou message de relais invalide.');
    const binary = atob(value.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(value.length / 4) * 4, '='));
    return Uint8Array.from(binary, char => char.charCodeAt(0));
};

export const validateRelayEndpoint = (value: string): string => {
    const url = new URL(value);
    if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash
        || !/^\/relay\/[A-Za-z0-9_-]{8,128}\/?$/.test(url.pathname)) {
        throw new Error('Adresse de relais HTTPS invalide.');
    }
    return `${url.origin}${url.pathname.replace(/\/$/, '')}`;
};

const openKeyDatabase = (): Promise<IDBDatabase> => new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore('keys', { keyPath: 'endpoint' });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error('Stockage sécurisé du relais indisponible.'));
});

const storeKeys = async (keys: RelayKeys) => {
    const db = await openKeyDatabase();
    try {
        await new Promise<void>((resolve, reject) => {
            const transaction = db.transaction('keys', 'readwrite');
            transaction.objectStore('keys').put(keys);
            transaction.oncomplete = () => resolve();
            transaction.onabort = transaction.onerror = () => reject(transaction.error || new Error('Impossible de conserver la clé du relais.'));
        });
    } finally { db.close(); }
};

const loadKeys = async (endpoint: string): Promise<RelayKeys> => {
    await pendingConfigurations.get(endpoint);
    const db = await openKeyDatabase();
    try {
        return await new Promise<RelayKeys>((resolve, reject) => {
            const request = db.transaction('keys').objectStore('keys').get(endpoint);
            request.onsuccess = () => request.result ? resolve(request.result) : reject(new Error('Clé du relais absente. Scannez un nouveau QR depuis votre ordinateur.'));
            request.onerror = () => reject(request.error);
        });
    } finally { db.close(); }
};

export const configureMobileRelay = (endpointInput: string, encodedSecret: string): Promise<void> => {
    const endpoint = validateRelayEndpoint(endpointInput);
    const secret = base64UrlToBytes(encodedSecret);
    if (secret.length !== 32 || bytesToBase64Url(secret) !== encodedSecret) throw new Error('La clé du relais doit contenir 32 octets.');
    const configuration = (async () => {
        const material = await crypto.subtle.importKey('raw', secret, 'HKDF', false, ['deriveKey']);
        const derive = (direction: 'request' | 'response') => crypto.subtle.deriveKey({
            name: 'HKDF', hash: 'SHA-256', salt: encoder.encode('dmx-relay-v1'), info: encoder.encode(direction),
        }, material, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
        const [requestKey, responseKey, authorization] = await Promise.all([
            derive('request'), derive('response'), crypto.subtle.digest('SHA-256', encoder.encode(encodedSecret)),
        ]);
        await storeKeys({ endpoint, requestKey, responseKey, authorization: bytesToBase64Url(new Uint8Array(authorization)) });
        sessionCookies.delete(endpoint);
        lockRelay();
    })();
    pendingConfigurations.set(endpoint, configuration);
    // Attach a rejection handler immediately; transport still awaits the original
    // promise and reports any storage failure to the user.
    void configuration.catch(() => undefined);
    localStorage.setItem(RELAY_ENDPOINT_KEY, endpoint);
    return configuration;
};

export const getMobileRelayEndpoint = (): string | null => {
    if (typeof window === 'undefined') return null;
    return localStorage.getItem(RELAY_ENDPOINT_KEY);
};

export const clearMobileRelay = async (): Promise<void> => {
    await Promise.allSettled(pendingConfigurations.values());
    pendingConfigurations.clear();
    sessionCookies.clear();
    lockRelay();
    localStorage.removeItem(RELAY_ENDPOINT_KEY);
    const db = await openKeyDatabase();
    try {
        await new Promise<void>((resolve, reject) => {
            const transaction = db.transaction('keys', 'readwrite');
            transaction.objectStore('keys').clear();
            transaction.oncomplete = () => resolve();
            transaction.onabort = transaction.onerror = () => reject(transaction.error);
        });
    } finally { db.close(); }
};

export const mobileTransportFetch = async (path: string, apiBaseUrl: string, init: RequestInit): Promise<Response> => {
    const endpoint = getMobileRelayEndpoint();
    if (!endpoint || endpoint !== apiBaseUrl) return fetch(new URL(path, apiBaseUrl).toString(), init);
    if (!/^\/(?:api|auth)\//.test(path) || path.includes('..')) throw new Error('Route de relais interdite.');
    const keys = await loadKeys(endpoint);
    const id = crypto.randomUUID();
    const headers = Object.fromEntries(new Headers(init.headers).entries());
    const cookie = sessionCookies.get(endpoint);
    if (cookie) headers.cookie = cookie;
    const body = typeof init.body === 'string' ? init.body : '';
    if (encoder.encode(body).length > MAX_BODY_BYTES) throw new Error('La modification dépasse la taille autorisée par le relais.');
    const plaintext = encoder.encode(JSON.stringify({ id, issuedAt: Date.now(), method: init.method || 'GET', path, headers, body }));
    if (plaintext.length + 16 > MAX_BODY_BYTES + 32 * 1024) throw new Error('La modification dépasse la taille chiffrée autorisée par le relais.');
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce }, keys.requestKey, plaintext);
    const response = await fetch(`${endpoint}/request`, {
        method: 'POST', cache: 'no-store', credentials: 'omit', signal: init.signal,
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${keys.authorization}` },
        body: JSON.stringify({ id, nonce: bytesToBase64Url(nonce), ciphertext: bytesToBase64Url(new Uint8Array(ciphertext)) }),
    });
    if (!response.ok) return response;
    const envelope = await response.json() as RelayEnvelope;
    if (envelope.id !== id || typeof envelope.nonce !== 'string' || envelope.nonce.length !== 16
        || typeof envelope.ciphertext !== 'string' || envelope.ciphertext.length > 12 * 1024 * 1024) {
        throw new Error('Réponse du relais invalide.');
    }
    const clear = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: base64UrlToBytes(envelope.nonce) }, keys.responseKey, base64UrlToBytes(envelope.ciphertext));
    const payload = JSON.parse(decoder.decode(clear)) as RelayResponse;
    if (payload.id !== id || !Number.isInteger(payload.status) || payload.status < 200 || payload.status > 599
        || typeof payload.body !== 'string' || !payload.headers || typeof payload.headers !== 'object') {
        throw new Error('Réponse chiffrée du relais invalide.');
    }
    const responseHeaders = new Headers(payload.headers);
    const setCookie = responseHeaders.get('set-cookie');
    if (setCookie) {
        const session = setCookie.split(';')[0];
        if (!/^dmxmoney_session=/.test(session)) throw new Error('Session du relais invalide.');
        if (/max-age=0(?:;|$)/i.test(setCookie) || session === 'dmxmoney_session=') sessionCookies.delete(endpoint);
        else sessionCookies.set(endpoint, session);
        responseHeaders.delete('set-cookie');
    }
    return new Response([204, 205, 304].includes(payload.status) ? null : payload.body, { status: payload.status, headers: responseHeaders });
};
