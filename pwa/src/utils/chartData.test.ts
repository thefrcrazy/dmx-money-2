import { expect, test } from 'bun:test';
import { buildBalanceHistory, chartRangeError, parseBankDate } from './chartData';
import type { Account, Transaction } from '../types';

test('bank dates reject impossible days and excessive chart ranges before allocation', () => {
  expect(Number.isNaN(parseBankDate('2026-02-30').getTime())).toBe(true);
  expect(chartRangeError(parseBankDate('2026-01-01'), parseBankDate('9999-12-31'), 1)).toContain('cinq ans');
  expect(chartRangeError(parseBankDate('2026-01-01'), parseBankDate('2026-12-31'), 2000)).toContain('Trop de points');
});

test('identical account names and metadata-like IDs preserve independent series', () => {
  const accounts = [{ id: 'date', name: 'Même nom', initialBalance: 100 }, { id: 'fullDate', name: 'Même nom', initialBalance: 200 }] as Account[];
  const transactions = [{ accountId: 'date', date: '2026-10-01', amount: 10, type: 'expense' }] as Transaction[];
  const result = buildBalanceHistory(accounts, transactions, parseBankDate('2026-10-01'), parseBankDate('2026-10-01'));
  expect(result).toHaveLength(1);
  expect(result[0].balances).toEqual({ date: 90, fullDate: 200 });
  expect(result[0].date).toBe('01 oct.');
});
