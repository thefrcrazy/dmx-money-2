import {
    filterDuplicateTransactions,
    type ImportTransactionInput,
    type ParsedStatementTransaction,
} from '../../utils/importParsers';

export const prepareOfxImport = (
    transactions: ParsedStatementTransaction[],
    accountId: string,
    categoryMapping: Record<string, string>,
    fallbackCategory: string,
): ImportTransactionInput[] => {
    const mapped = transactions.map(transaction => ({
        date: transaction.date,
        amount: Math.abs(transaction.amount),
        type: transaction.amount >= 0 ? 'income' as const : 'expense' as const,
        description: transaction.description,
        category: transaction.category ? categoryMapping[transaction.category] || fallbackCategory : fallbackCategory,
        accountId,
        checked: true,
        bankSource: transaction.bankSource,
        bankTransactionId: transaction.bankTransactionId,
    }));

    // Use the same effective selection for both the opening balance and import.
    // Without a bank identity, equal-valued purchases retain their multiplicity.
    return filterDuplicateTransactions(mapped, [], accountId).unique;
};

export const ofxInitialBalanceFromFinal = (
    transactions: ImportTransactionInput[],
    finalBalance: number,
): number => {
    const netCents = transactions.reduce((sum, transaction) =>
        sum + Math.round(transaction.amount * 100) * (transaction.type === 'income' ? 1 : -1), 0);
    return (Math.round(finalBalance * 100) - netCents) / 100;
};
