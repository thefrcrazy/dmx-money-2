import { hasTauriRuntime } from '../utils/runtime';

export interface MobileCompanionStatus {
    enabled: boolean;
    active: boolean;
    host?: string | null;
    port?: number | null;
    url?: string | null;
    dataVersion: number;
    secureBridge?: SecureBridgeStatus | null;
}

export interface MobilePasskeyInfo {
    id: string;
    credentialId: string;
    deviceLabel?: string | null;
    createdAt: string;
    lastUsedAt?: string | null;
    revokedAt?: string | null;
}

export interface SecureBridgeStatus {
    enabled: boolean;
    configured: boolean;
    active: boolean;
    domain?: string | null;
    appUrl?: string | null;
    localHost?: string | null;
    deviceId?: string | null;
    apiUrl?: string | null;
    port?: number | null;
    pairingUrl?: string | null;
    pairingTokenExpiresAt?: string | null;
    certificateExpiresAt?: string | null;
    certificateReady: boolean;
    dnsRecordId?: string | null;
    dnsLastUpdatedAt?: string | null;
    managed: boolean;
    managedServiceUrl: string;
    managedCredentialReady: boolean;
    passkeys: MobilePasskeyInfo[];
    lastError?: string | null;
    /** The bridge still serves the paired mobiles, but managed renewal is failing. */
    degraded?: boolean;
}

const invokeMobileCommand = async <T>(command: string, args?: Record<string, unknown>) => {
    if (!hasTauriRuntime()) {
        throw new Error('Le mode compagnon se configure depuis l’application desktop.');
    }

    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
};

export const mobileCompanionService = {
    getStatus: () => invokeMobileCommand<MobileCompanionStatus>('get_mobile_companion_status'),
    getSecureBridgeStatus: () => invokeMobileCommand<SecureBridgeStatus>('get_secure_bridge_status'),
    setSecureBridgeEnabled: (enabled: boolean) =>
        invokeMobileCommand<MobileCompanionStatus>('set_secure_bridge_enabled', { enabled }),
    regenerateSecurePairingToken: () =>
        invokeMobileCommand<MobileCompanionStatus>('regenerate_secure_pairing_token'),
    revokeMobilePasskey: (passkeyId: string) =>
        invokeMobileCommand<MobileCompanionStatus>('revoke_mobile_passkey', { passkeyId })
};
