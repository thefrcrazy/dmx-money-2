import { useMemo } from 'react';
import { useBank } from '../context/BankContext';
import { isSameMonth } from 'date-fns';

export const useFinancialMetrics = () => {
    const { accounts, transactions, filterAccount } = useBank();

    const metrics = useMemo(() => {
        const now = new Date();
        const isAllAccounts = filterAccount.length === 0;

        // 1. Filter Transactions based on account selection
        const relevantTransactions = isAllAccounts
            ? transactions
            : transactions.filter(t => filterAccount.includes(t.accountId));

        // 2. Calculate Current Balance (Initial + Transactions)
        const currentBalance = accounts
            .filter(acc => isAllAccounts || filterAccount.includes(acc.id))
            .reduce((sum, acc) => sum + acc.initialBalance, 0) +
            relevantTransactions.reduce((sum, t) => {
                return sum + (t.type === 'income' ? t.amount : -t.amount);
            }, 0);

        // 3. Calculate Checked Balance (Only checked transactions)
        const checkedBalance = accounts
            .filter(acc => isAllAccounts || filterAccount.includes(acc.id))
            .reduce((sum, acc) => sum + acc.initialBalance, 0) +
            relevantTransactions
                .filter(t => t.checked)
                .reduce((sum, t) => {
                    return sum + (t.type === 'income' ? t.amount : -t.amount);
                }, 0);

        // 4. Monthly Stats (Income vs Expenses)
        const monthTransactions = relevantTransactions.filter(t => {
            try {
                return isSameMonth(new Date(t.date), now);
            } catch (e) {
                return false;
            }
        });
        
        const monthlyIncome = Math.round(monthTransactions
            .filter(t => t.type === 'income')
            .reduce((sum, t) => sum + (t.amount || 0), 0) * 100) / 100;

        const monthlyExpenses = Math.round(monthTransactions
            .filter(t => t.type === 'expense')
            .reduce((sum, t) => sum + (t.amount || 0), 0) * 100) / 100;

        const monthlySaved = Math.round((monthlyIncome - monthlyExpenses) * 100) / 100;

        return {
            currentBalance: Math.round(currentBalance * 100) / 100,
            checkedBalance: Math.round(checkedBalance * 100) / 100,
            monthlyIncome,
            monthlyExpenses,
            monthlySaved,
            relevantTransactions
        };
    }, [accounts, transactions, filterAccount]);

    return metrics;
};
