import { affectsCollection } from './mutationDependencies';
import { applyBankMutation, bankKeys, type BankSnapshot, type BankKey } from './bankMutations';
import type { Account, Budget, Category, ScheduledTransaction, Settings, Transaction } from '../types';
import { getMobileApiBaseUrl } from '../utils/runtime';
import { decodeOfflinePayload, encodeOfflinePayload, type OfflinePayload } from './offlinePayload';

export type OfflineDataKey = 'accounts' | 'transactions' | 'categories' | 'budgets' | 'scheduled' | 'settings';

/** Decoded application-facing mutation; its body is opaque in IndexedDB. */
export interface OfflineMutation {
    id: string;
    scope: string;
    path: string;
    method: string;
    body?: string;
    createdAt: number;
    sequence?: number;
    failure?: { status: number; message: string; conflicts?: string[] };
}

type OfflineDataMap = {
    accounts: Account[];
    transactions: Transaction[];
    categories: Category[];
    budgets: Budget[];
    scheduled: ScheduledTransaction[];
    settings: Settings | null;
};

interface OfflineDataRecord<K extends OfflineDataKey = OfflineDataKey> {
    key: string;
    scope: string;
    dataKey: K;
    payload?: OfflinePayload;
    /** Version 1 only; accepted on reads and removed by the version 2 upgrade. */
    value?: OfflineDataMap[K];
    updatedAt: number;
    revision?: string;
    rowStore?: boolean;
    nextPosition?: number;
    firstPosition?: number;
}

interface StoredMutation extends Omit<OfflineMutation, 'body'> {
    payload?: OfflinePayload;
    /** Version 1 only. */
    body?: string;
}

const DB_NAME = 'dmxmoney-mobile-offline';
const DB_VERSION = 3;
const DATA_STORE = 'data';
const MUTATION_STORE = 'mutations';
const ROW_STORE = 'transactionRows';

const currentScope = () => getMobileApiBaseUrl() || 'unpaired';
const scopedKey = (key: OfflineDataKey, scope: string) => `${scope}:${key}`;
const unreadable = () => new Error('Données hors ligne illisibles. Les données sont conservées sur cet appareil.');
const blockedOpen = () => new Error('Fermez les autres fenêtres ou onglets DmxMoney puis réessayez. Vos données hors ligne sont conservées.');
const owns = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);

function readData<K extends OfflineDataKey>(record: OfflineDataRecord<K>): OfflineDataMap[K] {
    const value = owns(record, 'payload') ? decodeOfflinePayload<OfflineDataMap[K]>(record.payload) : record.value;
    if (record.dataKey === 'settings') {
        if (value !== null && (!value || typeof value !== 'object' || Array.isArray(value))) throw unreadable();
    } else if (!Array.isArray(value)) throw unreadable();
    return value as OfflineDataMap[K];
}

function readMutation(record: StoredMutation, validateBody = true): OfflineMutation {
    const decoded = owns(record, 'payload') ? decodeOfflinePayload<{ body?: string }>(record.payload) : { body: record.body };
    if (!decoded || typeof decoded !== 'object' || Array.isArray(decoded)
        || (decoded.body !== undefined && typeof decoded.body !== 'string')) throw unreadable();
    if (validateBody && decoded.body !== undefined) {
        try {
            const body: unknown = JSON.parse(decoded.body);
            if (!body || typeof body !== 'object' || Array.isArray(body)) throw unreadable();
        } catch { throw unreadable(); }
    }
    const { payload: _payload, body: _legacyBody, ...metadata } = record;
    return decoded.body === undefined ? metadata : { ...metadata, body: decoded.body };
}

function opaqueData<K extends OfflineDataKey>(record: OfflineDataRecord<K>): OfflineDataRecord<K> {
    const value = readData(record);
    const { value: _legacyValue, ...metadata } = record;
    return { ...metadata, payload: owns(record, 'payload') ? record.payload : encodeOfflinePayload(value) };
}

function opaqueMutation(record: StoredMutation, validateBody = true): StoredMutation {
    const decoded = readMutation(record, validateBody);
    const { body: _body, ...metadata } = decoded;
    return { ...metadata, payload: owns(record, 'payload') ? record.payload : encodeOfflinePayload({ body: decoded.body }) };
}

interface TransactionRow {
    key: string;
    scope: string;
    id: string;
    accountId: string;
    position: number;
    payload: OfflinePayload;
}
const rowKey = (scope: string, id: string) => JSON.stringify([scope, id]);
function readRow(row: TransactionRow): Transaction {
    const value = decodeOfflinePayload<Transaction>(row.payload);
    if (!value || value.id !== row.id || value.accountId !== row.accountId) throw unreadable();
    return value;
}
function rowRecord(scope: string, value: Transaction, position: number): TransactionRow {
    return { key: rowKey(scope, value.id), scope, id: value.id, accountId: value.accountId, position, payload: encodeOfflinePayload(value) };
}
async function collectionValue<K extends OfflineDataKey>(tx: IDBTransaction, record: OfflineDataRecord<K>): Promise<OfflineDataMap[K]> {
    if (!record.rowStore) return readData(record);
    const rows = await requestToPromise<TransactionRow[]>(tx.objectStore(ROW_STORE).index('scope').getAll(record.scope));
    return rows.sort((a, b) => a.position - b.position || a.id.localeCompare(b.id)).map(readRow) as OfflineDataMap[K];
}
async function writeCollection<K extends OfflineDataKey>(tx: IDBTransaction, scope: string, key: K, value: OfflineDataMap[K]) {
    let extra: Partial<OfflineDataRecord>;
    if (key === 'transactions') {
        const rows = tx.objectStore(ROW_STORE);
        const keys = await requestToPromise<IDBValidKey[]>(rows.index('scope').getAllKeys(scope));
        for (const id of keys) rows.delete(id);
        (value as Transaction[]).forEach((item, position) => rows.put(rowRecord(scope, item, position)));
        extra = { rowStore: true, nextPosition: (value as Transaction[]).length, firstPosition: 0 };
    } else extra = { payload: encodeOfflinePayload(value) };
    tx.objectStore(DATA_STORE).put({ key: scopedKey(key, scope), scope, dataKey: key, ...extra, updatedAt: Date.now(), revision: randomId() });
}

const requestToPromise = <T>(request: IDBRequest<T>) => new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error('IndexedDB request failed'));
});

const transactionDone = (transaction: IDBTransaction) => new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error || new Error('IndexedDB transaction failed'));
    transaction.onabort = () => reject(transaction.error || new Error('IndexedDB transaction aborted'));
});

const randomId = () => {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID();
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
};

async function abortTransaction(transaction: IDBTransaction, done: Promise<void>) {
    try { transaction.abort(); } catch { /* It may already have aborted after a failed request. */ }
    await done.catch(() => undefined);
}

export class OfflineStore {
    private dbPromise: Promise<IDBDatabase> | null = null;

    constructor(private readonly databaseName = DB_NAME) {}

    private openDb(): Promise<IDBDatabase> {
        if (this.dbPromise) return this.dbPromise;
        const opening = new Promise<IDBDatabase>((resolve, reject) => {
            const request = indexedDB.open(this.databaseName, DB_VERSION);
            let migrationError: unknown;
            let cancelled = false;
            request.onblocked = () => {
                cancelled = true;
                reject(blockedOpen());
            };
            request.onupgradeneeded = () => {
                if (cancelled) { request.transaction!.abort(); return; }
                const db = request.result;
                if (!db.objectStoreNames.contains(DATA_STORE)) db.createObjectStore(DATA_STORE, { keyPath: 'key' });
                if (!db.objectStoreNames.contains(MUTATION_STORE)) db.createObjectStore(MUTATION_STORE, { keyPath: 'id' });
                const transaction = request.transaction!;
                const rows = db.objectStoreNames.contains(ROW_STORE) ? transaction.objectStore(ROW_STORE) : db.createObjectStore(ROW_STORE, { keyPath: 'key' });
                if (!rows.indexNames.contains('scope')) rows.createIndex('scope', 'scope');
                if (!rows.indexNames.contains('account')) rows.createIndex('account', ['scope', 'accountId']);
                const queue = transaction.objectStore(MUTATION_STORE);
                if (!queue.indexNames.contains('scope')) queue.createIndex('scope', 'scope');
                // All cursor updates belong to the versionchange transaction. The
                // codec is synchronous: no crypto/network await can auto-close it.
                for (const name of [DATA_STORE, MUTATION_STORE]) {
                    const cursorRequest = transaction.objectStore(name).openCursor();
                    cursorRequest.onsuccess = () => {
                        const cursor = cursorRequest.result;
                        if (!cursor) return;
                        try {
                            const record = cursor.value;
                            // Even a previously malformed JSON body can be hidden
                            // losslessly; reading/sending it later still refuses it.
                            if (name === DATA_STORE && record.dataKey === 'transactions' && !record.rowStore) {
                                const values = readData(record) as Transaction[];
                                values.forEach((item, position) => rows.put(rowRecord(record.scope, item, position)));
                                const { value: _value, payload: _payload, ...metadata } = record;
                                cursor.update({ ...metadata, rowStore: true, nextPosition: values.length, firstPosition: 0 });
                            } else if (!record.rowStore && !owns(record, 'payload')) cursor.update(name === DATA_STORE ? opaqueData(record) : opaqueMutation(record, false));
                            cursor.continue();
                        } catch (error) {
                            migrationError = error;
                            transaction.abort();
                        }
                    };
                }
            };
            request.onsuccess = () => {
                const db = request.result;
                if (cancelled) { db.close(); return; }
                db.onversionchange = () => { db.close(); if (this.dbPromise === opening) this.dbPromise = null; };
                resolve(db);
            };
            request.onerror = () => reject(migrationError || request.error || new Error('IndexedDB open failed'));
        });
        this.dbPromise = opening;
        void opening.catch(() => { if (this.dbPromise === opening) this.dbPromise = null; });
        return opening;
    }

    async getData<K extends OfflineDataKey>(key: K): Promise<OfflineDataMap[K] | null> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, ROW_STORE]);
        const record = await requestToPromise<OfflineDataRecord<K> | undefined>(transaction.objectStore(DATA_STORE).get(scopedKey(key, scope)));
        return record ? collectionValue(transaction, record) : null;
    }

    async setData<K extends OfflineDataKey>(key: K, value: OfflineDataMap[K]): Promise<void> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, ROW_STORE], 'readwrite');
        const done = transactionDone(transaction);
        try { await writeCollection(transaction, scope, key, value); }
        catch (error) { await abortTransaction(transaction, done); throw error; }
        await done;
    }

    async getRevision(key: OfflineDataKey): Promise<string | number | undefined> {
        const scope = currentScope();
        const db = await this.openDb();
        const record = await requestToPromise<OfflineDataRecord | undefined>(db.transaction(DATA_STORE).objectStore(DATA_STORE).get(scopedKey(key, scope)));
        return record?.revision ?? record?.updatedAt;
    }

    async acceptRemoteData<K extends OfflineDataKey>(key: K, value: OfflineDataMap[K], revision: string | number | undefined): Promise<OfflineDataMap[K]> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE, ROW_STORE], 'readwrite');
        const done = transactionDone(transaction);
        const data = transaction.objectStore(DATA_STORE);
        try {
            const [current, queued] = await Promise.all([
                requestToPromise<OfflineDataRecord<K> | undefined>(data.get(scopedKey(key, scope))),
                requestToPromise<StoredMutation[]>(transaction.objectStore(MUTATION_STORE).index('scope').getAll(scope)),
            ]);
            const pending = queued.filter(item => item.scope === scope);
            pending.forEach(record => readMutation(record));
            const currentValue = current ? await collectionValue(transaction, current) : undefined;
            const changed = (current?.revision ?? current?.updatedAt) !== revision || pending.some(item => affectsCollection(item, key));
            if (changed && current) { await done; return currentValue as OfflineDataMap[K]; }
            await writeCollection(transaction, scope, key, value);
            await done;
            return value;
        } catch (error) { await abortTransaction(transaction, done); throw error; }
    }

    async commitBankMutation(path: string, method: string, body?: string, settings?: Settings): Promise<void> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE, ROW_STORE], 'readwrite');
        const done = transactionDone(transaction);
        const dataStore = transaction.objectStore(DATA_STORE);
        const resource = path.split('/')[2];
        const affected: BankKey[] = settings ? []
            : resource === 'transfers' ? ['transactions']
            : resource === 'accounts' && method === 'DELETE' ? ['accounts', 'transactions', 'scheduled', 'budgets']
            : resource === 'budgets' && method === 'DELETE' ? ['budgets', 'scheduled']
            : bankKeys.filter(key => key === resource);
        const requests = affected.map(key => requestToPromise<OfflineDataRecord | undefined>(dataStore.get(scopedKey(key, scope))));
        const queued = requestToPromise<StoredMutation[]>(transaction.objectStore(MUTATION_STORE).index('scope').getAll(scope));
        try {
            const [records, pending] = await Promise.all([Promise.all(requests), queued]);
            pending.filter(item => item.scope === scope).forEach(record => readMutation(record));
            const createdAt = pending.reduce((latest, item) => Math.max(latest, item.createdAt), Date.now());
            const sequence = pending.reduce((latest, item) => Math.max(latest, item.sequence || 0), 0) + 1;
            let mutation: { method: string; body?: string };
            if (!settings && affected.length === 1 && affected[0] === 'transactions') {
                const metadata = records[0];
                const rows = transaction.objectStore(ROW_STORE);
                const input = body ? JSON.parse(body) : undefined;
                const id = method === 'DELETE' ? decodeURIComponent(path.split('/')[3]) : input?.id ?? input?.fromTransaction?.id;
                const previous = id ? await requestToPromise<TransactionRow | undefined>(rows.get(rowKey(scope, id))) : undefined;
                const linkedId = previous ? readRow(previous).linkedTransactionId : input?.toTransaction?.id;
                const linked = linkedId ? await requestToPromise<TransactionRow | undefined>(rows.get(rowKey(scope, linkedId))) : undefined;
                const existing = [previous, linked].filter((item): item is TransactionRow => !!item);
                const snapshot = Object.fromEntries(bankKeys.map(key => [key, key === 'transactions' ? existing.map(readRow) : []])) as unknown as BankSnapshot;
                mutation = applyBankMutation(snapshot, path, method, body);
                const retained = new Set(snapshot.transactions.map(item => item.id));
                for (const item of existing) if (!retained.has(item.id)) rows.delete(item.key);
                let nextPosition = metadata?.nextPosition ?? 0;
                let firstPosition = metadata?.firstPosition ?? 0;
                for (const item of snapshot.transactions) {
                    const oldRow = existing.find(row => row.id === item.id);
                    const position = oldRow?.position ?? (resource === 'transfers' ? --firstPosition : nextPosition++);
                    rows.put(rowRecord(scope, item, position));
                }
                if (metadata || snapshot.transactions.length) dataStore.put({ key: scopedKey('transactions', scope), scope, dataKey: 'transactions', rowStore: true, nextPosition, firstPosition, updatedAt: Date.now(), revision: randomId() });
            } else {
                const values = await Promise.all(records.map(record => record ? collectionValue(transaction, record) : undefined));
                const snapshot = Object.fromEntries(bankKeys.map(key => [key, values[affected.indexOf(key)] ?? []])) as unknown as BankSnapshot;
                mutation = settings ? { method, body } : applyBankMutation(snapshot, path, method, body);
                if (settings) await writeCollection(transaction, scope, 'settings', settings);
                for (const [index, key] of affected.entries()) {
                    if (records[index] || snapshot[key].length > 0) await writeCollection(transaction, scope, key, snapshot[key]);
                }
            }
            transaction.objectStore(MUTATION_STORE).put({ id: randomId(), scope, path, method: mutation.method, payload: encodeOfflinePayload({ body: mutation.body }), createdAt, sequence } satisfies StoredMutation);
        } catch (error) { await abortTransaction(transaction, done); throw error; }
        await done;
    }

    async listMutations(): Promise<OfflineMutation[]> {
        const scope = currentScope();
        const db = await this.openDb();
        const mutations = await requestToPromise<StoredMutation[]>(db.transaction(MUTATION_STORE).objectStore(MUTATION_STORE).index('scope').getAll(scope));
        return mutations.filter(mutation => mutation.scope === scope).map(record => readMutation(record)).sort((a, b) => a.createdAt - b.createdAt || (a.sequence || 0) - (b.sequence || 0) || a.id.localeCompare(b.id));
    }

    /** Read settings and their pending changes from the same IndexedDB snapshot. */
    async getSettingsSnapshot(): Promise<{ settings: Settings | null; mutations: OfflineMutation[] }> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE]);
        const [record, queued] = await Promise.all([
            requestToPromise<OfflineDataRecord<'settings'> | undefined>(transaction.objectStore(DATA_STORE).get(scopedKey('settings', scope))),
            requestToPromise<StoredMutation[]>(transaction.objectStore(MUTATION_STORE).index('scope').getAll(scope)),
        ]);
        return {
            settings: record ? readData(record) : null,
            mutations: queued.map(item => readMutation(item)).sort((a, b) => a.createdAt - b.createdAt || (a.sequence || 0) - (b.sequence || 0) || a.id.localeCompare(b.id)),
        };
    }

    /** No payload decoding: an explicit recovery must also work with a corrupt queue. */
    async listMutationIds(): Promise<string[]> {
        const scope = currentScope();
        const db = await this.openDb();
        return await requestToPromise<IDBValidKey[]>(db.transaction(MUTATION_STORE).objectStore(MUTATION_STORE).index('scope').getAllKeys(scope)) as string[];
    }

    async removeMutation(id: string): Promise<void> {
        const db = await this.openDb();
        const transaction = db.transaction(MUTATION_STORE, 'readwrite');
        const done = transactionDone(transaction);
        transaction.objectStore(MUTATION_STORE).delete(id);
        await done;
    }

    async replaceWithRemoteSnapshot(values: OfflineDataMap, expectedIds: string[]): Promise<void> {
        const scope = currentScope();
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE, ROW_STORE], 'readwrite');
        const done = transactionDone(transaction);
        try {
            const store = transaction.objectStore(MUTATION_STORE);
            const ids = await requestToPromise<IDBValidKey[]>(store.index('scope').getAllKeys(scope));
            if (JSON.stringify([...ids].sort()) !== JSON.stringify([...expectedIds].sort())) throw new Error('Les modifications locales ont changé pendant le rechargement. Réessayez.');
            for (const key of [...bankKeys, 'settings'] as OfflineDataKey[]) await writeCollection(transaction, scope, key, values[key]);
            for (const id of ids) store.delete(id);
        } catch (error) { await abortTransaction(transaction, done); throw error; }
        await done;
    }

    async setMutationFailure(id: string, failure: OfflineMutation['failure']): Promise<void> {
        const db = await this.openDb();
        const transaction = db.transaction(MUTATION_STORE, 'readwrite');
        const done = transactionDone(transaction);
        const store = transaction.objectStore(MUTATION_STORE);
        try {
            const record = await requestToPromise<StoredMutation | undefined>(store.get(id));
            if (record && record.scope === currentScope()) store.put({ ...record, failure });
        } catch (error) { await abortTransaction(transaction, done); throw error; }
        await done;
    }

    /** Move one desktop's data and unsent mutations atomically to its new port. */
    async migrateScope(previousScope: string): Promise<number> {
        const scope = currentScope();
        if (!previousScope || previousScope === scope) return 0;
        const db = await this.openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE, ROW_STORE], 'readwrite');
        const done = transactionDone(transaction);
        const dataStore = transaction.objectStore(DATA_STORE);
        const mutationStore = transaction.objectStore(MUTATION_STORE);
        try {
            const [records, mutations] = await Promise.all([
                requestToPromise<OfflineDataRecord[]>(dataStore.getAll()), requestToPromise<StoredMutation[]>(mutationStore.getAll()),
            ]);
            const staleRecords = records.filter(record => record.scope === previousScope).map(record => record.rowStore ? record : opaqueData(record));
            const oldRows = await requestToPromise<TransactionRow[]>(transaction.objectStore(ROW_STORE).index('scope').getAll(previousScope));
            oldRows.forEach(readRow);
            const staleMutations = mutations.filter(mutation => mutation.scope === previousScope).map(record => opaqueMutation(record));
            const existingKeys = new Set(records.map(record => record.key));
            for (const record of staleRecords) {
                const key = scopedKey(record.dataKey, scope);
                if (!existingKeys.has(key)) dataStore.put({ ...record, key, scope });
                dataStore.delete(record.key);
            }
            const destinationHasTransactions = existingKeys.has(scopedKey('transactions', scope));
            for (const row of oldRows) {
                if (!destinationHasTransactions) transaction.objectStore(ROW_STORE).put({ ...row, key: rowKey(scope, row.id), scope });
                transaction.objectStore(ROW_STORE).delete(row.key);
            }
            for (const mutation of staleMutations) mutationStore.put({ ...mutation, scope });
            await done;
            return staleMutations.length;
        } catch (error) { await abortTransaction(transaction, done); throw error; }
    }

    async clearAll(): Promise<void> {
        // Explicit unlink/erase must remain possible even when a corrupt version 1
        // record prevents migration. Open the existing schema without decoding it.
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
            const request = indexedDB.open(this.databaseName);
            let cancelled = false;
            request.onblocked = () => { cancelled = true; reject(blockedOpen()); };
            request.onsuccess = () => {
                if (cancelled) { request.result.close(); return; }
                request.result.onversionchange = () => request.result.close();
                resolve(request.result);
            };
            request.onerror = () => reject(request.error || new Error('IndexedDB open failed'));
        });
        try {
            const stores = [DATA_STORE, MUTATION_STORE, ROW_STORE].filter(name => db.objectStoreNames.contains(name));
            if (!stores.length) return;
            const transaction = db.transaction(stores, 'readwrite');
            const done = transactionDone(transaction);
            try {
                for (const name of stores) transaction.objectStore(name).clear();
            } catch (error) { await abortTransaction(transaction, done); throw error; }
            await done;
        } finally { db.close(); }
    }
}

export const offlineStore = new OfflineStore();
