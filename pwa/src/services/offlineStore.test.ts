import { beforeEach, afterEach, expect, test, spyOn, mock } from 'bun:test';
import { indexedDB, IDBObjectStore } from 'fake-indexeddb';
import { offlineStore } from './offlineStore';
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
