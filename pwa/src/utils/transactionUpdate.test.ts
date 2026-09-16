import { expect, test } from 'bun:test';
import { applyTransactionUpdate } from './transactionUpdate';
import type { Transaction } from '../types';

test('updates both transfer legs without changing account or direction', () => {
  const from: Transaction = { id: 'from', accountId: 'a', type: 'expense', date: '2026-09-16', amount: 10, description: 'old', category: 'transfer', checked: false, isTransfer: true, linkedTransactionId: 'to' };
  const to: Transaction = { ...from, id: 'to', accountId: 'b', type: 'income', linkedTransactionId: 'from' };
  const updated = { ...from, amount: 25, checked: true, description: 'new' };
  const result = applyTransactionUpdate([from, to], updated);
  expect(result[1]).toEqual({ ...to, amount: 25, checked: true, description: 'new' });
  expect(result.reduce((sum, t) => sum + (t.type === 'income' ? t.amount : -t.amount), 0)).toBe(0);
  expect(to.amount).toBe(10);
});
