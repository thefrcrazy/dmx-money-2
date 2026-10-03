import type { OfflineMutation } from './offlineStore';

export function mutationKeys(mutation: OfflineMutation, includeReferences: boolean): Set<string> {
    const [, , resource, encodedId] = mutation.path.split('/');
    if (resource === 'settings') return new Set(['settings']);
    const body = mutation.body ? JSON.parse(mutation.body) : {};
    const items = resource === 'transfers' ? [body.fromTransaction, body.toTransaction] : [body];
    const keys = new Set<string>();
    const collection = resource === 'transfers' ? 'transactions' : resource;
    if (encodedId) keys.add(`${collection}:${decodeURIComponent(encodedId)}`);
    for (const item of items) {
        if (!item) continue;
        if (item.id) keys.add(`${collection}:${item.id}`);
        if (item.linkedTransactionId) keys.add(`transactions:${item.linkedTransactionId}`);
        if (!includeReferences) continue;
        for (const key of ['accountId', 'toAccountId']) if (item[key]) keys.add(`accounts:${item[key]}`);
        if (item.category) keys.add(`categories:${item.category}`);
        if (item.budgetId) keys.add(`budgets:${item.budgetId}`);
    }
    return keys;
}

export function affectsCollection(mutation: OfflineMutation, key: string): boolean {
    const resource = mutation.path.split('/')[2];
    return resource === key || (resource === 'transfers' && key === 'transactions')
        || (mutation.method === 'DELETE' && resource === 'accounts' && ['transactions', 'scheduled', 'budgets'].includes(key))
        || (mutation.method === 'DELETE' && resource === 'budgets' && key === 'scheduled');
}
