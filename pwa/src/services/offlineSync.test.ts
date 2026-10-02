import { afterEach, beforeEach, expect, mock, spyOn, test } from 'bun:test';
import { DatabaseService } from './db';
import { offlineStore } from './offlineStore';

const originalWindow = globalThis.window;
const originalStorage = globalThis.localStorage;
const originalFetch = globalThis.fetch;
let data: Record<string, unknown>;
let queue: { id: string; path: string; method: string; body?: string }[];
beforeEach(() => {
    const storage = new Map([['dmxmoney.secureApiBaseUrl', 'https://desktop.test:8443'], ['dmxmoney.secureCsrfToken', 'test']]);
    Object.defineProperty(globalThis, 'window', { configurable: true, value: { location: { pathname: '/mobile' }, setTimeout, clearTimeout } });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: (key: string) => storage.get(key) ?? null } });
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
afterEach(() => {
    mock.restore();
    globalThis.fetch = originalFetch;
    Object.defineProperty(globalThis, 'window', { configurable: true, value: originalWindow });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: originalStorage });
});
test('cached startup accepts empty collections and never contacts the desktop', async () => {
    globalThis.fetch = mock(() => { throw new Error('Network must not be used'); }) as never;
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
    globalThis.fetch = mock(async () => new Response('unavailable', { status: 503 })) as never;
    const db = new DatabaseService();
    expect(await db.getAccounts()).toEqual([]);
    expect(queue).toHaveLength(1);
    try { await db.getSyncStatus(); throw new Error('Expected offline error'); }
    catch (error) { expect(db.isOfflineError(error)).toBe(true); }
});
test('reconnection acknowledges queued edits before loading fresh data', async () => {
    queue = [{ id: 'queued', path: '/api/accounts/a', method: 'DELETE' }];
    const calls: string[] = [];
    globalThis.fetch = mock(async (input: string, init: RequestInit) => {
        const path = new URL(input).pathname;
        calls.push(`${init.method ?? 'GET'} ${path}`);
        return Response.json(path === '/api/status' ? { ok: true, dataVersion: 2 } : []);
    }) as never;
    const db = new DatabaseService();
    await db.getCachedBankData();
    await db.getSyncStatus();
    await db.getAccounts();
    expect(queue).toHaveLength(0);
    expect(calls).toEqual(['GET /api/status', 'DELETE /api/accounts/a', 'GET /api/status', 'GET /api/accounts']);
});
test('a concurrent local edit is not overwritten by an older GET response', async () => {
    globalThis.fetch = mock(async () => {
        data.accounts = [{ id: 'local' }];
        queue = [{ id: 'queued', path: '/api/accounts', method: 'POST' }];
        return Response.json([{ id: 'server' }]);
    }) as never;
    expect(await new DatabaseService().getAccounts()).toEqual([{ id: 'local' }] as never);
    expect(data.accounts).toEqual([{ id: 'local' }]);
});

test('legacy queued settings JSON fields are decoded and transmitted exactly once', async () => {
    queue = [{ id: 'groups', path: '/api/settings', method: 'PATCH', body: JSON.stringify({
        schemaVersion: 2, baseRevision: 1, values: { accountGroups: JSON.stringify({ a: 'group' }) },
        expectedValues: { accountGroups: JSON.stringify({}) },
    }) }];
    let transmitted: { values: { accountGroups: string }; expectedValues: { accountGroups: string } } | undefined;
    globalThis.fetch = mock(async (input: string, init: RequestInit) => {
        if (new URL(input).pathname === '/api/settings') transmitted = JSON.parse(init.body as string);
        return Response.json({ ok: true, dataVersion: 3, revision: 2, conflicts: [] });
    }) as never;
    Object.assign(window, { dispatchEvent: () => true });
    await new DatabaseService().getSyncStatus();
    expect(transmitted?.values.accountGroups).toBe('{"a":"group"}');
    expect(transmitted?.expectedValues.accountGroups).toBe('{}');
    expect(queue).toHaveLength(0);
});

test('unreadable settings mutations remain in the offline queue', async () => {
    queue = [{ id: 'broken', path: '/api/settings', method: 'PATCH', body: '{invalid' }];
    globalThis.fetch = mock(async () => Response.json({ ok: true, dataVersion: 1 })) as never;
    await expect(new DatabaseService().getSyncStatus()).rejects.toThrow('illisible');
    expect(queue).toHaveLength(1);
});
