/** Obfuscation only: base64 is reversible and provides no confidentiality. */
export interface OfflinePayload {
    version: 1;
    encoding: 'json-utf8-base64';
    data: string;
}

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const unreadable = () => new Error('Données hors ligne illisibles. Les données sont conservées sur cet appareil.');
type Base64Bytes = Uint8Array & { toBase64?: () => string };
const byteFactory = Uint8Array as typeof Uint8Array & { fromBase64?: (value: string) => Uint8Array };

export function encodeOfflinePayload(value: unknown): OfflinePayload {
    const json = JSON.stringify(value);
    if (json === undefined) throw unreadable();
    const bytes = encoder.encode(json);
    let data: string;
    if (typeof (bytes as Base64Bytes).toBase64 === 'function') {
        data = (bytes as Base64Bytes).toBase64!();
    } else {
        const chunks: string[] = [];
        for (let offset = 0; offset < bytes.length; offset += 8192) {
            chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 8192)));
        }
        data = btoa(chunks.join(''));
    }
    return { version: 1, encoding: 'json-utf8-base64', data };
}

export function decodeOfflinePayload<T>(value: unknown): T {
    try {
        if (!value || typeof value !== 'object' || Array.isArray(value)) throw unreadable();
        const payload = value as Partial<OfflinePayload>;
        if (payload.version !== 1 || payload.encoding !== 'json-utf8-base64' || typeof payload.data !== 'string') throw unreadable();
        const base64 = payload.data;
        const firstPadding = base64.indexOf('=');
        const padding = firstPadding === -1 ? 0 : base64.length - firstPadding;
        if (!base64.length || base64.length % 4 || /[^A-Za-z0-9+/=]/.test(base64)
            || padding > 2 || (padding && base64.slice(firstPadding) !== '='.repeat(padding))
            || (padding === 2 && (alphabet.indexOf(base64[base64.length - 3]) & 15) !== 0)
            || (padding === 1 && (alphabet.indexOf(base64[base64.length - 2]) & 3) !== 0)) throw unreadable();
        let bytes: Uint8Array;
        if (typeof byteFactory.fromBase64 === 'function') bytes = byteFactory.fromBase64(base64);
        else {
            const binary = atob(base64);
            bytes = new Uint8Array(binary.length);
            for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
        }
        return JSON.parse(decoder.decode(bytes)) as T;
    } catch {
        throw unreadable();
    }
}
