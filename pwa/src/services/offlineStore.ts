import { applyBankMutation, bankKeys, type BankSnapshot } from './bankMutations';
import type { Account, Budget, Category, ScheduledTransaction, Settings, Transaction } from '../types';
import { getMobileApiBaseUrl } from '../utils/runtime';

export type OfflineDataKey =
    | 'accounts'
    | 'transactions'
    | 'categories'
    | 'budgets'
    | 'scheduled'
    | 'settings';

export interface OfflineMutation {
    id: string;
    scope: string;
    path: string;
    method: string;
    body?: string;
    createdAt: number;
    sequence?: number;
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
    value: OfflineDataMap[K];
    updatedAt: number;
    revision?: string;
}

const DB_NAME = 'dmxmoney-mobile-offline';
const DB_VERSION = 1;
const DATA_STORE = 'data';
const MUTATION_STORE = 'mutations';

let dbPromise: Promise<IDBDatabase> | null = null;

const currentScope = () => getMobileApiBaseUrl() || 'unpaired';
const scopedKey = (key: OfflineDataKey, scope = currentScope()) => `${scope}:${key}`;

const requestToPromise = <T>(request: IDBRequest<T>) =>
    new Promise<T>((resolve, reject) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error || new Error('IndexedDB request failed'));
    });

const transactionDone = (transaction: IDBTransaction) =>
    new Promise<void>((resolve, reject) => {
        transaction.oncomplete = () => resolve();
        transaction.onerror = () => reject(transaction.error || new Error('IndexedDB transaction failed'));
        transaction.onabort = () => reject(transaction.error || new Error('IndexedDB transaction aborted'));
    });

const openDb = () => {
    if (dbPromise) return dbPromise;

    dbPromise = new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open(DB_NAME, DB_VERSION);

        request.onupgradeneeded = () => {
            const db = request.result;
            if (!db.objectStoreNames.contains(DATA_STORE)) {
                db.createObjectStore(DATA_STORE, { keyPath: 'key' });
            }
            if (!db.objectStoreNames.contains(MUTATION_STORE)) {
                db.createObjectStore(MUTATION_STORE, { keyPath: 'id' });
            }
        };

        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error || new Error('IndexedDB open failed'));
    });

    return dbPromise;
};

const randomId = () => {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
        return crypto.randomUUID();
    }
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
};

export const offlineStore = {
    async getData<K extends OfflineDataKey>(key: K): Promise<OfflineDataMap[K] | null> {
        const db = await openDb();
        const record = await requestToPromise<OfflineDataRecord<K> | undefined>(
            db.transaction(DATA_STORE, 'readonly')
                .objectStore(DATA_STORE)
                .get(scopedKey(key))
        );
        return record?.value ?? null;
    },

    async setData<K extends OfflineDataKey>(key: K, value: OfflineDataMap[K]): Promise<void> {
        const db = await openDb();
        const transaction = db.transaction(DATA_STORE, 'readwrite');
        transaction.objectStore(DATA_STORE).put({
            key: scopedKey(key),
            scope: currentScope(),
            dataKey: key,
            value,
            updatedAt: Date.now(),
            revision: randomId(),
        } satisfies OfflineDataRecord<K>);
        await transactionDone(transaction);
    },

    async getRevision(key: OfflineDataKey): Promise<string | number | undefined> {
        const db = await openDb();
        const record = await requestToPromise<OfflineDataRecord | undefined>(db.transaction(DATA_STORE).objectStore(DATA_STORE).get(scopedKey(key)));
        return record?.revision ?? record?.updatedAt;
    },

    async acceptRemoteData<K extends OfflineDataKey>(key: K, value: OfflineDataMap[K], revision: string | number | undefined): Promise<OfflineDataMap[K]> {
        const db = await openDb();
        const scope = currentScope();
        const tx = db.transaction([DATA_STORE, MUTATION_STORE], 'readwrite');
        const done = transactionDone(tx);
        const data = tx.objectStore(DATA_STORE);
        const [current, queued] = await Promise.all([
            requestToPromise<OfflineDataRecord<K> | undefined>(data.get(scopedKey(key, scope))),
            requestToPromise<OfflineMutation[]>(tx.objectStore(MUTATION_STORE).getAll()),
        ]);
        const changed = (current?.revision ?? current?.updatedAt) !== revision || queued.some(item => item.scope === scope);
        if (changed && current) {
            await done;
            return current.value;
        }
        data.put({ key: scopedKey(key, scope), scope, dataKey: key, value, updatedAt: Date.now(), revision: randomId() });
        await done;
        return value;
    },

    async commitBankMutation(path: string, method: string, body?: string, settings?: Settings): Promise<void> {
        const db = await openDb();
        const scope = currentScope();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE], 'readwrite');
        const done = transactionDone(transaction);
        const dataStore = transaction.objectStore(DATA_STORE);
        const requests = bankKeys.map(key => requestToPromise<OfflineDataRecord | undefined>(dataStore.get(scopedKey(key, scope))));
        const queued = requestToPromise<OfflineMutation[]>(transaction.objectStore(MUTATION_STORE).getAll());
        try {
            const [records, pending] = await Promise.all([Promise.all(requests), queued]);
            const createdAt = pending.reduce((latest, item) => Math.max(latest, item.createdAt), Date.now());
            const sequence = pending.reduce((latest, item) => Math.max(latest, item.sequence || 0), 0) + 1;
            const snapshot = Object.fromEntries(bankKeys.map((key, index) => [key, records[index]?.value ?? []])) as unknown as BankSnapshot;
            const mutation = settings ? { method, body } : applyBankMutation(snapshot, path, method, body);
            if (settings) dataStore.put({ key: scopedKey('settings', scope), scope, dataKey: 'settings', value: settings, updatedAt: Date.now(), revision: randomId() });
            bankKeys.forEach((key, index) => {
                // Don't turn a partial cache into a complete bank snapshot.
                if (records[index] || snapshot[key].length > 0) {
                    dataStore.put({ key: scopedKey(key, scope), scope, dataKey: key, value: snapshot[key], updatedAt: Date.now(), revision: randomId() });
                }
            });
            transaction.objectStore(MUTATION_STORE).put({
                id: randomId(), scope, path, ...mutation, createdAt, sequence,
            } satisfies OfflineMutation);
        } catch (error) {
            transaction.abort();
            await done.catch(() => undefined);
            throw error;
        }
        await done;
    },

    async listMutations(): Promise<OfflineMutation[]> {
        const db = await openDb();
        const mutations = await requestToPromise<OfflineMutation[]>(
            db.transaction(MUTATION_STORE, 'readonly')
                .objectStore(MUTATION_STORE)
                .getAll()
        );
        const scope = currentScope();
        return mutations
            .filter(mutation => mutation.scope === scope)
            .sort((a, b) => (
                a.createdAt - b.createdAt
                || (a.sequence || 0) - (b.sequence || 0)
                || a.id.localeCompare(b.id)
            ));
    },

    async removeMutation(id: string): Promise<void> {
        const db = await openDb();
        const transaction = db.transaction(MUTATION_STORE, 'readwrite');
        transaction.objectStore(MUTATION_STORE).delete(id);
        await transactionDone(transaction);
    },

    /**
     * Carries the cached data and the still-unsent mutations over to a new API
     * base URL. The desktop can come back on another port or with a re-issued
     * bridge host, and everything the mobile changed while it was away lives
     * under the old scope: without this it would stay there forever.
     */
    async migrateScope(previousScope: string): Promise<number> {
        const scope = currentScope();
        if (!previousScope || previousScope === scope) return 0;

        const db = await openDb();
        const readTransaction = db.transaction([DATA_STORE, MUTATION_STORE], 'readonly');
        const [records, mutations] = await Promise.all([
            requestToPromise<OfflineDataRecord[]>(readTransaction.objectStore(DATA_STORE).getAll()),
            requestToPromise<OfflineMutation[]>(readTransaction.objectStore(MUTATION_STORE).getAll()),
        ]);

        const staleRecords = records.filter(record => record.scope === previousScope);
        const staleMutations = mutations.filter(mutation => mutation.scope === previousScope);
        if (staleRecords.length === 0 && staleMutations.length === 0) return 0;

        const existingKeys = new Set(records.map(record => record.key));
        const writeTransaction = db.transaction([DATA_STORE, MUTATION_STORE], 'readwrite');
        const dataStore = writeTransaction.objectStore(DATA_STORE);
        const mutationStore = writeTransaction.objectStore(MUTATION_STORE);

        staleRecords.forEach(record => {
            const key = scopedKey(record.dataKey, scope);
            // Data already fetched under the new scope is fresher than the cache
            // we are carrying over, so it wins.
            if (!existingKeys.has(key)) {
                dataStore.put({ ...record, key, scope });
            }
            dataStore.delete(record.key);
        });

        staleMutations.forEach(mutation => {
            mutationStore.put({ ...mutation, scope });
        });

        await transactionDone(writeTransaction);
        return staleMutations.length;
    },

    async clearAll(): Promise<void> {
        const db = await openDb();
        const transaction = db.transaction([DATA_STORE, MUTATION_STORE], 'readwrite');
        transaction.objectStore(DATA_STORE).clear();
        transaction.objectStore(MUTATION_STORE).clear();
        await transactionDone(transaction);
    },
};
