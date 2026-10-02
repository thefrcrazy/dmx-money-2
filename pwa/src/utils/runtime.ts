const MOBILE_PAIRING_KEY = 'dmxmoney.securePairingToken';
const MOBILE_API_BASE_KEY = 'dmxmoney.secureApiBaseUrl';
const MOBILE_PREVIOUS_API_BASE_KEY = 'dmxmoney.securePreviousApiBaseUrl';
const MOBILE_CSRF_KEY = 'dmxmoney.secureCsrfToken';
const MOBILE_PASSKEY_READY_KEY = 'dmxmoney.securePasskeyReady';

const normalizeApiBaseUrl = (value: string) => validateRelayEndpoint(value.trim());

const sameDesktopEndpoint = (previous: string, next: string) => {
    try {
        const first = new URL(previous);
        const second = new URL(next);
        return first.origin === second.origin && first.pathname === second.pathname;
    } catch { return false; }
};

/** Enough history to survive a few reconnections before anything is flushed. */
const MAX_PREVIOUS_API_BASE_URLS = 5;

const readPreviousApiBaseUrls = (): string[] => {
    try {
        const raw = localStorage.getItem(MOBILE_PREVIOUS_API_BASE_KEY);
        const parsed = raw ? JSON.parse(raw) : [];
        return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === 'string') : [];
    } catch {
        return [];
    }
};

/**
 * The offline cache and the queued mutations are keyed by the API base URL, so a
 * same encrypted endpoint can retain queued work across reconnections. A
 * different endpoint must never receive another desktop's pending edits.
 */
export const setMobileApiBaseUrl = (value: string) => {
    if (typeof window === 'undefined') return;

    const next = normalizeApiBaseUrl(value);
    if (!next) return;

    const current = localStorage.getItem(MOBILE_API_BASE_KEY);
    if (current && current !== next) {
        const previous = readPreviousApiBaseUrls().filter(url => url !== current && url !== next && sameDesktopEndpoint(url, next));
        localStorage.setItem(
            MOBILE_PREVIOUS_API_BASE_KEY,
            JSON.stringify((sameDesktopEndpoint(current, next) ? [...previous, current] : previous).slice(-MAX_PREVIOUS_API_BASE_URLS)),
        );
        localStorage.removeItem(MOBILE_CSRF_KEY);
        setMobileRelayCsrfToken(null);
        if (!sameDesktopEndpoint(current, next)) localStorage.removeItem(MOBILE_PASSKEY_READY_KEY);
    }
    if (getMobileRelayEndpoint() !== next) localStorage.removeItem('dmxmoney.remoteRelayEndpoint');
    localStorage.setItem(MOBILE_API_BASE_KEY, next);
};

export const getMobilePreviousApiBaseUrls = () => {
    if (typeof window === 'undefined') return [];
    const current = localStorage.getItem(MOBILE_API_BASE_KEY);
    return readPreviousApiBaseUrls().filter(url => url !== current && current && sameDesktopEndpoint(url, current));
};

export const clearMobilePreviousApiBaseUrls = () => {
    if (typeof window === 'undefined') return;
    localStorage.removeItem(MOBILE_PREVIOUS_API_BASE_KEY);
};

const getHashParamsFromValue = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return null;

    try {
        const url = new URL(trimmed, typeof window !== 'undefined' ? window.location.origin : 'https://dmxmoney-companion.pages.dev');
        const hash = url.hash.startsWith('#') ? url.hash.slice(1) : url.hash;
        if (hash) return new URLSearchParams(hash);
    } catch {
        // The input may already be a raw fragment or query string.
    }

    const raw = trimmed.startsWith('#') ? trimmed.slice(1) : trimmed;
    return new URLSearchParams(raw);
};

export const hasTauriRuntime = () => {
    if (typeof window === 'undefined') return false;
    return Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
};

export const isMobileCompanion = () => {
    if (typeof window === 'undefined' || hasTauriRuntime()) return false;
    return window.location.pathname.startsWith('/mobile')
        || Boolean(localStorage.getItem(MOBILE_API_BASE_KEY));
};

export const isStandalonePwa = () => {
    if (typeof window === 'undefined') return false;
    return window.matchMedia?.('(display-mode: standalone)').matches
        || Boolean((navigator as Navigator & { standalone?: boolean }).standalone);
};

export const getMobilePlatform = (): 'ios' | 'android' | 'other' => {
    if (typeof navigator === 'undefined') return 'other';
    const userAgent = navigator.userAgent;
    const platform = navigator.platform || '';
    if (/iPhone|iPad|iPod/i.test(userAgent) || (platform === 'MacIntel' && navigator.maxTouchPoints > 1)) {
        return 'ios';
    }
    if (/Android/i.test(userAgent)) return 'android';
    return 'other';
};

export const initializeMobileCompanionToken = () => {
    if (typeof window === 'undefined') return;

    const hash = window.location.hash.startsWith('#')
        ? window.location.hash.slice(1)
        : window.location.hash;
    if (!hash) return;

    const params = getHashParamsFromValue(hash);
    if (params?.has('pairing') || params?.has('api') || params?.has('relay') || params?.has('key')) {
        applyMobileCompanionPairingUrl(hash);
        window.history.replaceState(null, document.title, `${window.location.pathname}${window.location.search}`);
    }
};

export const applyMobileCompanionPairingUrl = (value: string) => {
    if (typeof window === 'undefined') {
        return { ok: false, error: 'Navigateur indisponible.' };
    }

    const params = getHashParamsFromValue(value);
    const pairing = params?.get('pairing');
    const api = params?.get('api');
    const relay = params?.get('relay');
    const key = params?.get('key');

    if (!relay || !key || !pairing) {
        return { ok: false, error: 'Lien d’appairage Internet incomplet. Scannez le QR affiché dans DmxMoney sur votre ordinateur.' };
    }
    try {
        const endpoint = validateRelayEndpoint(relay);
        if (api && normalizeApiBaseUrl(api) !== endpoint) throw new Error('Les adresses du QR ne correspondent pas.');
        void configureMobileRelay(endpoint, key);
        localStorage.removeItem(MOBILE_CSRF_KEY);
        setMobileApiBaseUrl(endpoint);
        localStorage.setItem(MOBILE_PAIRING_KEY, pairing);
        return { ok: true, error: null };
    } catch (error) {
        return { ok: false, error: error instanceof Error ? error.message : 'Lien QR invalide.' };
    }
};

export const getMobileApiBaseUrl = () => {
    if (typeof window === 'undefined') return null;
    const value = localStorage.getItem(MOBILE_API_BASE_KEY);
    try { return value ? normalizeApiBaseUrl(value) : null; } catch { return null; }
};

export const getMobilePairingToken = () => {
    if (typeof window === 'undefined') return null;
    return localStorage.getItem(MOBILE_PAIRING_KEY);
};

export const hasMobileCompanionSetup = () => {
    if (typeof window === 'undefined') return false;
    const endpoint = getMobileApiBaseUrl();
    return Boolean(endpoint && getMobileRelayEndpoint() === endpoint);
};

export const hasMobilePasskeySetup = () => {
    if (typeof window === 'undefined') return false;
    return hasMobileCompanionSetup() && (
        Boolean(localStorage.getItem(MOBILE_PASSKEY_READY_KEY))
        || !localStorage.getItem(MOBILE_PAIRING_KEY)
    );
};

export const markMobilePasskeyReady = () => {
    if (typeof window === 'undefined') return;
    localStorage.setItem(MOBILE_PASSKEY_READY_KEY, '1');
};

export const clearMobilePairingToken = () => {
    if (typeof window === 'undefined') return;
    localStorage.removeItem(MOBILE_PAIRING_KEY);
};

export const clearMobileCompanionLocalState = () => {
    if (typeof window === 'undefined') return;

    Object.keys(localStorage)
        .filter(key => key.startsWith('dmxmoney.'))
        .forEach(key => localStorage.removeItem(key));

    sessionStorage.clear();
};

export const getMobileCsrfToken = () => {
    if (typeof window === 'undefined') return null;
    return getMobileRelayCsrfToken();
};

export const setMobileCsrfToken = (token: string | null) => {
    if (typeof window === 'undefined') return;
    setMobileRelayCsrfToken(token);
    localStorage.removeItem(MOBILE_CSRF_KEY);
};
import { configureMobileRelay, getMobileRelayCsrfToken, getMobileRelayEndpoint, setMobileRelayCsrfToken, validateRelayEndpoint } from '../services/relayTransport';
