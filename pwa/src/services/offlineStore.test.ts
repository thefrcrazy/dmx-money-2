import { beforeEach, afterEach, expect, test, spyOn, mock } from 'bun:test';
import { indexedDB, IDBObjectStore, IDBCursor, IDBIndex } from 'fake-indexeddb';
import { OfflineStore, offlineStore } from './offlineStore';
import { decodeOfflinePayload, encodeOfflinePayload } from './offlinePayload';
import { getMobileApiBaseUrl } from '../utils/runtime';
import type { Settings } from '../types';
const oldWindow = globalThis.window;
const oldStorage = globalThis.localStorage;
const oldIndexedDB = globalThis.indexedDB;
beforeEach(() => {
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
    const scope = `https://fixture.invalid/relay/${crypto.randomUUID()}`;
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => scope } });
});
afterEach(() => {
    mock.restore();
    Object.defineProperty(globalThis, 'window', { configurable: true, value: oldWindow });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: oldStorage });
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: oldIndexedDB });
});
test('concurrent offline writes retain every record and every outgoing message', async () => {
    await offlineStore.setData('accounts', []);
    await Promise.all(Array.from({ length: 20 }, (_, i) => offlineStore.commitBankMutation('/api/accounts', 'POST', JSON.stringify({ id: String(i) }))));
    expect(await offlineStore.getData('accounts')).toHaveLength(20);
    const pending = await offlineStore.listMutations();
    expect(pending).toHaveLength(20);
    expect(new Set(pending.map(item => item.sequence)).size).toBe(20);
});
test('a failed queue write rolls back the corresponding visible edit', async () => {
    await offlineStore.setData('accounts', []);
    const put = IDBObjectStore.prototype.put;
    spyOn(IDBObjectStore.prototype, 'put').mockImplementation(function(value: unknown, key?: IDBValidKey) {
        if (this.name === 'mutations') throw new DOMException('Quota', 'QuotaExceededError');
        return put.call(this, value, key);
    });
    await expect(offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"lost"}')).rejects.toThrow('Quota');
    expect(await offlineStore.getData('accounts')).toEqual([]);
    expect(await offlineStore.listMutations()).toEqual([]);
});
test('an older GET cannot overwrite an offline write even after its acknowledgement', async () => {
    await offlineStore.setData('accounts', []);
    const revision = await offlineStore.getRevision('accounts');
    await offlineStore.commitBankMutation('/api/accounts', 'POST', '{"id":"local"}');
    for (const mutation of await offlineStore.listMutations()) await offlineStore.removeMutation(mutation.id);
    expect(await offlineStore.acceptRemoteData('accounts', [], revision)).toEqual([{ id: 'local' }] as never);
    expect(await offlineStore.getData('accounts')).toEqual([{ id: 'local' }] as never);
});
test('successive offline edits preserve the right base for each queued patch', async () => {
    await offlineStore.setData('budgets', [{ id: 'b', name: 'Courses', amount: 10, category: '5' }]);
    await offlineStore.commitBankMutation('/api/budgets', 'PUT', '{"id":"b","name":"Courses","amount":20,"category":"5"}');
    await offlineStore.commitBankMutation('/api/budgets', 'PUT', '{"id":"b","name":"Courses","amount":30,"category":"5"}');
    const pending = await offlineStore.listMutations();
    expect(pending.map(item => JSON.parse(item.body!)._base.amount)).toEqual([10, 20]);
});
test('editing a budget leaves unrelated cached collections untouched', async () => {
    await offlineStore.setData('accounts', []);
    const revision = await offlineStore.getRevision('accounts');
    await offlineStore.commitBankMutation('/api/budgets', 'POST', '{"id":"budget","amount":10}');
    expect(await offlineStore.getRevision('accounts')).toBe(revision);
    expect(await offlineStore.getData('transactions')).toBeNull();
});

const requestResult = <T>(request: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
});
const completed = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = transaction.onabort = () => reject(transaction.error);
});
async function seedLegacy(name: string, data: unknown[], mutations: unknown[]) {
    const opening = indexedDB.open(name, 1);
    opening.onupgradeneeded = () => {
        opening.result.createObjectStore('data', { keyPath: 'key' });
        opening.result.createObjectStore('mutations', { keyPath: 'id' });
    };
    const db = await requestResult(opening);
    const transaction = db.transaction(['data', 'mutations'], 'readwrite');
    const done = completed(transaction);
    for (const record of data) transaction.objectStore('data').put(record);
    for (const mutation of mutations) transaction.objectStore('mutations').put(mutation);
    await done;
    db.close();
}
async function rawDatabase(name: string) {
    const db = await requestResult(indexedDB.open(name));
    const transaction = db.transaction(['data', 'mutations']);
    const [data, mutations] = await Promise.all([
        requestResult(transaction.objectStore('data').getAll()), requestResult(transaction.objectStore('mutations').getAll()),
    ]);
    const version = db.version;
    db.close();
    return { version, data, mutations };
}
const databaseName = () => `dmx-offline-test-${crypto.randomUUID()}`;
const fixtureSettings: Settings = {
    theme: 'light', primaryColor: 'default', componentSpacing: 6, componentPadding: 6,
    windowPosition: null, windowSize: null, settingsRevision: 1,
};
const emptyRemote = () => ({ accounts: [], transactions: [], categories: [], budgets: [], scheduled: [], settings: fixtureSettings });

test('settings and their outbox share a single snapshot during a concurrent local change', async () => {
    const store = new OfflineStore(databaseName());
    await store.setData('settings', fixtureSettings);
    const get = IDBObjectStore.prototype.get;
    const getAll = IDBIndex.prototype.getAll;
    let settingsTransaction: IDBTransaction | undefined;
    let outboxTransaction: IDBTransaction | undefined;
    spyOn(IDBObjectStore.prototype, 'get').mockImplementation(function(key: IDBValidKey) {
        if (this.name === 'data' && String(key).endsWith(':settings')) settingsTransaction = this.transaction;
        return get.call(this, key);
    });
    spyOn(IDBIndex.prototype, 'getAll').mockImplementation(function(...args: Parameters<typeof getAll>) {
        if (this.objectStore.name === 'mutations' && this.objectStore.transaction.mode === 'readonly') outboxTransaction = this.objectStore.transaction;
        return getAll.apply(this, args);
    });
    const reading = store.getSettingsSnapshot();
    const writing = store.commitBankMutation('/api/settings', 'PATCH', JSON.stringify({
        schemaVersion: 2, baseRevision: 1, values: { theme: 'dark' }, expectedValues: { theme: 'light' },
    }), { ...fixtureSettings, theme: 'dark' });
    const snapshot = await reading;
    // The writer's later requests must not obscure the transactions used by the snapshot.
    expect(settingsTransaction).toBeDefined();
    expect(outboxTransaction).toBe(settingsTransaction);
    expect(snapshot.settings?.theme).toBe(snapshot.mutations.length ? 'dark' : 'light');
    await writing;
    const latest = await store.getSettingsSnapshot();
    expect(latest.settings?.theme).toBe('dark');
    expect(latest.mutations).toHaveLength(1);
    expect(JSON.parse(latest.mutations[0].body!).values.theme).toBe('dark');
});

test('explicit recovery reads raw IDs and replaces an unreadable queue only with the selected snapshot', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    await seedLegacy(name, [{ key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [{ id: 'saved' }], updatedAt: 1 }], [
        { id: 'broken', scope, path: '/api/accounts', method: 'POST', body: '{invalid', createdAt: 1 },
        { id: 'another-desktop', scope: 'https://other.invalid/relay/device', path: '/api/accounts', method: 'POST', body: '{invalid', createdAt: 2 },
    ]);
    const store = new OfflineStore(name);
    await expect(store.listMutations()).rejects.toThrow('illisibles');
    const ids = await store.listMutationIds();
    expect(ids).toEqual(['broken']);
    await store.replaceWithRemoteSnapshot(emptyRemote(), ids);
    expect(await store.getData('accounts')).toEqual([]);
    expect(await store.getData('settings')).toEqual(fixtureSettings);
    expect(await store.listMutations()).toEqual([]);
    expect((await rawDatabase(name)).mutations.map(item => item.id)).toEqual(['another-desktop']);
});

for (const body of ['null', '[]', '"text"', '42', 'true']) {
    test(`non-object queued JSON ${body} remains recoverable without replay or data loss`, async () => {
        const name = databaseName();
        const scope = getMobileApiBaseUrl()!;
        await seedLegacy(name, [{ key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [{ id: 'saved' }], updatedAt: 1 }], [
            { id: 'invalid-object', scope, path: '/api/transfers', method: 'PATCH', body, createdAt: 1 },
        ]);
        const store = new OfflineStore(name);
        await expect(store.listMutations()).rejects.toThrow('illisibles');
        const raw = await rawDatabase(name);
        expect(decodeOfflinePayload(raw.mutations[0].payload)).toEqual({ body });
        expect(await store.listMutationIds()).toEqual(['invalid-object']);
        await expect(store.commitBankMutation('/api/accounts', 'POST', '{"id":"new"}')).rejects.toThrow('illisibles');
        expect(await rawDatabase(name)).toEqual(raw);
        await store.replaceWithRemoteSnapshot(emptyRemote(), await store.listMutationIds());
        expect(await store.listMutations()).toEqual([]);
    });
}

test('recovery quota failure rolls back all collections, transaction rows and the corrupt queue', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    await seedLegacy(name, [{ key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [{ id: 'saved' }], updatedAt: 1 }], [
        { id: 'broken', scope, path: '/api/accounts', method: 'POST', body: '{invalid', createdAt: 1 },
    ]);
    const store = new OfflineStore(name);
    await store.setData('transactions', [{ id: 'saved-row', accountId: 'a', date: '2026-10-03', type: 'expense', amount: 10, description: 'Fixture', category: 'c', checked: false }]);
    const before = await rawDatabase(name);
    const put = IDBObjectStore.prototype.put;
    spyOn(IDBObjectStore.prototype, 'put').mockImplementation(function(value: unknown, key?: IDBValidKey) {
        if (this.name === 'data' && (value as { dataKey?: string }).dataKey === 'settings') throw new DOMException('Fixture quota', 'QuotaExceededError');
        return put.call(this, value, key);
    });
    await expect(store.replaceWithRemoteSnapshot(emptyRemote(), await store.listMutationIds())).rejects.toThrow('Fixture quota');
    expect(await rawDatabase(name)).toEqual(before);
    expect((await store.getData('transactions'))?.map(item => item.id)).toEqual(['saved-row']);
    expect(await store.listMutationIds()).toEqual(['broken']);
});

test('version 1 migration hides financial cache and pending bodies while retaining metadata and Unicode', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const account = { id: 'a', name: 'Épargne 🧾 巴黎', initialBalance: 123.45 };
    const record = { key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [account], updatedAt: 123, revision: 'preserved' };
    const mutation = { id: 'queued', scope, path: '/api/accounts', method: 'POST', body: JSON.stringify(account), createdAt: 124, sequence: 7 };
    await seedLegacy(name, [record], [mutation]);
    const store = new OfflineStore(name);
    expect(await store.getData('accounts')).toEqual([account] as never);
    expect(await store.getRevision('accounts')).toBe('preserved');
    expect(await store.listMutations()).toEqual([mutation]);
    const raw = await rawDatabase(name);
    expect(raw.version).toBe(3);
    expect(raw.data[0].value).toBeUndefined();
    expect(raw.mutations[0].body).toBeUndefined();
    expect(raw.data[0]).toMatchObject({ key: record.key, scope, dataKey: 'accounts', updatedAt: 123, revision: 'preserved' });
    expect(JSON.stringify(raw)).not.toContain(account.name);
    expect(raw.mutations[0]).toMatchObject({ id: 'queued', scope, path: '/api/accounts', method: 'POST', createdAt: 124, sequence: 7 });
    expect(decodeOfflinePayload(raw.mutations[0].payload)).toEqual({ body: mutation.body });
});

test('an aborted upgrade leaves all legacy records intact and retries safely', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const account = { id: 'original', name: 'Café 🧾' };
    const record = { key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [account], updatedAt: 123 };
    const mutation = { id: 'queued', scope, path: '/api/accounts', method: 'POST', body: JSON.stringify(account), createdAt: 124 };
    await seedLegacy(name, [record], [mutation]);
    const update = IDBCursor.prototype.update;
    const failing = spyOn(IDBCursor.prototype, 'update').mockImplementation(function(value: unknown) {
        if ((this.source as IDBObjectStore).name === 'mutations') throw new DOMException('Quota', 'QuotaExceededError');
        return update.call(this, value);
    });
    const store = new OfflineStore(name);
    await expect(store.getData('accounts')).rejects.toThrow('Quota');
    failing.mockRestore();
    expect(await rawDatabase(name)).toEqual({ version: 1, data: [record], mutations: [mutation] });
    expect(await store.getData('accounts')).toEqual([account] as never);
    expect((await rawDatabase(name)).version).toBe(3);
});

test('an old tab blocking the upgrade gives a recoverable error and closes stale opens', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const record = { key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [{ id: 'saved' }], updatedAt: 1 };
    await seedLegacy(name, [record], []);
    const blocking = await requestResult(indexedDB.open(name, 1));
    const store = new OfflineStore(name);
    await expect(store.getData('accounts')).rejects.toThrow('Fermez les autres fenêtres');
    blocking.close();
    expect(await store.getData('accounts')).toEqual([{ id: 'saved' }] as never);
    expect((await rawDatabase(name)).version).toBe(3);
    expect(await store.getData('accounts')).toEqual([{ id: 'saved' }] as never);
});

test('writes arriving during upgrade retain every record and every opaque outgoing body', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    await seedLegacy(name, [{ key: `${scope}:accounts`, scope, dataKey: 'accounts', value: [{ id: 'old' }], updatedAt: 1 }], []);
    const store = new OfflineStore(name);
    await Promise.all(Array.from({ length: 20 }, (_, i) => store.commitBankMutation('/api/accounts', 'POST', JSON.stringify({ id: `new-${i}`, name: 'Café 🧾' }))));
    expect(await store.getData('accounts')).toHaveLength(21);
    const queue = await store.listMutations();
    expect(queue).toHaveLength(20);
    expect(new Set(queue.map(item => item.sequence)).size).toBe(20);
    const raw = await rawDatabase(name);
    expect(raw.data.every(record => record.payload && !('value' in record))).toBe(true);
    expect(raw.mutations.every(record => record.payload && !('body' in record))).toBe(true);
});

test('malformed legacy JSON is obfuscated losslessly but blocks replay and scope migration', async () => {
    const name = databaseName();
    const previousScope = getMobileApiBaseUrl()!;
    const broken = { id: 'broken', scope: previousScope, path: '/api/accounts', method: 'POST', body: '{invalid', createdAt: 1 };
    await seedLegacy(name, [{ key: `${previousScope}:accounts`, scope: previousScope, dataKey: 'accounts', value: [], updatedAt: 1 }], [broken]);
    const store = new OfflineStore(name);
    await expect(store.listMutations()).rejects.toThrow('illisibles');
    const original = await rawDatabase(name);
    expect(decodeOfflinePayload(original.mutations[0].payload)).toEqual({ body: broken.body });
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => 'https://next.invalid/relay/device-next' } });
    await expect(store.migrateScope(previousScope)).rejects.toThrow('illisibles');
    expect(await rawDatabase(name)).toEqual(original);
});

test('corrupt queued payloads block writes and remote refresh without deleting or retargeting data', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const originalData = { key: `${scope}:accounts`, scope, dataKey: 'accounts', payload: encodeOfflinePayload([{ id: 'saved' }]), updatedAt: 1, revision: 'old' };
    const broken = { id: 'broken', scope, path: '/api/accounts', method: 'POST', payload: { version: 1, encoding: 'json-utf8-base64', data: 'corrupt!' }, createdAt: 1 };
    await seedLegacy(name, [originalData], [broken]);
    const store = new OfflineStore(name);
    await expect(store.listMutations()).rejects.toThrow('illisibles');
    const original = await rawDatabase(name);
    await expect(store.commitBankMutation('/api/accounts', 'POST', '{"id":"new"}')).rejects.toThrow('illisibles');
    await expect(store.acceptRemoteData('accounts', [{ id: 'remote' }] as never, 'old')).rejects.toThrow('illisibles');
    expect(await rawDatabase(name)).toEqual(original);
});

test('corrupt financial cache is preserved when a refresh or local mutation is attempted', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const record = { key: `${scope}:accounts`, scope, dataKey: 'accounts', payload: { version: 1, encoding: 'json-utf8-base64', data: '/w==' }, updatedAt: 1, revision: 'old' };
    await seedLegacy(name, [record], []);
    const store = new OfflineStore(name);
    await expect(store.getData('accounts')).rejects.toThrow('illisibles');
    await expect(store.acceptRemoteData('accounts', [], 'old')).rejects.toThrow('illisibles');
    await expect(store.commitBankMutation('/api/accounts', 'POST', '{"id":"new"}')).rejects.toThrow('illisibles');
    expect(await rawDatabase(name)).toEqual({ version: 3, data: [record], mutations: [] });
});

test('explicit erase can recover an unreadable legacy cache without silently losing it first', async () => {
    const name = databaseName();
    const scope = getMobileApiBaseUrl()!;
    const record = { key: `${scope}:accounts`, scope, dataKey: 'accounts', value: 'invalid cache', updatedAt: 1 };
    const mutation = { id: 'saved', scope, path: '/api/accounts', method: 'POST', body: '{"id":"saved"}', createdAt: 2 };
    await seedLegacy(name, [record], [mutation]);
    const store = new OfflineStore(name);
    await expect(store.getData('accounts')).rejects.toThrow('illisibles');
    expect(await rawDatabase(name)).toEqual({ version: 1, data: [record], mutations: [mutation] });
    await store.clearAll();
    expect(await rawDatabase(name)).toEqual({ version: 1, data: [], mutations: [] });
    await store.setData('accounts', [{ id: 'new' }] as never);
    expect(await store.getData('accounts')).toEqual([{ id: 'new' }] as never);
    expect((await rawDatabase(name)).version).toBe(3);
});

test('editing one operation in a 30000 row journal writes only that row and its outbox record', async () => {
    const store = new OfflineStore(`rows-${crypto.randomUUID()}`);
    const values = Array.from({ length: 30_000 }, (_, i) => ({ id: `t${i}`, accountId: 'a', date: '2026-10-03', type: 'expense' as const, amount: 10, description: 'Fictif', category: 'c', checked: false }));
    await store.setData('transactions', values);
    const getAll = IDBObjectStore.prototype.getAll;
    const put = IDBObjectStore.prototype.put;
    let fullRowReads = 0;
    let rowWrites = 0;
    spyOn(IDBObjectStore.prototype, 'getAll').mockImplementation(function(...args: Parameters<typeof getAll>) {
        if (this.name === 'transactionRows') fullRowReads++;
        return getAll.apply(this, args);
    });
    spyOn(IDBObjectStore.prototype, 'put').mockImplementation(function(value: unknown, key?: IDBValidKey) {
        if (this.name === 'transactionRows') rowWrites++;
        return put.call(this, value, key);
    });
    await store.commitBankMutation('/api/transactions', 'PUT', JSON.stringify({ ...values[12_345], amount: 25 }));
    expect(fullRowReads).toBe(0);
    expect(rowWrites).toBe(1);
    expect((await store.getData('transactions'))?.find(item => item.id === 't12345')?.amount).toBe(25);
    expect(JSON.parse((await store.listMutations())[0].body!)._base.amount).toBe(10);
});

test('a failed transaction outbox write rolls back both reciprocal transfer rows', async () => {
    const store = new OfflineStore(`pair-${crypto.randomUUID()}`);
    const from = { id: 'from', accountId: 'a', date: '2026-10-03', type: 'expense' as const, amount: 10, description: '', category: 'transfer', checked: false, isTransfer: true, linkedTransactionId: 'to' };
    const to = { ...from, id: 'to', accountId: 'b', type: 'income' as const, linkedTransactionId: 'from' };
    await store.setData('transactions', [from, to]);
    const put = IDBObjectStore.prototype.put;
    spyOn(IDBObjectStore.prototype, 'put').mockImplementation(function(value: unknown, key?: IDBValidKey) {
        if (this.name === 'mutations') throw new DOMException('Quota', 'QuotaExceededError');
        return put.call(this, value, key);
    });
    await expect(store.commitBankMutation('/api/transactions', 'PUT', JSON.stringify({ ...from, amount: 25 }))).rejects.toThrow('Quota');
    expect((await store.getData('transactions'))?.map(item => item.amount)).toEqual([10, 10]);
    expect(await store.listMutations()).toEqual([]);
});

test('atomic transfer edits retain both original form baselines and both account directions', async () => {
    const store = new OfflineStore(`atomic-transfer-${crypto.randomUUID()}`);
    const from = { id: 'from', accountId: 'source', date: '2026-10-03', type: 'expense' as const, amount: 10, description: '', category: 'transfer', checked: false, isTransfer: true, linkedTransactionId: 'to' };
    const to = { ...from, id: 'to', accountId: 'destination', type: 'income' as const, linkedTransactionId: 'from' };
    // A desktop edit arrived while the form was open.
    await store.setData('transactions', [{ ...from, amount: 12 }, { ...to, amount: 12 }]);
    await store.commitBankMutation('/api/transfers', 'PATCH', JSON.stringify({
        fromTransaction: { ...from, accountId: 'source2', description: 'Fictif' },
        toTransaction: { ...to, accountId: 'destination2', description: 'Fictif' },
        _base: { fromTransaction: from, toTransaction: to },
    }));
    const rows = await store.getData('transactions');
    expect(rows?.map(item => [item.type, item.accountId, item.amount])).toEqual([['expense', 'source2', 12], ['income', 'destination2', 12]]);
    const outgoing = JSON.parse((await store.listMutations())[0].body!);
    expect(outgoing._base.fromTransaction.amount).toBe(10);
    expect(outgoing._base.toTransaction.accountId).toBe('destination');
    expect(rows?.some(item => '_base' in item)).toBe(false);
});

test('discarding a changed offline queue is refused and retains financial rows', async () => {
    const store = new OfflineStore(`discard-${crypto.randomUUID()}`);
    await store.setData('accounts', []);
    await store.commitBankMutation('/api/accounts', 'POST', '{"id":"keep"}');
    await expect(store.replaceWithRemoteSnapshot({ accounts: [], transactions: [], categories: [], budgets: [], scheduled: [], settings: null }, [])).rejects.toThrow('changé');
    expect(await store.getData('accounts')).toEqual([{ id: 'keep' }] as never);
    expect(await store.listMutations()).toHaveLength(1);
});
