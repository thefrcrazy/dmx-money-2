import { beforeEach, afterEach, expect, test, spyOn, mock } from 'bun:test';
import { indexedDB, IDBObjectStore, IDBCursor } from 'fake-indexeddb';
import { OfflineStore, offlineStore } from './offlineStore';
import { decodeOfflinePayload, encodeOfflinePayload } from './offlinePayload';
import { getMobileApiBaseUrl } from '../utils/runtime';
const oldWindow = globalThis.window;
const oldStorage = globalThis.localStorage;
const oldIndexedDB = globalThis.indexedDB;
beforeEach(() => {
    Object.defineProperty(globalThis, 'indexedDB', { configurable: true, value: indexedDB });
    Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
    const scope = `https://fixture-${crypto.randomUUID()}.invalid`;
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
    expect(raw.version).toBe(2);
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
    expect((await rawDatabase(name)).version).toBe(2);
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
    expect((await rawDatabase(name)).version).toBe(2);
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
    Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: { getItem: () => 'https://next.invalid' } });
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
    expect(await rawDatabase(name)).toEqual({ version: 2, data: [record], mutations: [] });
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
    expect((await rawDatabase(name)).version).toBe(2);
});
