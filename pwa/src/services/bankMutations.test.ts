import { expect, test } from 'bun:test';
import { applyBankMutation, type BankSnapshot } from './bankMutations';
const empty = (): BankSnapshot => ({ accounts: [], transactions: [], categories: [], budgets: [], scheduled: [] });

test('repeated creations remain unique in every offline collection', () => {
    for (const key of ['accounts', 'transactions', 'categories', 'budgets', 'scheduled'] as const) {
        const data = empty();
        for (let i = 0; i < 3; i++) applyBankMutation(data, `/api/${key}`, 'POST', JSON.stringify({ id: 'same' }));
        expect(data[key]).toHaveLength(1);
    }
});
test('queued edits carry the preceding local version without changing it', () => {
    const data = empty();
    data.budgets = [{ id: 'b', name: 'Courses', amount: 50, category: '5' }];
    const next = { ...data.budgets[0], amount: 75 };
    const mutation = applyBankMutation(data, '/api/budgets', 'PUT', JSON.stringify(next));
    expect(mutation.method).toBe('PATCH');
    expect(JSON.parse(mutation.body!)._base.amount).toBe(50);
    expect(data.budgets[0].amount).toBe(75);
    const later = applyBankMutation(data, '/api/budgets', 'PUT', JSON.stringify({ ...next, amount: 90 }));
    expect(JSON.parse(later.body!)._base.amount).toBe(75);
});
test('a transfer is cached once and deletion removes both sides', () => {
    const data = empty();
    const from = { id: 'from', accountId: 'a', linkedTransactionId: 'to', isTransfer: true };
    const to = { id: 'to', accountId: 'b', linkedTransactionId: 'from', isTransfer: true };
    for (let i = 0; i < 3; i++) applyBankMutation(data, '/api/transfers', 'POST', JSON.stringify({ fromTransaction: from, toTransaction: to }));
    expect(data.transactions).toHaveLength(2);
    applyBankMutation(data, '/api/transactions/from', 'DELETE');
    expect(data.transactions).toHaveLength(0);
});
test('deleting the destination account removes its scheduled transfers too', () => {
    const data = empty();
    data.scheduled = [{ id: 's', accountId: 'source', toAccountId: 'destination' }] as never;
    applyBankMutation(data, '/api/accounts/destination', 'DELETE');
    expect(data.scheduled).toHaveLength(0);
});
