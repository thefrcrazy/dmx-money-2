import { expect, test } from 'bun:test';
import { decodeOfflinePayload, encodeOfflinePayload } from './offlinePayload';

test('opaque JSON preserves accented text, emoji, CJK, and byte order marks', () => {
    const value = { label: '\uFEFFCafé 🧾 巴黎', amount: 12.34, nested: [null, true, 'Épargne 💶'] };
    const payload = encodeOfflinePayload(value);
    expect(payload).toMatchObject({ version: 1, encoding: 'json-utf8-base64' });
    expect(payload.data).not.toContain('Café');
    expect(decodeOfflinePayload(payload)).toEqual(value);
    const bytes = new TextEncoder().encode(JSON.stringify(value)).length;
    expect(payload.data.length).toBe(4 * Math.ceil(bytes / 3));
});

test('unknown formats and malformed base64, UTF-8, or JSON are refused without exposing content', () => {
    const valid = encodeOfflinePayload({ label: 'private account' });
    for (const payload of [
        null, [], { ...valid, version: 2 }, { ...valid, encoding: 'encrypted' },
        { ...valid, data: '!!!!' }, { ...valid, data: 'AAAA=' }, { ...valid, data: 'AA=A' },
        { ...valid, data: 'AB==' }, { ...valid, data: '/w==' }, { ...valid, data: btoa('{invalid') },
    ]) {
        expect(() => decodeOfflinePayload(payload)).toThrow('Données hors ligne illisibles');
    }
    expect(() => encodeOfflinePayload(undefined)).toThrow('Données hors ligne illisibles');
});

test('large payloads use bounded chunks and round-trip without a spread stack overflow', () => {
    const value = { description: 'Épargne 🧾 '.repeat(100_000) };
    expect(decodeOfflinePayload(encodeOfflinePayload(value))).toEqual(value);
});

test('fallback browsers preserve the same large Unicode payload without native base64 methods', () => {
    const prototype = Uint8Array.prototype;
    const toBase64 = Object.getOwnPropertyDescriptor(prototype, 'toBase64');
    const fromBase64 = Object.getOwnPropertyDescriptor(Uint8Array, 'fromBase64');
    const value = { label: '\uFEFFCafé 🧾 巴黎'.repeat(100_000) };
    const expected = encodeOfflinePayload(value);
    try {
        Object.defineProperty(prototype, 'toBase64', { configurable: true, value: undefined });
        Object.defineProperty(Uint8Array, 'fromBase64', { configurable: true, value: undefined });
        const payload = encodeOfflinePayload(value);
        expect(payload).toEqual(expected);
        expect(decodeOfflinePayload(payload)).toEqual(value);
    } finally {
        if (toBase64) Object.defineProperty(prototype, 'toBase64', toBase64);
        else Reflect.deleteProperty(prototype, 'toBase64');
        if (fromBase64) Object.defineProperty(Uint8Array, 'fromBase64', fromBase64);
        else Reflect.deleteProperty(Uint8Array, 'fromBase64');
    }
});
