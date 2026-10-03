import { expect, test } from 'bun:test';
import { filterDuplicateTransactions, parseOfxTransactions } from '../../utils/importParsers';
import type { Transaction } from '../../types';
import { ofxInitialBalanceFromFinal, prepareOfxImport } from './ofxImportMapping';

const statement = (account: string, ids: string[]) => `<STMTRS><BANKACCTFROM><BANKID>1<ACCTID>${account}<ACCTTYPE>CHECKING</BANKACCTFROM>${ids.map(id => `<STMTTRN><DTPOSTED>20261003<TRNAMT>-10<NAME>Achat fictif<FITID>${id}</STMTTRN>`).join('')}</STMTRS>`;
const document = (...statements: string[]) => `<OFX><FI><FID>BANK<ORG>ORG</FI>${statements.join('')}</OFX>`;

test('the final OFX mapping preserves bank identity through import and repeat deduplication', () => {
    const parsed = parseOfxTransactions(document(statement('A', ['one', 'two']), statement('B', ['one'])));
    const incoming = prepareOfxImport(parsed, 'local', {}, 'category');
    expect(incoming.map(transaction => [transaction.bankSource, transaction.bankTransactionId])).toEqual([
        ['ofx:["BANK","ORG","1","","A","CHECKING"]', 'one'],
        ['ofx:["BANK","ORG","1","","A","CHECKING"]', 'two'],
        ['ofx:["BANK","ORG","1","","B","CHECKING"]', 'one'],
    ]);
    const first = filterDuplicateTransactions(incoming, [], 'local').unique;
    expect(first).toHaveLength(3);
    const existing: Transaction[] = first.map((transaction, index) => ({ ...transaction, id: `saved-${index}`, accountId: 'local', checked: true }));
    expect(filterDuplicateTransactions(incoming, existing, 'local')).toEqual({ unique: [], duplicateCount: 3 });
});

test('a duplicated FITID does not inflate the opening balance of a new account', () => {
    const parsed = parseOfxTransactions(document(statement('A', ['one', 'one'])));
    const openingSelection = prepareOfxImport(parsed, 'new', {}, 'category');
    const initialBalance = ofxInitialBalanceFromFinal(openingSelection, 100);
    const imported = filterDuplicateTransactions(prepareOfxImport(parsed, 'created', {}, 'category'), [], 'created').unique;
    expect(initialBalance).toBe(110);
    expect(imported).toHaveLength(1);
    expect(initialBalance + imported.reduce((sum, transaction) => sum + (transaction.type === 'income' ? transaction.amount : -transaction.amount), 0)).toBe(100);
});

test('distinct FITIDs and purchases without identity retain multiplicity in the opening balance', () => {
    const distinct = prepareOfxImport(parseOfxTransactions(document(statement('A', ['one', 'two']))), 'new', {}, 'category');
    expect(distinct).toHaveLength(2);
    expect(ofxInitialBalanceFromFinal(distinct, 100)).toBe(120);
    const anonymous = [{ date: '2026-10-03', amount: -0.1, description: 'Achat fictif', category: 'source' }, { date: '2026-10-03', amount: -0.2, description: 'Achat fictif', category: 'source' }];
    const mapped = prepareOfxImport([...anonymous, anonymous[0]], 'new', { source: 'mapped-category' }, 'fallback');
    expect(mapped).toHaveLength(3);
    expect(mapped.every(transaction => transaction.category === 'mapped-category')).toBe(true);
    expect(ofxInitialBalanceFromFinal(mapped, 100)).toBe(100.4);
});
