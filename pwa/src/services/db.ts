import { applyTransactionUpdate } from '../utils/transactionUpdate';
import { Account, Transaction, Category, ScheduledTransaction, Settings, Budget } from '../types';
import { offlineStore, OfflineDataKey } from './offlineStore';
import {
    clearMobileCompanionLocalState,
    clearMobilePairingToken,
    clearMobilePreviousApiBaseUrls,
    getMobileApiBaseUrl,
    getMobileCsrfToken,
    getMobilePairingToken,
    getMobilePreviousApiBaseUrls,
    hasTauriRuntime,
    isMobileCompanion,
    isStandalonePwa,
    markMobilePasskeyReady,
    setMobileApiBaseUrl,
    setMobileCsrfToken
} from '../utils/runtime';
import { selectNewestVersion } from '../utils/version';
import {
    applySettingsMutation,
    createSettingsMutation,
    hasSettingsMutationChanges,
    mergeSettingsMutations,
    SettingsMutation,
} from './settingsSync';

type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

interface RawSettings extends Omit<Settings,
    | 'accountGroups'
    | 'customGroups'
    | 'customGroupsOrder'
    | 'accountsOrder'
    | 'dismissedBudgetSuggestions'
    | 'dismissedScheduledSuggestions'
    | 'predictionFakeTransactions'
    | 'analyticsHiddenExpenseCategories'
    | 'analyticsHiddenIncomeCategories'
> {
    accountGroups?: string | Settings['accountGroups'] | null;
    customGroups?: string | Settings['customGroups'] | null;
    customGroupsOrder?: string | Settings['customGroupsOrder'] | null;
    accountsOrder?: string | Settings['accountsOrder'] | null;
    dismissedBudgetSuggestions?: string | Settings['dismissedBudgetSuggestions'] | null;
    dismissedScheduledSuggestions?: string | Settings['dismissedScheduledSuggestions'] | null;
    predictionFakeTransactions?: string | Settings['predictionFakeTransactions'] | null;
    analyticsHiddenExpenseCategories?: string | Settings['analyticsHiddenExpenseCategories'] | null;
    analyticsHiddenIncomeCategories?: string | Settings['analyticsHiddenIncomeCategories'] | null;
}

interface SyncStatus {
    ok: boolean;
    dataVersion: number;
}

interface SettingsPatchResult {
    ok: boolean;
    revision: number;
    conflicts: string[];
}

class MobileNetworkError extends Error {
    constructor(message = 'Serveur mobile local indisponible.') {
        super(message);
        this.name = 'MobileNetworkError';
    }
}

class MobilePasskeyInvalidStateError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'MobilePasskeyInvalidStateError';
    }
}

interface SecureSessionResponse {
    ok: boolean;
    csrfToken: string;
    passkeyRequired?: boolean;
}

interface WebauthnOptionsResponse<T> {
    challengeId: string;
    publicKey: T;
}

type RegistrationChallenge = Omit<PublicKeyCredentialCreationOptions, 'challenge' | 'user' | 'excludeCredentials'> & {
    challenge: string;
    user: PublicKeyCredentialUserEntity & { id: string };
    excludeCredentials?: Array<PublicKeyCredentialDescriptor & { id: string }>;
};

type AuthenticationChallenge = Omit<PublicKeyCredentialRequestOptions, 'challenge' | 'allowCredentials'> & {
    challenge: string;
    allowCredentials?: Array<PublicKeyCredentialDescriptor & { id: string }>;
};

const parseMaybeJson = <T>(value: string | T | null | undefined): T | undefined => {
    if (!value) return undefined;
    if (typeof value === 'string') return JSON.parse(value) as T;
    return value;
};

const parseSettings = (res: RawSettings | null): Settings | null => {
    if (!res) return null;

    return {
        ...res,
        accountGroups: parseMaybeJson<Settings['accountGroups']>(res.accountGroups),
        customGroups: parseMaybeJson<Settings['customGroups']>(res.customGroups),
        customGroupsOrder: parseMaybeJson<Settings['customGroupsOrder']>(res.customGroupsOrder),
        accountsOrder: parseMaybeJson<Settings['accountsOrder']>(res.accountsOrder),
        dismissedBudgetSuggestions: parseMaybeJson<Settings['dismissedBudgetSuggestions']>(res.dismissedBudgetSuggestions) || [],
        dismissedScheduledSuggestions: parseMaybeJson<Settings['dismissedScheduledSuggestions']>(res.dismissedScheduledSuggestions) || [],
        predictionFakeTransactions: parseMaybeJson<Settings['predictionFakeTransactions']>(res.predictionFakeTransactions) || [],
        analyticsHiddenExpenseCategories: parseMaybeJson<Settings['analyticsHiddenExpenseCategories']>(res.analyticsHiddenExpenseCategories) || [],
        analyticsHiddenIncomeCategories: parseMaybeJson<Settings['analyticsHiddenIncomeCategories']>(res.analyticsHiddenIncomeCategories) || []
    };
};

const serializeSettings = (settings: Settings): RawSettings => ({
    ...settings,
    accountGroups: settings.accountGroups ? JSON.stringify(settings.accountGroups) : null,
    customGroups: settings.customGroups ? JSON.stringify(settings.customGroups) : null,
    customGroupsOrder: settings.customGroupsOrder ? JSON.stringify(settings.customGroupsOrder) : null,
    accountsOrder: settings.accountsOrder ? JSON.stringify(settings.accountsOrder) : null,
    dismissedBudgetSuggestions: JSON.stringify(settings.dismissedBudgetSuggestions || []),
    dismissedScheduledSuggestions: JSON.stringify(settings.dismissedScheduledSuggestions || []),
    predictionFakeTransactions: JSON.stringify(settings.predictionFakeTransactions || []),
    analyticsHiddenExpenseCategories: JSON.stringify(settings.analyticsHiddenExpenseCategories || []),
    analyticsHiddenIncomeCategories: JSON.stringify(settings.analyticsHiddenIncomeCategories || [])
});

const serializeSettingsValues = (
    values: SettingsMutation['values'] | SettingsMutation['expectedValues'],
) => {
    if (!values) return undefined;
    const serialized: Record<string, unknown> = { ...values };

    const serializeJsonField = (key: keyof RawSettings) => {
        if (!Object.prototype.hasOwnProperty.call(values, key)) return;
        const value = values[key as keyof typeof values];
        serialized[key] = value === null || value === undefined ? null : JSON.stringify(value);
    };

    serializeJsonField('accountGroups');
    serializeJsonField('customGroups');
    serializeJsonField('customGroupsOrder');
    serializeJsonField('accountsOrder');
    return serialized;
};

const serializeSettingsMutation = (mutation: SettingsMutation) => ({
    ...mutation,
    values: serializeSettingsValues(mutation.values),
    expectedValues: serializeSettingsValues(mutation.expectedValues),
});

const parseQueuedSettingsMutation = (body?: string): SettingsMutation | null => {
    if (!body) return null;
    try {
        const parsed = JSON.parse(body) as SettingsMutation;
        return typeof parsed === 'object' && parsed !== null ? parsed : null;
    } catch {
        return null;
    }
};

const legacySettingsToMutation = (body?: string): SettingsMutation | null => {
    if (!body) return null;
    try {
        const settings = parseSettings(JSON.parse(body) as RawSettings);
        if (!settings) return null;
        return {
            schemaVersion: 1,
            baseRevision: settings.settingsRevision || 0,
            values: {
                lastSeenVersion: settings.lastSeenVersion,
            },
            dismissedBudgetSuggestionsAdd: settings.dismissedBudgetSuggestions,
            dismissedScheduledSuggestionsAdd: settings.dismissedScheduledSuggestions,
            analyticsHiddenExpenseCategoriesAdd: settings.analyticsHiddenExpenseCategories,
            analyticsHiddenIncomeCategoriesAdd: settings.analyticsHiddenIncomeCategories,
        };
    } catch {
        return null;
    }
};

/**
 * Réponse de l'assistant du Mac. Les montants et l'interprétation viennent de `dmx-core` :
 * le mobile n'en calcule aucun.
 */
export interface AssistantAnswer {
    ok: boolean;
    /** Phrase de réponse. */
    summary: string;
    /** Lignes de détail (comptes, catégories…). */
    details: string[];
    /** Vrai si des données ont été écrites. */
    changed: boolean;
    /** Faux quand la demande n'a pas été comprise. */
    understood: boolean;
    /** Phrase réellement analysée (reformulée par le modèle local du Mac, le cas échéant). */
    interpreted: string;
}

export class DatabaseService {
    private invokeFn: Promise<InvokeFn> | null = null;
    private flushPromise: Promise<void> | null = null;
    private sessionPromise: Promise<void> | null = null;
    private recoveryPromise: Promise<boolean> | null = null;
    private lastRecoveryAt = 0;
    private readonly requestTimeoutMs = 2500;
    private readonly statusTimeoutMs = 2500;
    private readonly probeTimeoutMs = 1500;
    /** The desktop walks up from its preferred port when that port is taken. */
    private readonly portSweepRange = 6;
    private readonly recoveryCooldownMs = 30_000;

    async init(): Promise<void> {
        await this.getAccounts();
    }

    private usesHttp() {
        return isMobileCompanion();
    }

    private async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
        if (!hasTauriRuntime()) {
            throw new Error('API Tauri indisponible dans ce contexte.');
        }

        if (!this.invokeFn) {
            this.invokeFn = import('@tauri-apps/api/core').then(mod => mod.invoke as InvokeFn);
        }

        const invoke = await this.invokeFn;
        return invoke<T>(command, args);
    }

    private async request<T>(
        path: string,
        init: RequestInit = {},
        timeoutMs = this.requestTimeoutMs,
        retrySession = true,
        allowRecovery = true,
    ): Promise<T> {
        const apiBaseUrl = getMobileApiBaseUrl();
        const method = init.method || 'GET';
        if (!apiBaseUrl) {
            throw new Error('Configuration mobile manquante. Ouvrez le lien depuis le QR code.');
        }

        const headers = new Headers(init.headers);
        const csrfToken = getMobileCsrfToken();
        if (path.startsWith('/api/') && method !== 'GET' && !csrfToken) {
            if (retrySession) {
                await this.ensureSecureMobileSession();
                return this.request<T>(path, init, timeoutMs, false, allowRecovery);
            }
            throw new MobileNetworkError('Session mobile à reconnecter.');
        }
        if (csrfToken && method !== 'GET') {
            headers.set('X-Dmx-Csrf', csrfToken);
        }
        if (init.body && !headers.has('Content-Type')) {
            headers.set('Content-Type', 'application/json');
        }

        const controller = new AbortController();
        const timeoutId = window.setTimeout(() => controller.abort(), timeoutMs);
        const url = new URL(path, apiBaseUrl || window.location.origin).toString();

        let response: Response;
        let text: string;
        try {
            response = await fetch(url, {
                ...init,
                cache: 'no-store',
                credentials: 'include',
                headers,
                signal: controller.signal
            });
            text = await response.text();
        } catch (error) {
            const unreachable = (error instanceof DOMException && error.name === 'AbortError')
                || error instanceof TypeError;
            if (!unreachable) throw error;

            window.clearTimeout(timeoutId);
            // The desktop may have come back on another port. Find it again and
            // replay the request instead of stranding the mobile offline.
            if (allowRecovery && await this.recoverMobileApiBaseUrl()) {
                return this.request<T>(path, init, timeoutMs, retrySession, false);
            }
            throw error instanceof DOMException
                ? new MobileNetworkError('Serveur mobile local indisponible ou trop lent.')
                : new MobileNetworkError((error as TypeError).message);
        } finally {
            window.clearTimeout(timeoutId);
        }

        if (!response.ok) {
            let message = text || `Erreur HTTP ${response.status}`;
            try {
                message = JSON.parse(text).error || message;
            } catch {
                // Keep the raw HTTP body when it is not JSON.
            }
            if (apiBaseUrl && response.status === 401) {
                setMobileCsrfToken(null);
                if (retrySession && path.startsWith('/api/')) {
                    await this.ensureSecureMobileSession();
                    return this.request<T>(path, init, timeoutMs, false, allowRecovery);
                }
            }
            throw new Error(message);
        }

        if (!text) return undefined as T;
        return JSON.parse(text) as T;
    }

    /**
     * The pairing QR pins the desktop's host *and* port, and the desktop walks up
     * from its preferred port whenever that port is busy. Rather than asking for a
     * new QR, probe the neighbouring ports on the same bridge host and adopt the
     * one that answers, carrying the offline queue over with it.
     */
    private async recoverMobileApiBaseUrl(): Promise<boolean> {
        if (!this.usesHttp()) return false;
        if (this.recoveryPromise) return this.recoveryPromise;
        if (Date.now() - this.lastRecoveryAt < this.recoveryCooldownMs) return false;

        this.recoveryPromise = (async () => {
            this.lastRecoveryAt = Date.now();
            const current = getMobileApiBaseUrl();
            if (!current) return false;

            let url: URL;
            try {
                url = new URL(current);
            } catch {
                return false;
            }

            const basePort = Number(url.port);
            if (!Number.isInteger(basePort)) return false;

            const candidates: number[] = [];
            for (let offset = 1; offset <= this.portSweepRange; offset += 1) {
                candidates.push(basePort + offset, basePort - offset);
            }

            for (const port of candidates) {
                if (port < 1 || port > 65535) continue;
                const candidate = `${url.protocol}//${url.hostname}:${port}`;
                if (await this.probeMobileBridge(candidate)) {
                    console.info('Mobile bridge found again on', candidate);
                    setMobileApiBaseUrl(candidate);
                    setMobileCsrfToken(null);
                    this.lastRecoveryAt = 0;
                    return true;
                }
            }
            return false;
        })().finally(() => {
            this.recoveryPromise = null;
        });

        return this.recoveryPromise;
    }

    private async probeMobileBridge(baseUrl: string): Promise<boolean> {
        const controller = new AbortController();
        const timeoutId = window.setTimeout(() => controller.abort(), this.probeTimeoutMs);
        try {
            const response = await fetch(new URL('/api/status', baseUrl).toString(), {
                cache: 'no-store',
                credentials: 'include',
                signal: controller.signal,
            });
            // An unauthenticated /api/status answers 401, which already proves the
            // bridge is the one listening here. Session cookies ignore ports, so a
            // 200 is possible too: check it really is our status payload.
            if (response.status === 401) return true;
            if (!response.ok) return false;
            const payload = await response.json().catch(() => null);
            return typeof payload?.dataVersion === 'number';
        } catch {
            return false;
        } finally {
            window.clearTimeout(timeoutId);
        }
    }

    /**
     * Moves the offline cache and the unsent mutations onto the endpoint we are
     * now talking to, so changes made on the mobile while the desktop was away
     * are not lost when the bridge comes back at a different address.
     */
    private async adoptPendingOfflineWork() {
        const previousUrls = getMobilePreviousApiBaseUrls();
        if (previousUrls.length === 0) return;

        try {
            let migrated = 0;
            // Oldest first, so the queue keeps its original order.
            for (const previous of previousUrls) {
                migrated += await offlineStore.migrateScope(previous);
            }
            clearMobilePreviousApiBaseUrls();
            if (migrated > 0) {
                console.info(`Mobile offline: ${migrated} modification(s) reprises depuis ${previousUrls.join(', ')}`);
            }
        } catch (error) {
            console.warn('Mobile offline scope migration failed:', error);
        }
    }

    private async ensureSecureMobileSession() {
        if (!getMobileApiBaseUrl() || getMobileCsrfToken()) return;
        if (this.sessionPromise) return this.sessionPromise;

        this.sessionPromise = this.createSecureMobileSession().finally(() => {
            this.sessionPromise = null;
        });
        return this.sessionPromise;
    }

    private async createSecureMobileSession() {
        const pairingToken = getMobilePairingToken();
        if (pairingToken) {
            setMobileCsrfToken(null);
            let pairingStarted = false;
            try {
                await this.requestSecureAuth<SecureSessionResponse>('/auth/pairing/start', {
                    method: 'POST',
                    body: JSON.stringify({ token: pairingToken, deviceLabel: this.mobileDeviceLabel() })
                });
                pairingStarted = true;
                try {
                    await this.registerMobilePasskey();
                } catch (error) {
                    if (error instanceof MobilePasskeyInvalidStateError) {
                        clearMobilePairingToken();
                        await this.loginWithMobilePasskey();
                        return;
                    }
                    throw error;
                }
                clearMobilePairingToken();
            } catch (error) {
                setMobileCsrfToken(null);
                if (pairingStarted) clearMobilePairingToken();
                throw error;
            }
            return;
        }

        await this.loginWithMobilePasskey();
    }

    async connectMobileCompanion() {
        if (!this.usesHttp()) return;
        await this.ensureSecureMobileSession();
        await this.getSyncStatus();
    }

    async unlinkMobileCompanion() {
        if (this.usesHttp() && getMobileApiBaseUrl()) {
            try {
                await this.requestSecureAuth('/auth/unlink', { method: 'POST' });
            } catch (error) {
                console.warn('Mobile passkey unlink failed, clearing local state anyway:', error);
                try {
                    await this.requestSecureAuth('/auth/logout', { method: 'POST' });
                } catch {
                    // Local unlink must remain possible offline.
                }
            }
        }

        this.flushPromise = null;
        this.sessionPromise = null;
        setMobileCsrfToken(null);

        try {
            await offlineStore.clearAll();
        } catch (error) {
            console.warn('Mobile offline cache clear failed:', error);
        }

        clearMobileCompanionLocalState();
    }

    private async requestSecureAuth<T>(path: string, init: RequestInit): Promise<T> {
        const apiBaseUrl = getMobileApiBaseUrl();
        if (!apiBaseUrl) throw new Error('URL API sécurisée manquante.');
        const headers = new Headers(init.headers);
        if (init.body && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json');
        const controller = new AbortController();
        const timeoutId = window.setTimeout(() => controller.abort(), this.requestTimeoutMs);
        let response: Response;
        let text: string;
        try {
            response = await fetch(new URL(path, apiBaseUrl).toString(), {
                ...init,
                cache: 'no-store',
                credentials: 'include',
                headers,
                signal: controller.signal,
            });
            text = await response.text();
        } catch (error) {
            if (error instanceof DOMException && error.name === 'AbortError') {
                throw new MobileNetworkError('Pont HTTPS local indisponible ou trop lent.');
            }
            if (error instanceof TypeError) {
                throw new MobileNetworkError(error.message);
            }
            throw error;
        } finally {
            window.clearTimeout(timeoutId);
        }
        if (!response.ok) {
            let message = text || `Erreur HTTP ${response.status}`;
            try {
                message = JSON.parse(text).error || message;
            } catch {
                // Keep raw body.
            }
            throw new Error(message);
        }
        return text ? JSON.parse(text) as T : undefined as T;
    }

    private async registerMobilePasskey() {
        this.assertPasskeyAvailable();
        const options = await this.requestSecureAuth<WebauthnOptionsResponse<RegistrationChallenge>>('/auth/passkey/register/options', {
            method: 'POST',
            body: JSON.stringify({ deviceLabel: this.mobileDeviceLabel() })
        });
        let credential: PublicKeyCredential | null;
        try {
            credential = await navigator.credentials.create({
                publicKey: this.toCreationOptions(options.publicKey)
            }) as PublicKeyCredential | null;
        } catch (error) {
            throw this.normalizePasskeyError(error, 'register');
        }
        if (!credential) throw new Error('Création de la passkey annulée.');
        const response = credential.response as AuthenticatorAttestationResponse;
        const transports = typeof response.getTransports === 'function' ? response.getTransports() : [];
        const session = await this.requestSecureAuth<SecureSessionResponse>('/auth/passkey/register/verify', {
            method: 'POST',
            body: JSON.stringify({
                challengeId: options.challengeId,
                deviceLabel: this.mobileDeviceLabel(),
                response: {
                    id: credential.id,
                    attestationObject: this.bufferToBase64Url(response.attestationObject),
                    clientDataJSON: this.bufferToBase64Url(response.clientDataJSON),
                    transports,
                }
            })
        });
        setMobileCsrfToken(session.csrfToken);
        markMobilePasskeyReady();
    }

    private async loginWithMobilePasskey() {
        this.assertPasskeyAvailable();
        const options = await this.requestSecureAuth<WebauthnOptionsResponse<AuthenticationChallenge>>('/auth/passkey/login/options', {
            method: 'POST',
            body: JSON.stringify({})
        });
        let credential: PublicKeyCredential | null;
        try {
            credential = await navigator.credentials.get({
                publicKey: this.toRequestOptions(options.publicKey)
            }) as PublicKeyCredential | null;
        } catch (error) {
            throw this.normalizePasskeyError(error, 'login');
        }
        if (!credential) throw new Error('Authentification passkey annulée.');
        const response = credential.response as AuthenticatorAssertionResponse;
        const session = await this.requestSecureAuth<SecureSessionResponse>('/auth/passkey/login/verify', {
            method: 'POST',
            body: JSON.stringify({
                challengeId: options.challengeId,
                deviceLabel: this.mobileDeviceLabel(),
                response: {
                    id: credential.id,
                    authenticatorData: this.bufferToBase64Url(response.authenticatorData),
                    signature: this.bufferToBase64Url(response.signature),
                    clientDataJSON: this.bufferToBase64Url(response.clientDataJSON),
                    userHandle: response.userHandle ? this.bufferToBase64Url(response.userHandle) : undefined,
                }
            })
        });
        setMobileCsrfToken(session.csrfToken);
        markMobilePasskeyReady();
    }

    private assertPasskeyAvailable() {
        if (!window.isSecureContext) {
            throw new Error('La clé d’accès nécessite HTTPS. Ouvre la PWA depuis dmxmoney.develop-max.com.');
        }
        if (!window.PublicKeyCredential || !navigator.credentials) {
            throw new Error('Passkey indisponible dans ce navigateur.');
        }
    }

    private normalizePasskeyError(error: unknown, mode: 'register' | 'login') {
        const name = error instanceof DOMException ? error.name : '';
        const message = error instanceof Error ? error.message : String(error);
        const lowerMessage = message.toLowerCase();

        if (name === 'InvalidStateError' || lowerMessage.includes('invalid state')) {
            const details = mode === 'register'
                ? 'Une clé d’accès existe déjà pour cette PWA ou Safari a gardé un état précédent. Je tente une reconnexion avec cette clé.'
                : 'La clé d’accès enregistrée n’est pas utilisable dans cette PWA. Déconnecte cette PWA, puis scanne un nouveau QR depuis l’application desktop.';
            return mode === 'register'
                ? new MobilePasskeyInvalidStateError(details)
                : new Error(details);
        }

        if (name === 'NotAllowedError') {
            return new Error('Opération Passkey annulée, refusée ou expirée.');
        }

        if (name === 'SecurityError') {
            return new Error('Passkey refusée par le navigateur. Ouvre la PWA depuis le QR code de l’application desktop, en HTTPS.');
        }

        return error instanceof Error ? error : new Error(message);
    }

    private toCreationOptions(challenge: RegistrationChallenge): PublicKeyCredentialCreationOptions {
        return {
            ...challenge,
            challenge: this.base64UrlToBuffer(challenge.challenge),
            user: {
                ...challenge.user,
                id: this.base64UrlToBuffer(challenge.user.id),
            },
            excludeCredentials: challenge.excludeCredentials?.map(credential => ({
                ...credential,
                id: this.base64UrlToBuffer(credential.id),
            })),
        };
    }

    private toRequestOptions(challenge: AuthenticationChallenge): PublicKeyCredentialRequestOptions {
        return {
            ...challenge,
            challenge: this.base64UrlToBuffer(challenge.challenge),
            allowCredentials: challenge.allowCredentials?.map(credential => ({
                ...credential,
                id: this.base64UrlToBuffer(credential.id),
            })),
        };
    }

    private base64UrlToBuffer(value: string): ArrayBuffer {
        const padded = value.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(value.length / 4) * 4, '=');
        const binary = atob(padded);
        const bytes = new Uint8Array(binary.length);
        for (let index = 0; index < binary.length; index += 1) {
            bytes[index] = binary.charCodeAt(index);
        }
        return bytes.buffer;
    }

    private bufferToBase64Url(value: ArrayBuffer): string {
        const bytes = new Uint8Array(value);
        let binary = '';
        bytes.forEach(byte => {
            binary += String.fromCharCode(byte);
        });
        return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
    }

    private mobileDeviceLabel() {
        const userAgent = navigator.userAgent;
        const platform = navigator.platform || '';
        let device = 'Mobile';
        if (/iPhone/i.test(userAgent)) device = 'iPhone';
        else if (/iPad/i.test(userAgent) || (platform === 'MacIntel' && navigator.maxTouchPoints > 1)) device = 'iPad';
        else if (/Android/i.test(userAgent)) device = 'Android';

        let browser = 'Navigateur';
        if (/CriOS|Chrome/i.test(userAgent) && !/Edg/i.test(userAgent)) browser = 'Chrome';
        else if (/FxiOS|Firefox/i.test(userAgent)) browser = 'Firefox';
        else if (/EdgiOS|Edg/i.test(userAgent)) browser = 'Edge';
        else if (/Safari/i.test(userAgent)) browser = 'Safari';

        const mode = isStandalonePwa() ? 'PWA' : 'Web';
        return `${device} - ${browser} ${mode}`;
    }

    private isMobileNetworkError(error: unknown) {
        return error instanceof MobileNetworkError;
    }

    isOfflineError(error: unknown) {
        return this.isMobileNetworkError(error);
    }

    private async getMobileData<K extends OfflineDataKey>(
        key: K,
        path: string,
    ): Promise<Awaited<ReturnType<typeof offlineStore.getData<K>>>> {
        try {
            const data = await this.request<Awaited<ReturnType<typeof offlineStore.getData<K>>>>(path);
            if (data !== null) {
                await offlineStore.setData(key, data as never);
            }
            return data;
        } catch (error) {
            if (!this.isMobileNetworkError(error)) throw error;

            const cached = await offlineStore.getData(key);
            if (cached !== null) return cached as Awaited<ReturnType<typeof offlineStore.getData<K>>>;
            throw error;
        }
    }

    /**
     * Envoie une demande en langage naturel au pont du Mac.
     *
     * Le texte part tel quel : l'application de bureau le fait normaliser par son modèle local
     * (Apple Intelligence sur macOS 26) quand elle en a un, puis `dmx-core` l'interprète et
     * produit la réponse. Sans modèle, la même analyse déterministe s'applique.
     */
    async askAssistant(text: string, apply = true): Promise<AssistantAnswer> {
        if (!this.usesHttp()) {
            throw new Error("L'assistant a besoin de la connexion au Mac.");
        }
        // Le modèle local peut prendre quelques secondes : délai plus large que les autres appels.
        return this.request<AssistantAnswer>('/api/assistant', {
            method: 'POST',
            body: JSON.stringify({ text, apply }),
        }, 20000);
    }

    private async commitMobileMutation(
        path: string,
        method: string,
        body: string | undefined,
        applyLocalChange: () => Promise<void>,
    ) {
        await applyLocalChange();
        await offlineStore.enqueueMutation(path, method, body);
        this.flushPendingMobileMutations().catch(error => {
            if (!this.isMobileNetworkError(error)) {
                console.warn('Mobile offline sync failed:', error);
            }
        });
    }

    private async flushPendingMobileMutations() {
        if (!this.usesHttp()) return;
        if (this.flushPromise) return this.flushPromise;

        this.flushPromise = (async () => {
            if (getMobileCsrfToken()) await this.adoptPendingOfflineWork();
            const mutations = await offlineStore.listMutations();
            for (let index = 0; index < mutations.length;) {
                const mutation = mutations[index];
                if (mutation.path === '/api/settings' && mutation.method === 'PATCH') {
                    const ids: string[] = [];
                    let merged: SettingsMutation | null = null;
                    let refreshSettings = false;

                    while (index < mutations.length) {
                        const candidate = mutations[index];
                        if (candidate.path !== '/api/settings' || candidate.method !== 'PATCH') break;
                        const parsed = parseQueuedSettingsMutation(candidate.body);
                        if (parsed) {
                            merged = merged ? mergeSettingsMutations(merged, parsed) : parsed;
                        }
                        ids.push(candidate.id);
                        index += 1;
                    }

                    if (merged && hasSettingsMutationChanges(merged)) {
                        const mutationToSend = {
                            ...merged,
                            baseRevision: merged.baseRevision,
                        };
                        await this.request<SettingsPatchResult>('/api/settings', {
                            method: 'PATCH',
                            body: JSON.stringify(serializeSettingsMutation(mutationToSend)),
                        }, this.requestTimeoutMs);
                        refreshSettings = true;
                    }
                    for (const id of ids) await offlineStore.removeMutation(id);
                    if (refreshSettings) {
                        window.dispatchEvent(new CustomEvent('dmxmoney-settings-refresh'));
                    }
                    continue;
                }

                await this.request(mutation.path, {
                    method: mutation.method,
                    body: mutation.body,
                }, this.requestTimeoutMs);
                await offlineStore.removeMutation(mutation.id);
                index += 1;
            }
        })().finally(() => {
            this.flushPromise = null;
        });

        return this.flushPromise;
    }

    // Accounts
    async getAccounts(): Promise<Account[]> {
        if (this.usesHttp()) return (await this.getMobileData('accounts', '/api/accounts')) || [];
        return this.invoke<Account[]>('get_accounts');
    }

    async addAccount(account: Account): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(account);
            await this.commitMobileMutation('/api/accounts', 'POST', body, () =>
                offlineStore.updateCollection<Account>('accounts', items => [...items, account])
            );
            return;
        }
        await this.invoke('add_account', { account });
    }

    async updateAccount(account: Account): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(account);
            await this.commitMobileMutation('/api/accounts', 'PUT', body, () =>
                offlineStore.updateCollection<Account>('accounts', items => items.map(item => item.id === account.id ? account : item))
            );
            return;
        }
        await this.invoke('update_account', { account });
    }

    async deleteAccount(id: string): Promise<void> {
        if (this.usesHttp()) {
            await this.commitMobileMutation(`/api/accounts/${encodeURIComponent(id)}`, 'DELETE', undefined, async () => {
                await offlineStore.updateCollection<Account>('accounts', items => items.filter(item => item.id !== id));
                await offlineStore.updateCollection<Transaction>('transactions', items => items.filter(item => item.accountId !== id));
                await offlineStore.updateCollection<ScheduledTransaction>('scheduled', items => items.filter(item => item.accountId !== id));
                await offlineStore.updateCollection<Budget>('budgets', items => items.map(item => item.accountId === id ? { ...item, accountId: undefined } : item));
            });
            return;
        }
        await this.invoke('delete_account', { id });
    }

    // Transactions
    async getTransactions(): Promise<Transaction[]> {
        if (this.usesHttp()) return (await this.getMobileData('transactions', '/api/transactions')) || [];
        return this.invoke<Transaction[]>('get_transactions');
    }

    async addTransaction(transaction: Transaction): Promise<string> {
        if (this.usesHttp()) {
            const body = JSON.stringify(transaction);
            await this.commitMobileMutation('/api/transactions', 'POST', body, () =>
                offlineStore.updateCollection<Transaction>('transactions', items => [transaction, ...items])
            );
            return transaction.id;
        }
        await this.invoke('add_transaction', { transaction });
        return transaction.id;
    }

    async addTransfer(fromTransaction: Transaction, toTransaction: Transaction): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify({ fromTransaction, toTransaction });
            await this.commitMobileMutation('/api/transfers', 'POST', body, () =>
                offlineStore.updateCollection<Transaction>('transactions', items => [fromTransaction, toTransaction, ...items])
            );
            return;
        }

        await Promise.all([
            this.addTransaction(fromTransaction),
            this.addTransaction(toTransaction)
        ]);
    }

    async updateTransaction(transaction: Transaction): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(transaction);
            await this.commitMobileMutation('/api/transactions', 'PUT', body, () =>
                offlineStore.updateCollection<Transaction>('transactions', items => applyTransactionUpdate(items, transaction))
            );
            return;
        }
        await this.invoke('update_transaction', { transaction });
    }

    async deleteTransaction(id: string): Promise<void> {
        if (this.usesHttp()) {
            await this.commitMobileMutation(`/api/transactions/${encodeURIComponent(id)}`, 'DELETE', undefined, async () => {
                const transactions = await offlineStore.getData('transactions') || [];
                const transaction = transactions.find(item => item.id === id);
                const idsToRemove = new Set([id]);
                if (transaction?.linkedTransactionId) idsToRemove.add(transaction.linkedTransactionId);
                await offlineStore.setData('transactions', transactions.filter(item => !idsToRemove.has(item.id)));
            });
            return;
        }
        await this.invoke('delete_transaction', { id });
    }

    // Categories
    async getCategories(): Promise<Category[]> {
        if (this.usesHttp()) return (await this.getMobileData('categories', '/api/categories')) || [];
        return this.invoke<Category[]>('get_categories');
    }

    async addCategory(category: Category): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(category);
            await this.commitMobileMutation('/api/categories', 'POST', body, () =>
                offlineStore.updateCollection<Category>('categories', items => [...items, category])
            );
            return;
        }
        await this.invoke('add_category', { category });
    }

    async updateCategory(category: Category): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(category);
            await this.commitMobileMutation('/api/categories', 'PUT', body, () =>
                offlineStore.updateCollection<Category>('categories', items => items.map(item => item.id === category.id ? category : item))
            );
            return;
        }
        await this.invoke('update_category', { category });
    }

    async deleteCategory(id: string): Promise<void> {
        if (this.usesHttp()) {
            await this.commitMobileMutation(`/api/categories/${encodeURIComponent(id)}`, 'DELETE', undefined, () =>
                offlineStore.updateCollection<Category>('categories', items => items.filter(item => item.id !== id))
            );
            return;
        }
        await this.invoke('delete_category', { id });
    }

    // Budgets
    async getBudgets(): Promise<Budget[]> {
        if (this.usesHttp()) return (await this.getMobileData('budgets', '/api/budgets')) || [];
        return this.invoke<Budget[]>('get_budgets');
    }

    async addBudget(budget: Budget): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(budget);
            await this.commitMobileMutation('/api/budgets', 'POST', body, () =>
                offlineStore.updateCollection<Budget>('budgets', items => [budget, ...items])
            );
            return;
        }
        await this.invoke('add_budget', { budget });
    }

    async updateBudget(budget: Budget): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(budget);
            await this.commitMobileMutation('/api/budgets', 'PUT', body, () =>
                offlineStore.updateCollection<Budget>('budgets', items => items.map(item => item.id === budget.id ? budget : item))
            );
            return;
        }
        await this.invoke('update_budget', { budget });
    }

    async deleteBudget(id: string): Promise<void> {
        if (this.usesHttp()) {
            await this.commitMobileMutation(`/api/budgets/${encodeURIComponent(id)}`, 'DELETE', undefined, async () => {
                await offlineStore.updateCollection<Budget>('budgets', items => items.filter(item => item.id !== id));
                await offlineStore.updateCollection<ScheduledTransaction>('scheduled', items => items.map(item => item.budgetId === id ? { ...item, budgetId: undefined, includeInForecast: false } : item));
            });
            return;
        }
        await this.invoke('delete_budget', { id });
    }

    // Scheduled
    async getScheduled(): Promise<ScheduledTransaction[]> {
        if (this.usesHttp()) return (await this.getMobileData('scheduled', '/api/scheduled')) || [];
        return this.invoke<ScheduledTransaction[]>('get_scheduled');
    }

    async addScheduled(scheduled: ScheduledTransaction): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(scheduled);
            await this.commitMobileMutation('/api/scheduled', 'POST', body, () =>
                offlineStore.updateCollection<ScheduledTransaction>('scheduled', items => [...items, scheduled])
            );
            return;
        }
        await this.invoke('add_scheduled', { scheduled });
    }

    async updateScheduled(scheduled: ScheduledTransaction): Promise<void> {
        if (this.usesHttp()) {
            const body = JSON.stringify(scheduled);
            await this.commitMobileMutation('/api/scheduled', 'PUT', body, () =>
                offlineStore.updateCollection<ScheduledTransaction>('scheduled', items => items.map(item => item.id === scheduled.id ? scheduled : item))
            );
            return;
        }
        await this.invoke('update_scheduled', { scheduled });
    }

    async deleteScheduled(id: string): Promise<void> {
        if (this.usesHttp()) {
            await this.commitMobileMutation(`/api/scheduled/${encodeURIComponent(id)}`, 'DELETE', undefined, () =>
                offlineStore.updateCollection<ScheduledTransaction>('scheduled', items => items.filter(item => item.id !== id))
            );
            return;
        }
        await this.invoke('delete_scheduled', { id });
    }

    async processDueScheduled(): Promise<number> {
        if (!this.usesHttp()) return 0;
        const result = await this.request<{ processed: number }>('/api/scheduled/process-due', { method: 'POST' });
        return result.processed;
    }

    // Settings
    async getSettings(): Promise<Settings | null> {
        let cachedSettings: Settings | null = null;
        if (this.usesHttp()) {
            try {
                cachedSettings = parseSettings(
                    await offlineStore.getData('settings') as RawSettings | null
                );
            } catch {
                // A broken local cache must not block a fresh server read.
            }
        }

        try {
            const res = this.usesHttp()
                ? await this.getMobileData('settings', '/api/settings') as RawSettings | null
                : await this.invoke<RawSettings | null>('get_settings');
            const parsed = parseSettings(res);
            if (!parsed) return cachedSettings;
            const merged: Settings = {
                ...parsed,
                lastSeenVersion: selectNewestVersion(
                    cachedSettings?.lastSeenVersion,
                    parsed.lastSeenVersion
                )
            };
            const pendingMutations = this.usesHttp()
                ? await offlineStore.listMutations()
                : [];
            const pendingSettingsMutations = pendingMutations
                .filter(mutation => mutation.path === '/api/settings')
                .map(mutation => (
                    mutation.method === 'PATCH'
                        ? parseQueuedSettingsMutation(mutation.body)
                        : mutation.method === 'PUT'
                            ? legacySettingsToMutation(mutation.body)
                            : null
                ))
                .filter((mutation): mutation is SettingsMutation => mutation !== null);
            const withPendingChanges = pendingMutations.reduce<Settings>((current, mutation) => {
                if (mutation.path !== '/api/settings') return current;
                const pending = mutation.method === 'PATCH'
                    ? parseQueuedSettingsMutation(mutation.body)
                    : mutation.method === 'PUT'
                        ? legacySettingsToMutation(mutation.body)
                        : null;
                return pending ? applySettingsMutation(current, pending) : current;
            }, merged);
            if (pendingSettingsMutations.length > 0) {
                withPendingChanges.settingsRevision = Math.min(
                    parsed.settingsRevision || 0,
                    ...pendingSettingsMutations.map(mutation => mutation.baseRevision),
                );
            }
            if (this.usesHttp()) {
                await offlineStore.setData('settings', withPendingChanges);
            }
            return withPendingChanges;
        } catch {
            return cachedSettings;
        }
    }

    async saveSettings(settings: Settings): Promise<void> {
        const current = await this.getSettings();
        if (!current) {
            const settingsToSend = serializeSettings(settings);
            await this.invoke('save_settings', { settings: settingsToSend });
            return;
        }

        const changedKeys = (Object.keys(settings) as Array<keyof Settings>)
            .filter(key => key !== 'settingsRevision' && JSON.stringify(current[key]) !== JSON.stringify(settings[key]));
        const mutation = createSettingsMutation(current, settings, changedKeys);
        await this.patchSettings(mutation, settings);
    }

    async patchSettings(mutation: SettingsMutation, localSettings: Settings): Promise<void> {
        if (!hasSettingsMutationChanges(mutation)) return;

        if (this.usesHttp()) {
            const serialized = serializeSettingsMutation(mutation);
            await this.commitMobileMutation(
                '/api/settings',
                'PATCH',
                JSON.stringify(serialized),
                () => offlineStore.setData('settings', localSettings),
            );
            return;
        }

        const mutationToSend = {
            ...mutation,
            baseRevision: mutation.baseRevision,
        };
        const result = await this.invoke<SettingsPatchResult>('patch_settings', {
            patch: serializeSettingsMutation(mutationToSend),
        });
        if (result.conflicts.length === 0) {
            localSettings.settingsRevision = result.revision;
        } else {
            window.setTimeout(() => {
                window.dispatchEvent(new CustomEvent('dmxmoney-settings-refresh'));
            }, 0);
        }
    }

    async getSyncStatus(): Promise<SyncStatus> {
        if (!this.usesHttp()) return { ok: true, dataVersion: 0 };
        if (getMobileCsrfToken()) {
            await this.flushPendingMobileMutations();
        }
        return this.request<SyncStatus>('/api/status', {}, this.statusTimeoutMs);
    }

    // Data Management
    async exportData(): Promise<unknown> {
        const [accounts, transactions, categories, scheduled, budgets, settings] = await Promise.all([
            this.getAccounts(),
            this.getTransactions(),
            this.getCategories(),
            this.getScheduled(),
            this.getBudgets(),
            this.getSettings()
        ]);

        return {
            version: 1,
            timestamp: new Date().toISOString(),
            data: { accounts, transactions, categories, scheduled, budgets, settings }
        };
    }

    async importData(backupData: unknown): Promise<void> {
        if (this.usesHttp()) {
            throw new Error('Import indisponible en mode compagnon mobile.');
        }

        const payload = backupData as { data?: Record<string, unknown> };
        if (!payload || !payload.data) {
            throw new Error('Invalid backup data format');
        }

        const importPayload = {
            accounts: payload.data.accounts || [],
            transactions: payload.data.transactions || [],
            categories: payload.data.categories || [],
            scheduled: payload.data.scheduled || [],
            budgets: payload.data.budgets || []
        };

        await this.invoke('import_data', { data: importPayload });

        if (payload.data.settings) {
            const currentSettings = await this.getSettings() || {} as Settings;
            const importedSettings = payload.data.settings as Partial<Settings>;

            const newSettings: Settings = {
                ...currentSettings,
                accountGroups: importedSettings.accountGroups || currentSettings.accountGroups,
                customGroups: importedSettings.customGroups || currentSettings.customGroups,
                customGroupsOrder: importedSettings.customGroupsOrder || currentSettings.customGroupsOrder,
                accountsOrder: importedSettings.accountsOrder || currentSettings.accountsOrder
            };

            await this.saveSettings(newSettings);
        }
    }

    async mergeData(backupData: unknown): Promise<void> {
        if (this.usesHttp()) {
            throw new Error('Import indisponible en mode compagnon mobile.');
        }

        const payload = backupData as { data?: Record<string, unknown[]> };
        if (!payload || !payload.data) {
            throw new Error('Invalid backup data format');
        }

        const [currentAccounts, currentTransactions, currentCategories, currentScheduled, currentBudgets] = await Promise.all([
            this.getAccounts(),
            this.getTransactions(),
            this.getCategories(),
            this.getScheduled(),
            this.getBudgets()
        ]);

        const mergeArrays = <T extends { id: string }>(current: T[], incoming: T[] = []) => {
            const map = new Map(current.map(item => [item.id, item]));
            incoming.forEach(item => {
                map.set(item.id, item);
            });
            return Array.from(map.values());
        };

        const importPayload = {
            accounts: mergeArrays(currentAccounts, (payload.data.accounts || []) as Account[]),
            transactions: mergeArrays(currentTransactions, (payload.data.transactions || []) as Transaction[]),
            categories: mergeArrays(currentCategories, (payload.data.categories || []) as Category[]),
            scheduled: mergeArrays(currentScheduled, (payload.data.scheduled || []) as ScheduledTransaction[]),
            budgets: mergeArrays(currentBudgets, (payload.data.budgets || []) as Budget[])
        };

        await this.invoke('import_data', { data: importPayload });
    }
}

export const dbService = new DatabaseService();
