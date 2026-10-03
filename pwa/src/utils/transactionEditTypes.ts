import type { Transaction, TransactionType } from '../types';

type ExistingTransaction = Pick<Transaction, 'type' | 'category' | 'isTransfer' | 'linkedTransactionId'>;

export const TRANSACTION_CONVERSION_NOTICE = 'Pour convertir une opération simple en virement, ou l’inverse, utilisez l’application native.';

export const allowedTransactionFormTypes = (transaction: ExistingTransaction | null): readonly TransactionType[] => {
    if (!transaction) return ['expense', 'income', 'transfer'];
    // Preserve the transfer boundary even for incomplete legacy metadata.
    return transaction.linkedTransactionId || transaction.isTransfer || transaction.category === 'transfer' || transaction.type === 'transfer'
        ? ['transfer'] : ['expense', 'income'];
};

export const transactionFormTypeError = (transaction: ExistingTransaction | null, type: string): string | null => {
    const allowed = allowedTransactionFormTypes(transaction);
    if (allowed.includes(type as TransactionType)) return null;
    if (!transaction) return 'Sélectionnez un type valide : Dépense, Revenu ou Virement.';
    return `${allowed[0] === 'transfer'
        ? 'Ce virement doit conserver le type Virement.'
        : 'Cette opération doit conserver le type Dépense ou Revenu.'} ${TRANSACTION_CONVERSION_NOTICE}`;
};
