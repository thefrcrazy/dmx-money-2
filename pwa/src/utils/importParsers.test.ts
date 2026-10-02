import { describe, expect, test } from 'bun:test';
import {
    filterDuplicateTransactions,
    parseBankAmount,
    parseBankDate,
    parseDelimitedRows,
    parseOfxTransactions,
    parseQifTransactions
} from './importParsers';
import type { Transaction } from '../types';

describe('importParsers', () => {
    test('parses quoted CSV rows with separators inside cells', () => {
        const rows = parseDelimitedRows('Date;Montant;Libelle\n"18/05/2026";"-1 234,56";"Achat; carte"\n', ';', true);

        expect(rows).toEqual([
            ['18/05/2026', '-1 234,56', 'Achat; carte']
        ]);
    });

    test('normalizes common bank amount and date formats', () => {
        expect(parseBankAmount('1 234,56 €')).toBe(1234.56);
        expect(parseBankAmount('1,234.56')).toBe(1234.56);
        expect(parseBankDate('18/05/26')).toBe('2026-05-18');
        expect(parseBankDate('2026-05-18')).toBe('2026-05-18');
    });

    test('parses QIF transactions', () => {
        const transactions = parseQifTransactions("!Type:Bank\nD18/05'26\nT-12,34\nPCafe\nLRestaurants\n^\n");

        expect(transactions).toEqual([
            {
                date: '2026-05-18',
                amount: -12.34,
                description: 'Cafe',
                category: 'Restaurants'
            }
        ]);
    });

    test('parses OFX SGML transactions', () => {
        const transactions = parseOfxTransactions('<OFX><STMTTRN><DTPOSTED>20260518120000<TRNAMT>-42.50<NAME>SHOP &amp; CO<MEMO>Card</STMTTRN></OFX>');

        expect(transactions).toEqual([
            {
                date: '2026-05-18',
                amount: -42.5,
                description: 'SHOP & CO - Card'
            }
        ]);
    });

    test('filters duplicate imports with accent-insensitive descriptions', () => {
        const existing: Transaction[] = [{
            id: 'existing',
            date: '2026-05-18',
            accountId: 'account-1',
            type: 'expense',
            amount: 12.34,
            category: 'food',
            description: 'Café du centre',
            checked: true
        }];

        const result = filterDuplicateTransactions([
            {
                date: '2026-05-18',
                accountId: 'account-1',
                type: 'expense',
                amount: 12.34,
                category: 'food',
                description: 'Cafe du centre',
                checked: true
            }
        ], existing, 'account-1');

        expect(result.unique).toHaveLength(0);
        expect(result.duplicateCount).toBe(1);
    });

    test('preserves legitimate identical purchases and matches repeated statement occurrences', () => {
        const purchase = { date: '2026-05-18', amount: 10, type: 'expense' as const, description: 'Café', category: 'food', accountId: 'account-1' };
        expect(filterDuplicateTransactions([purchase, purchase], [], 'account-1').unique).toHaveLength(2);
        const firstImport = [{ ...purchase, id: 'first' }, { ...purchase, id: 'second' }];
        expect(filterDuplicateTransactions([purchase, purchase], firstImport, 'account-1').duplicateCount).toBe(2);
        expect(filterDuplicateTransactions([purchase, purchase], [firstImport[0]], 'account-1').unique).toHaveLength(1);
    });

    test('rejects invalid monetary values and impossible dates without replacing them silently', () => {
        for (const amount of ['', 'abc', 'NaN', '1e5', 'Infinity', '12,34,56']) expect(() => parseBankAmount(amount)).toThrow();
        for (const date of ['', '31/02/2026', '2026-13-01', 'oops']) expect(() => parseBankDate(date)).toThrow();
        expect(parseBankDate('29/02/2024')).toBe('2024-02-29');
        expect(() => parseOfxTransactions('<STMTTRN><DTPOSTED>20260231<TRNAMT>12')).toThrow();
        expect(() => parseQifTransactions('!Type:Bank\nPCafe\n^')).toThrow();
        expect(() => parseDelimitedRows('"unfinished;123', ';')).toThrow();
    });
});
