import type { Account, Budget, Category, ScheduledTransaction, Transaction } from '../types';
import { applyTransactionUpdate, mergeTransactionDisplay } from '../utils/transactionUpdate';

export interface BankSnapshot {
    accounts: Account[];
    transactions: Transaction[];
    categories: Category[];
    budgets: Budget[];
    scheduled: ScheduledTransaction[];
}
export type BankKey = keyof BankSnapshot;
export const bankKeys: BankKey[] = ['accounts', 'transactions', 'categories', 'budgets', 'scheduled'];

// Pure transformation used inside one IndexedDB transaction with the outgoing message.
export function applyBankMutation(data: BankSnapshot, path: string, method: string, body?: string) {
    const [, , resource, encodedId] = path.split('/');
    const raw = body ? JSON.parse(body) : undefined;
    const { _base: explicitBase, ...payload } = raw || {};
    const key = resource as BankKey;
    if (resource === 'transfers' && method === 'PATCH') {
        const additions: Transaction[] = [payload.fromTransaction, payload.toTransaction];
        for (const item of additions) {
            const current = data.transactions.find(row => row.id === item.id);
            const base = item.type === 'expense' ? explicitBase?.fromTransaction : explicitBase?.toTransaction;
            if (!current || !base) throw new Error('Virement absent ou état initial manquant. Reconnectez-vous avant de le modifier.');
            data.transactions = data.transactions.map(row => row.id === item.id ? mergeTransactionDisplay(current, item, base) : row);
        }
        return { method, body };
    }
    if (resource === 'transfers' && method === 'POST') {
        const additions: Transaction[] = [payload.fromTransaction, payload.toTransaction];
        data.transactions = [...additions, ...data.transactions.filter(item => !additions.some(next => next.id === item.id))];
        return { method, body };
    }
    if (!bankKeys.includes(key)) throw new Error('Collection de synchronisation inconnue.');
    const items = data[key] as { id: string }[];
    if (method === 'POST') {
        // Repeating a local submission must not duplicate the displayed record either.
        (data[key] as { id: string }[]) = [...items.filter(item => item.id !== payload.id), payload];
    } else if (method === 'PUT') {
        const previous = items.find(item => item.id === payload.id);
        if (!previous) throw new Error('Élément absent du cache. Reconnectez-vous avant de le modifier.');
        if (key === 'transactions') data.transactions = applyTransactionUpdate(data.transactions, explicitBase ? mergeTransactionDisplay(previous as Transaction, payload, explicitBase) : payload);
        else (data[key] as { id: string }[]) = items.map(item => item.id === payload.id ? payload : item);
        return { method: 'PATCH', body: JSON.stringify({ ...payload, _base: explicitBase ?? previous }) };
    } else if (method === 'DELETE') {
        const id = decodeURIComponent(encodedId);
        (data[key] as { id: string }[]) = items.filter(item => item.id !== id);
        if (key === 'accounts') {
            data.transactions = data.transactions.filter(item => item.accountId !== id);
            data.scheduled = data.scheduled.filter(item => item.accountId !== id && item.toAccountId !== id);
            data.budgets = data.budgets.map(item => item.accountId === id ? { ...item, accountId: undefined } : item);
        } else if (key === 'transactions') {
            const previous = (items as Transaction[]).find(item => item.id === id);
            if (previous?.linkedTransactionId) data.transactions = data.transactions.filter(item => item.id !== previous.linkedTransactionId);
        } else if (key === 'budgets') {
            data.scheduled = data.scheduled.map(item => item.budgetId === id ? { ...item, budgetId: undefined, includeInForecast: false } : item);
        }
    } else throw new Error('Mutation de synchronisation inconnue.');
    return { method, body };
}
