import { expect, test } from 'bun:test';
import type { Transaction } from '../types';
import { allowedTransactionFormTypes, transactionFormTypeError } from './transactionEditTypes';

const simple: Transaction = {
    id: 'operation', date: '2026-10-03', accountId: 'account', type: 'expense', amount: 47.32,
    category: 'category', description: 'Opération fictive', checked: true,
    bankSource: 'ofx:bank', bankTransactionId: 'FITID-1',
};

test('new operations retain all three types while an unknown type is rejected', () => {
    expect(allowedTransactionFormTypes(null)).toEqual(['expense', 'income', 'transfer']);
    for (const type of ['expense', 'income', 'transfer']) expect(transactionFormTypeError(null, type)).toBeNull();
    expect(transactionFormTypeError(null, 'refund')).toContain('type valide');
});

test('editing a simple operation allows expense/income but rejects conversion', () => {
    for (const originalType of ['expense', 'income'] as const) {
        const transaction = { ...simple, type: originalType };
        const before = structuredClone(transaction);
        expect(allowedTransactionFormTypes(transaction)).toEqual(['expense', 'income']);
        expect(transactionFormTypeError(transaction, 'expense')).toBeNull();
        expect(transactionFormTypeError(transaction, 'income')).toBeNull();
        const error = transactionFormTypeError(transaction, 'transfer');
        expect(error).toContain('Dépense ou Revenu');
        expect(error).toContain('application native');
        expect(transaction).toEqual(before);
    }
});

test('both sides of an existing transfer keep transfer-only editing and retain identities', () => {
    for (const direction of ['expense', 'income'] as const) {
        const transaction = { ...simple, type: direction, category: 'transfer', isTransfer: true, linkedTransactionId: 'peer' };
        const before = structuredClone(transaction);
        expect(allowedTransactionFormTypes(transaction)).toEqual(['transfer']);
        expect(transactionFormTypeError(transaction, 'transfer')).toBeNull();
        for (const type of ['expense', 'income']) {
            expect(transactionFormTypeError(transaction, type)).toContain('conserver le type Virement');
            expect(transactionFormTypeError(transaction, type)).toContain('application native');
        }
        expect(transaction).toEqual(before);
    }
});

test('incomplete transfer metadata cannot enable conversion to a simple operation', () => {
    for (const marker of [{ isTransfer: true }, { linkedTransactionId: 'peer' }, { category: 'transfer' }, { type: 'transfer' as const }]) {
        const transaction = { ...simple, ...marker };
        expect(allowedTransactionFormTypes(transaction)).toEqual(['transfer']);
        expect(transactionFormTypeError(transaction, 'expense')).not.toBeNull();
        expect(transactionFormTypeError(transaction, 'income')).not.toBeNull();
    }
});
