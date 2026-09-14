import React, { useState, useMemo, useCallback, useEffect } from 'react';
import { Plus, Search, Trash2, Edit2, CheckCircle2, ArrowRightLeft, Tag, Circle } from 'lucide-react';
import Button from '../components/ui/Button';
import { useBank } from '../context/BankContext';
import { useToast } from '../context/ToastContext';
import FormPopup from '../components/ui/FormPopup';
import ConfirmModal from '../components/ui/ConfirmModal';
import SearchableSelect from '../components/ui/SearchableSelect';
import MultiSelect from '../components/ui/MultiSelect';
import { Transaction } from '../types';
import { ICONS } from '../constants/icons';
import Table from '../components/ui/Table';
import Input from '../components/ui/Input';
import { useFinancialMetrics } from '../hooks/useFinancialMetrics';
import { formatCurrency, formatDate } from '../utils/format';

type TransactionWithBalance = Transaction & { balance: number };

const normalizeSearchValue = (value: unknown) => String(value ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');

const getMonthKey = (date: string) => date.slice(0, 7);
const getBudgetSpendKey = (category: string, accountId: string | undefined, monthKey: string) => `${category}|${accountId || 'all'}|${monthKey}`;

const TRANSACTION_TYPE_FILTER_OPTIONS = [
    { id: 'expense', label: 'Dépenses', icon: 'TrendingDown', color: '#ef4444' },
    { id: 'income', label: 'Revenus', icon: 'TrendingUp', color: '#10b981' },
    { id: 'transfer', label: 'Virements', icon: 'ArrowRightLeft', color: '#6366f1' }
];

const TRANSACTION_STATUS_FILTER_OPTIONS = [
    { id: 'checked', label: 'Pointées', icon: 'CheckCircle2', color: '#10b981' },
    { id: 'unchecked', label: 'Non pointées', icon: 'Circle', color: '#94a3b8' }
];

const TRANSACTION_BUDGET_FILTER_OPTIONS = [
    { id: 'budgeted', label: 'Avec budget', icon: 'Tag', color: '#6366f1' },
    { id: 'unbudgeted', label: 'Hors budget', icon: 'Tag', color: '#94a3b8' }
];

const Transactions: React.FC = () => {
    const {
        accounts,
        transactions,
        categories,
        addTransaction,
        addTransfer,
        updateTransaction,
        deleteTransaction,
        toggleTransactionCheck,
        processDueScheduledTransactions,
        filterAccount,
        budgets
    } = useBank();

    const { showToast } = useToast();
    const { relevantTransactions } = useFinancialMetrics();

    const [searchTerm, setSearchTerm] = useState('');
    const [filterCategories, setFilterCategories] = useState<string[]>([]);
    const [filterTypes, setFilterTypes] = useState<string[]>([]);
    const [filterStatuses, setFilterStatuses] = useState<string[]>([]);
    const [filterBudgets, setFilterBudgets] = useState<string[]>([]);
    const [isModalOpen, setIsModalOpen] = useState(false);
    const [editingTransaction, setEditingTransaction] = useState<Transaction | null>(null);
    const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
    const [transactionToDelete, setTransactionToDelete] = useState<string | null>(null);
    
    // Selection state
    const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
    const [isGroupDeleteModalOpen, setIsGroupDeleteModalOpen] = useState(false);

    const [formData, setFormData] = useState({
        date: new Date().toISOString().split('T')[0],
        description: '',
        amount: '',
        type: 'expense' as any,
        categoryId: '',
        accountId: '',
        toAccountId: '',
        isTransfer: false
    });

    useEffect(() => {
        processDueScheduledTransactions();
    }, [processDueScheduledTransactions]);

    const accountMap = useMemo(() => new Map(accounts.map(account => [account.id, account])), [accounts]);
    const categoryMap = useMemo(() => new Map(categories.map(category => [category.id, category])), [categories]);

    const getCategoryDetails = useCallback((id: string) => {
        if (id === 'transfer') return { name: 'Virement', color: '#6366f1', icon: 'ArrowRightLeft' };
        return categoryMap.get(id) || { name: 'Inconnu', color: '#9ca3af', icon: 'Tag' };
    }, [categoryMap]);

    const budgetSpentByScope = useMemo(() => {
        return transactions.reduce((acc, transaction) => {
            if (transaction.type !== 'expense' || transaction.category === 'transfer') return acc;

            const monthKey = getMonthKey(transaction.date);
            const categoryKey = getBudgetSpendKey(transaction.category, undefined, monthKey);
            const accountKey = getBudgetSpendKey(transaction.category, transaction.accountId, monthKey);

            acc.set(categoryKey, (acc.get(categoryKey) || 0) + transaction.amount);
            acc.set(accountKey, (acc.get(accountKey) || 0) + transaction.amount);
            return acc;
        }, new Map<string, number>());
    }, [transactions]);

    const getApplicableBudget = useCallback((transaction: Transaction) => {
        if (transaction.type !== 'expense' || transaction.category === 'transfer') return undefined;

        const matchingBudgets = budgets.filter(budget => (
            budget.category === transaction.category && (!budget.accountId || budget.accountId === transaction.accountId)
        ));

        return matchingBudgets.find(budget => budget.accountId === transaction.accountId)
            || matchingBudgets.find(budget => !budget.accountId);
    }, [budgets]);

    const getTransactionBudgetRemaining = useCallback((transaction: Transaction) => {
        const budget = getApplicableBudget(transaction);
        if (!budget) return null;

        const monthKey = getMonthKey(transaction.date);
        const spent = budgetSpentByScope.get(getBudgetSpendKey(budget.category, budget.accountId, monthKey)) || 0;

        return {
            budget,
            remaining: budget.amount - spent
        };
    }, [budgetSpentByScope, getApplicableBudget]);

    const isTransactionBudgeted = useCallback((transaction: Transaction) => (
        !!getApplicableBudget(transaction)
    ), [getApplicableBudget]);

    const categoryFilterOptions = useMemo(() => [
        { id: 'transfer', label: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' },
        ...categories
            .filter(category => category.id !== 'transfer')
            .map(category => ({ id: category.id, label: category.name, icon: category.icon, color: category.color }))
    ], [categories]);

    const transactionsWithBalance = useMemo<TransactionWithBalance[]>(() => {
        const allSorted = transactions
            .map((transaction, index) => ({ transaction, index }))
            .sort((a, b) => {
                const dateDiff = new Date(a.transaction.date).getTime() - new Date(b.transaction.date).getTime();
                return dateDiff || b.index - a.index;
            })
            .map(({ transaction }) => transaction);

        const accountBalances: Record<string, number> = {};
        accounts.forEach(acc => accountBalances[acc.id] = acc.initialBalance);

        const withBalance = allSorted.map(t => {
            const currentBal = accountBalances[t.accountId] || 0;
            const newBal = currentBal + (t.type === 'income' ? t.amount : -t.amount);
            accountBalances[t.accountId] = newBal;
            return { ...t, balance: newBal };
        });

        return withBalance.reverse();
    }, [transactions, accounts]);

    const displayTransactions = useMemo(() => {
        const relevantIds = new Set(relevantTransactions.map(transaction => transaction.id));
        const searchTokens = normalizeSearchValue(searchTerm).split(/\s+/).filter(Boolean);

        return transactionsWithBalance.filter(transaction => {
            if (!relevantIds.has(transaction.id)) return false;
            if (filterCategories.length > 0 && !filterCategories.includes(transaction.category)) return false;

            const transactionType = transaction.category === 'transfer' ? 'transfer' : transaction.type;
            if (filterTypes.length > 0 && !filterTypes.includes(transactionType)) return false;

            const transactionStatus = transaction.checked ? 'checked' : 'unchecked';
            if (filterStatuses.length > 0 && !filterStatuses.includes(transactionStatus)) return false;

            const transactionBudgetStatus = isTransactionBudgeted(transaction) ? 'budgeted' : 'unbudgeted';
            if (filterBudgets.length > 0 && !filterBudgets.includes(transactionBudgetStatus)) return false;

            if (searchTokens.length === 0) return true;

            const account = accountMap.get(transaction.accountId);
            const category = getCategoryDetails(transaction.category);
            const typeLabel = transaction.category === 'transfer'
                ? 'Virement'
                : transaction.type === 'income'
                    ? 'Revenu'
                    : 'Dépense';
            const amountPrefix = transaction.type === 'income' ? '+' : '-';
            const checkedLabel = transaction.checked ? 'Pointé coché validé' : 'Non pointé non coché actuel';
            const budgetRemaining = getTransactionBudgetRemaining(transaction);
            const budgetLabel = budgetRemaining
                ? `Budget budgété prévu ${budgetRemaining.budget.name} ${formatCurrency(budgetRemaining.remaining)} restant`
                : 'Hors budget non budgété';
            const searchableText = [
                account?.name,
                account?.type,
                transaction.date,
                formatDate(transaction.date),
                formatDate(transaction.date, 'dd MMM'),
                category.name,
                typeLabel,
                transaction.description,
                transaction.amount,
                formatCurrency(transaction.amount),
                `${amountPrefix}${formatCurrency(transaction.amount)}`,
                checkedLabel,
                budgetLabel,
                transaction.balance,
                formatCurrency(transaction.balance)
            ].map(normalizeSearchValue).join(' ');

            return searchTokens.every(token => searchableText.includes(token));
        });
    }, [
        relevantTransactions,
        transactionsWithBalance,
        filterCategories,
        filterTypes,
        filterStatuses,
        filterBudgets,
        searchTerm,
        accountMap,
        getCategoryDetails,
        isTransactionBudgeted,
        getTransactionBudgetRemaining
    ]);

    const mobileGroupedTransactions = useMemo(() => {
        const groups = new Map<string, TransactionWithBalance[]>();
        displayTransactions.forEach(transaction => {
            const key = transaction.date;
            groups.set(key, [...(groups.get(key) || []), transaction]);
        });

        return Array.from(groups.entries()).map(([date, items]) => ({
            date,
            label: formatDate(date, 'EEEE d MMM'),
            items
        }));
    }, [displayTransactions]);

    const activeFilterCount = filterCategories.length + filterTypes.length + filterStatuses.length + filterBudgets.length;
    const mobileVisibleNet = useMemo(() => displayTransactions.reduce((sum, transaction) => (
        sum + (transaction.type === 'income' ? transaction.amount : -transaction.amount)
    ), 0), [displayTransactions]);

    const handleOpenModal = (transaction?: Transaction) => {
        if (transaction) {
            setEditingTransaction(transaction);
            
            let toAccountId = '';
            let type = transaction.type;
            
            if (transaction.category === 'transfer' && transaction.linkedTransactionId) {
                type = 'transfer' as any;
                const linkedTx = transactions.find(t => t.id === transaction.linkedTransactionId);
                if (linkedTx) {
                    if (transaction.type === 'expense') {
                        toAccountId = linkedTx.accountId;
                    } else {
                        toAccountId = transaction.accountId;
                    }
                }
            }

            setFormData({
                date: transaction.date,
                description: transaction.description,
                amount: transaction.amount.toString(),
                type: type,
                categoryId: transaction.category,
                accountId: transaction.type === 'income' && transaction.category === 'transfer' && transaction.linkedTransactionId 
                    ? (transactions.find(t => t.id === transaction.linkedTransactionId)?.accountId || transaction.accountId) 
                    : transaction.accountId,
                toAccountId: toAccountId,
                isTransfer: !!transaction.linkedTransactionId
            });
        } else {
            setEditingTransaction(null);
            setFormData({
                date: new Date().toISOString().split('T')[0],
                description: '',
                amount: '',
                type: 'expense',
                categoryId: '',
                accountId: filterAccount.length === 1 ? filterAccount[0] : accounts[0]?.id || '',
                toAccountId: '',
                isTransfer: false
            });
        }
        setIsModalOpen(true);
    };

    const handleConfirmDelete = async () => {
        if (transactionToDelete) {
            try {
                await deleteTransaction(transactionToDelete);
                showToast("Transaction supprimée", "success");
                setTransactionToDelete(null);
            } catch (e) {
                showToast("Erreur lors de la suppression", "error");
            }
        }
        setIsDeleteModalOpen(false);
    };

    const handleToggleSelect = useCallback((id: string | number) => {
        setSelectedIds(prev => {
            const next = new Set(prev);
            if (next.has(id as string)) next.delete(id as string);
            else next.add(id as string);
            return next;
        });
    }, []);

    const handleSelectAll = useCallback(() => {
        if (selectedIds.size === displayTransactions.length) {
            setSelectedIds(new Set());
        } else {
            setSelectedIds(new Set(displayTransactions.map(t => t.id)));
        }
    }, [selectedIds.size, displayTransactions]);

    const handleGroupDelete = async () => {
        const count = selectedIds.size;
        try {
            await Promise.all(Array.from(selectedIds).map(id => deleteTransaction(id)));
            setSelectedIds(new Set());
            showToast(`${count} transactions supprimées`, "success");
        } catch (e) {
            showToast("Erreur lors de la suppression groupée", "error");
        }
        setIsGroupDeleteModalOpen(false);
    };

    const handleGroupCheck = async () => {
        const count = selectedIds.size;
        try {
            // Determine the next state (if all checked, uncheck all, otherwise check all)
            const selectedTxs = transactions.filter(t => selectedIds.has(t.id));
            const allChecked = selectedTxs.every(t => t.checked);
            
            for (const id of Array.from(selectedIds)) {
                const tx = selectedTxs.find(t => t.id === id);
                if (tx && tx.checked === allChecked) {
                    await toggleTransactionCheck(id);
                }
            }
            showToast(`${count} transactions mises à jour`, "success");
        } catch (e) {
            showToast("Erreur lors de la mise à jour groupée", "error");
        }
    };

    const handleCellUpdate = async (transaction: any, accessor: any, newValue: any) => {
        try {
            let updatedValue = newValue;
            
            // Validation simple
            if (accessor === 'amount') {
                updatedValue = parseFloat(String(newValue).replace(',', '.'));
                if (isNaN(updatedValue)) return;
            }
            
            if (transaction[accessor] === updatedValue) return;

            await updateTransaction({
                ...transaction,
                [accessor]: updatedValue
            });
            showToast("Transaction mise à jour", "success");
        } catch (e) {
            showToast("Erreur lors de la mise à jour", "error");
        }
    };

    const renderCategoryIcon = (iconName: string, className: string = "w-4 h-4") => {
        if (iconName === 'ArrowRightLeft') return <ArrowRightLeft className={className} />;
        const Icon = ICONS[iconName] || Tag;
        return <Icon className={className} />;
    };

    return (
        <div className="flex-1 flex flex-col min-h-0 space-y-4 md:space-y-6">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-1 flex-none w-full">
                <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-200">Journal</h2>
                <Button 
                    onClick={() => handleOpenModal()} 
                    size="sm" 
                    icon={Plus}
                    className="w-full sm:w-auto"
                >
                    Nouvelle transaction
                </Button>
            </div>

            <div className="md:hidden space-y-3">
                <div className="grid grid-cols-3 gap-2">
                    <div className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] p-3">
                        <div className="text-[10px] font-bold uppercase tracking-wider text-gray-400">Lignes</div>
                        <div className="mt-1 text-lg font-bold text-gray-950 dark:text-white">{displayTransactions.length}</div>
                    </div>
                    <div className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] p-3">
                        <div className="text-[10px] font-bold uppercase tracking-wider text-gray-400">Filtres</div>
                        <div className="mt-1 text-lg font-bold text-gray-950 dark:text-white">{activeFilterCount}</div>
                    </div>
                    <div className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] p-3">
                        <div className="text-[10px] font-bold uppercase tracking-wider text-gray-400">Net</div>
                        <div className={`mt-1 text-lg font-bold truncate ${mobileVisibleNet >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                            {mobileVisibleNet >= 0 ? '+' : ''}{formatCurrency(mobileVisibleNet)}
                        </div>
                    </div>
                </div>

                <Input
                    placeholder="Rechercher..."
                    value={searchTerm}
                    onChange={(e) => setSearchTerm(e.target.value)}
                    icon={Search}
                />

                <div className="grid grid-cols-2 gap-2">
                    <MultiSelect
                        value={filterCategories}
                        onChange={setFilterCategories}
                        options={categoryFilterOptions}
                        placeholder="Catégories"
                        className="w-full min-w-0"
                        size="sm"
                    />
                    <MultiSelect
                        value={filterTypes}
                        onChange={setFilterTypes}
                        options={TRANSACTION_TYPE_FILTER_OPTIONS}
                        placeholder="Types"
                        className="w-full min-w-0"
                        size="sm"
                    />
                    <MultiSelect
                        value={filterStatuses}
                        onChange={setFilterStatuses}
                        options={TRANSACTION_STATUS_FILTER_OPTIONS}
                        placeholder="États"
                        className="w-full min-w-0"
                        size="sm"
                    />
                    <MultiSelect
                        value={filterBudgets}
                        onChange={setFilterBudgets}
                        options={TRANSACTION_BUDGET_FILTER_OPTIONS}
                        placeholder="Budgets"
                        className="w-full min-w-0"
                        size="sm"
                    />
                </div>
            </div>

            <div className="hidden md:flex flex-col xl:flex-row gap-3 px-1 flex-none">
                <div className="w-full xl:max-w-sm xl:flex-none">
                    <Input
                        placeholder="Rechercher dans toutes les colonnes..."
                        value={searchTerm}
                        onChange={(e) => setSearchTerm(e.target.value)}
                        icon={Search}
                    />
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-3 w-full xl:flex-1 min-w-0">
                    <MultiSelect
                        value={filterCategories}
                        onChange={setFilterCategories}
                        options={categoryFilterOptions}
                        placeholder="Toutes les catégories"
                        className="w-full min-w-0"
                    />
                    <MultiSelect
                        value={filterTypes}
                        onChange={setFilterTypes}
                        options={TRANSACTION_TYPE_FILTER_OPTIONS}
                        placeholder="Tous les types"
                        className="w-full min-w-0"
                    />
                    <MultiSelect
                        value={filterStatuses}
                        onChange={setFilterStatuses}
                        options={TRANSACTION_STATUS_FILTER_OPTIONS}
                        placeholder="Tous les états"
                        className="w-full min-w-0"
                    />
                    <MultiSelect
                        value={filterBudgets}
                        onChange={setFilterBudgets}
                        options={TRANSACTION_BUDGET_FILTER_OPTIONS}
                        placeholder="Tous les budgets"
                        className="w-full min-w-0"
                    />
                </div>
            </div>

            {/* Selection Toolbar */}
            {selectedIds.size > 0 && (
                <div className="bg-primary-500/10 border border-primary-500/20 p-2 px-3 md:px-4 rounded-2xl md:rounded-xl flex flex-col md:flex-row md:items-center justify-between gap-2 animate-in fade-in slide-in-from-top-2 duration-150">
                    <div className="flex items-center gap-3 md:gap-4 text-sm font-semibold text-primary-700 dark:text-primary-400">
                        <span>{selectedIds.size} sélectionnée{selectedIds.size > 1 ? 's' : ''}</span>
                        <div className="h-4 w-px bg-primary-500/30"></div>
                        <div className="flex gap-2">
                            <button 
                                onClick={handleGroupCheck}
                                className="flex items-center gap-1.5 hover:text-primary-800 transition-colors"
                            >
                                <CheckCircle2 className="w-4 h-4" /> Pointer/Dépointer
                            </button>
                        </div>
                    </div>
                    <div className="flex justify-end gap-2">
                        <Button 
                            variant="ghost" 
                            size="sm" 
                            onClick={() => setSelectedIds(new Set())}
                            className="text-primary-700 dark:text-primary-400 hover:bg-primary-500/10"
                        >
                            Annuler
                        </Button>
                        <Button 
                            variant="danger" 
                            size="sm" 
                            icon={Trash2}
                            onClick={() => setIsGroupDeleteModalOpen(true)}
                        >
                            Supprimer
                        </Button>
                    </div>
                </div>
            )}

            <div className="hidden md:flex flex-1 bg-white dark:bg-[#121212] rounded-xl border border-black/[0.05] dark:border-white/10 shadow-sm overflow-hidden flex-col min-h-[calc(100vh-170px)] max-h-[calc(100vh-170px)]">
                <Table
                    data={displayTransactions}
                    keyExtractor={(t) => t.id}
                    selectedIds={selectedIds as any}
                    onSelectRow={handleToggleSelect}
                    onSelectAll={handleSelectAll}
                    isAllSelected={selectedIds.size > 0 && selectedIds.size === displayTransactions.length}
                    onCellUpdate={handleCellUpdate}
                    emptyMessage={
                        <div className="flex flex-col items-center gap-4 py-12 text-center">
                            <div className="w-20 h-20 rounded-full bg-gray-50 dark:bg-neutral-900 flex items-center justify-center">
                                <Search className="w-10 h-10 text-gray-300 dark:text-gray-700" />
                            </div>
                            <div>
                                <p className="text-gray-900 dark:text-gray-100 font-bold text-lg">Aucune transaction</p>
                                <p className="text-gray-500 dark:text-gray-400 text-sm max-w-xs">
                                    {searchTerm || filterCategories.length > 0 || filterTypes.length > 0 || filterStatuses.length > 0 || filterBudgets.length > 0
                                        ? "Aucun résultat pour vos filtres actuels."
                                        : "Commencez par ajouter une transaction ou importez un relevé bancaire."}
                                </p>
                            </div>
                            {!searchTerm && filterCategories.length === 0 && filterTypes.length === 0 && filterStatuses.length === 0 && filterBudgets.length === 0 && (
                                <Button onClick={() => handleOpenModal()} size="sm" icon={Plus}>Ajouter une transaction</Button>
                            )}
                        </div>
                    }
                    columns={[
                        {
                            header: 'Compte',
                            width: '120px',
                            accessor: 'accountId',
                            truncate: true,
                            render: (t) => {
                                const acc = accounts.find(a => a.id === t.accountId);
                                return (
                                    <div className="flex items-center gap-2">
                                        <div className="w-1 h-4 rounded-full flex-none" style={{ backgroundColor: acc?.color || '#eee' }} />
                                        <span className="font-medium truncate">{acc?.name}</span>
                                    </div>
                                );
                            }
                        },
                        {
                            header: 'Date',
                            width: '90px',
                            render: (t) => <span className="text-gray-500">{formatDate(t.date, 'dd MMM')}</span>
                        },
                        {
                            header: 'Catégorie',
                            width: '130px',
                            render: (t) => {
                                const cat = getCategoryDetails(t.category);
                                return (
                                    <div className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-tight" style={{ backgroundColor: `${cat.color}15`, color: cat.color }}>
                                        {renderCategoryIcon(cat.icon, "w-3 h-3")}
                                        <span className="truncate">{cat.name}</span>
                                    </div>
                                );
                            }
                        },
                        {
                            header: 'Description',
                            width: '1fr',
                            accessor: 'description',
                            truncate: true,
                            editable: true,
                            render: (t) => <span className="font-medium text-gray-900 dark:text-gray-100">{t.description}</span>
                        },
                        {
                            header: 'Montant',
                            width: '120px',
                            align: 'right',
                            accessor: 'amount',
                            className: "tabular-nums font-bold",
                            editable: true,
                            editType: 'number',
                            render: (t) => (
                                <span className={t.type === 'income' ? 'text-emerald-600' : 'text-red-600'}>
                                    {t.type === 'income' ? '+' : '-'}{new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2 }).format(t.amount)} €
                                </span>
                            )
                        },
                        {
                            header: 'Budget restant',
                            width: '140px',
                            align: 'right',
                            className: "tabular-nums",
                            render: (t) => {
                                const budgetRemaining = getTransactionBudgetRemaining(t);
                                if (!budgetRemaining) return null;

                                return (
                                    <span className="inline-flex items-center justify-end px-2 py-0.5 rounded text-[10px] font-bold text-indigo-500/70 border border-indigo-500/20 bg-indigo-500/[0.03] whitespace-nowrap">
                                        {formatCurrency(budgetRemaining.remaining)}
                                    </span>
                                );
                            }
                        },
                        {
                            header: 'État',
                            width: '60px',
                            align: 'center',
                            render: (t) => (
                                <button onClick={() => toggleTransactionCheck(t.id)} className={`transition-colors ${t.checked ? 'text-emerald-500' : 'text-gray-300 dark:text-gray-600 hover:text-gray-400'}`}>
                                    {t.checked ? <CheckCircle2 className="w-5 h-5" /> : <Circle className="w-5 h-5" />}
                                </button>
                            )
                        },
                        {
                            header: 'Solde',
                            width: '120px',
                            align: 'right',
                            className: "tabular-nums text-gray-400 opacity-60",
                            render: (t) => <span>{formatCurrency(t.balance)}</span>
                        },
                        {
                            header: '',
                            width: '80px',
                            align: 'right',
                            render: (t) => (
                                <div className="flex gap-1 justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                                    <Button variant="ghost" size="sm" icon={Edit2} onClick={() => handleOpenModal(t)} className="h-8 w-8 p-0" />
                                    <Button variant="ghost" size="sm" icon={Trash2} onClick={() => { setTransactionToDelete(t.id); setIsDeleteModalOpen(true); }} className="h-8 w-8 p-0 text-red-400 hover:text-red-600" />
                                </div>
                            )
                        }
                    ]}
                />
            </div>

            <div className="md:hidden space-y-3 pb-4">
                {displayTransactions.length > 0 ? (
                    mobileGroupedTransactions.map(group => (
                        <section key={group.date} className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] shadow-sm overflow-hidden">
                            <div className="px-4 py-2.5 bg-gray-50 dark:bg-white/[0.03] border-b border-black/[0.04] dark:border-white/10">
                                <h3 className="text-[11px] font-bold uppercase tracking-[0.14em] text-gray-400 dark:text-neutral-500 capitalize">{group.label}</h3>
                            </div>
                            <div className="divide-y divide-gray-100 dark:divide-neutral-800">
                                {group.items.map(transaction => {
                                    const account = accountMap.get(transaction.accountId);
                                    const category = getCategoryDetails(transaction.category);
                                    const budgetRemaining = getTransactionBudgetRemaining(transaction);
                                    const isIncome = transaction.type === 'income';
                                    const isSelected = selectedIds.has(transaction.id);

                                    return (
                                        <article key={transaction.id} className={`p-3 transition-colors ${isSelected ? 'bg-primary-50/60 dark:bg-primary-500/10' : ''}`}>
                                            <div className="flex items-start gap-3">
                                                <input
                                                    type="checkbox"
                                                    aria-label={`Sélectionner ${transaction.description}`}
                                                    className="mt-4 w-4 h-4 rounded border-gray-300 dark:border-neutral-700 text-primary-600 focus:ring-primary-500 bg-white dark:bg-neutral-900"
                                                    checked={isSelected}
                                                    onChange={() => handleToggleSelect(transaction.id)}
                                                />

                                                <button
                                                    type="button"
                                                    onClick={() => handleOpenModal(transaction)}
                                                    className="min-w-0 flex-1 text-left"
                                                >
                                                    <div className="flex items-start gap-3">
                                                        <div
                                                            className="mt-0.5 h-10 w-10 shrink-0 rounded-2xl flex items-center justify-center"
                                                            style={{ backgroundColor: `${category.color}16`, color: category.color }}
                                                        >
                                                            {renderCategoryIcon(category.icon, 'w-5 h-5')}
                                                        </div>
                                                        <div className="min-w-0 flex-1">
                                                            <div className="flex items-start justify-between gap-2">
                                                                <p className="text-[15px] font-semibold leading-tight text-gray-950 dark:text-white truncate">
                                                                    {transaction.description || category.name}
                                                                </p>
                                                                <p className={`shrink-0 text-[15px] font-bold tabular-nums ${isIncome ? 'text-emerald-600' : 'text-red-600'}`}>
                                                                    {isIncome ? '+' : '-'}{formatCurrency(transaction.amount)}
                                                                </p>
                                                            </div>
                                                            <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[12px] text-gray-500 dark:text-neutral-400">
                                                                <span className="truncate max-w-[120px]">{account?.name || 'Compte'}</span>
                                                                <span className="h-1 w-1 rounded-full bg-gray-300 dark:bg-neutral-700" />
                                                                <span className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-tight" style={{ backgroundColor: `${category.color}14`, color: category.color }}>
                                                                    {category.name}
                                                                </span>
                                                                {budgetRemaining && (
                                                                    <span className="text-[11px] font-semibold text-indigo-500">
                                                                        {formatCurrency(budgetRemaining.remaining)} restant
                                                                    </span>
                                                                )}
                                                            </div>
                                                        </div>
                                                    </div>
                                                </button>

                                                <div className="flex shrink-0 flex-col items-center gap-1">
                                                    <button
                                                        type="button"
                                                        onClick={() => toggleTransactionCheck(transaction.id)}
                                                        className={`rounded-full p-1.5 transition-colors ${transaction.checked ? 'text-emerald-500 bg-emerald-50 dark:bg-emerald-500/10' : 'text-gray-300 dark:text-neutral-600 bg-gray-50 dark:bg-white/[0.04]'}`}
                                                        aria-label={transaction.checked ? 'Dépointer' : 'Pointer'}
                                                    >
                                                        {transaction.checked ? <CheckCircle2 className="w-4 h-4" /> : <Circle className="w-4 h-4" />}
                                                    </button>
                                                    <button
                                                        type="button"
                                                        onClick={() => handleOpenModal(transaction)}
                                                        className="rounded-full p-1.5 text-gray-400 bg-gray-50 dark:bg-white/[0.04] dark:text-neutral-500"
                                                        aria-label="Modifier"
                                                    >
                                                        <Edit2 className="w-4 h-4" />
                                                    </button>
                                                    <button
                                                        type="button"
                                                        onClick={() => {
                                                            setTransactionToDelete(transaction.id);
                                                            setIsDeleteModalOpen(true);
                                                        }}
                                                        className="rounded-full p-1.5 text-red-500 bg-red-50 dark:bg-red-500/10"
                                                        aria-label="Supprimer"
                                                    >
                                                        <Trash2 className="w-4 h-4" />
                                                    </button>
                                                </div>
                                            </div>
                                        </article>
                                    );
                                })}
                            </div>
                        </section>
                    ))
                ) : (
                    <div className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] p-8 text-center shadow-sm">
                        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-gray-50 dark:bg-neutral-900">
                            <Search className="h-8 w-8 text-gray-300 dark:text-neutral-700" />
                        </div>
                        <p className="text-base font-bold text-gray-950 dark:text-white">Aucune transaction</p>
                        <p className="mt-1 text-sm text-gray-500 dark:text-neutral-400">
                            {searchTerm || activeFilterCount > 0
                                ? 'Aucun résultat pour les filtres actuels.'
                                : 'Ajoute une transaction pour commencer.'}
                        </p>
                    </div>
                )}
            </div>

            <FormPopup isOpen={isModalOpen} onClose={() => setIsModalOpen(false)}>
                <form onSubmit={async (e) => {
                    e.preventDefault();
                    try {
                        const amount = parseFloat(formData.amount);
                        const isTransfer = formData.type === 'transfer';
                        if (editingTransaction) {
                            const transactionData = {
                                date: formData.date,
                                amount,
                                description: formData.description,
                                category: isTransfer ? 'transfer' : formData.categoryId,
                                accountId: editingTransaction.type === 'income' && isTransfer ? (formData.toAccountId || formData.accountId) : formData.accountId,
                                type: isTransfer ? editingTransaction.type : formData.type as 'income' | 'expense',
                            };
                            await updateTransaction({ ...editingTransaction, ...transactionData });
                            
                            // Mettre à jour la transaction liée si elle existe
                            if (isTransfer && editingTransaction.linkedTransactionId) {
                                const linkedTx = transactions.find(t => t.id === editingTransaction.linkedTransactionId);
                                if (linkedTx) {
                                    await updateTransaction({
                                        ...linkedTx,
                                        date: formData.date,
                                        amount,
                                        description: formData.description,
                                        accountId: formData.toAccountId || linkedTx.accountId,
                                    });
                                }
                            }
                            
                            showToast("Transaction mise à jour", "success");
                        } else {
                            if (isTransfer && formData.toAccountId) {
                                await addTransfer(formData.accountId, formData.toAccountId, amount, formData.date, formData.description);
                                showToast("Virement ajouté", "success");
                            } else {
                                const transactionData = {
                                    date: formData.date,
                                    amount,
                                    description: formData.description,
                                    category: formData.categoryId,
                                    accountId: formData.accountId,
                                    type: formData.type as 'income' | 'expense',
                                    checked: false
                                };
                                await addTransaction(transactionData);
                                showToast("Transaction ajoutée", "success");
                            }
                        }
                        setIsModalOpen(false);
                    } catch (err) {
                        showToast("Une erreur est survenue", "error");
                    }
                }} className="p-6 space-y-6">
                    <h3 className="text-lg font-semibold">{editingTransaction ? "Modifier" : "Nouvelle"} transaction</h3>
                    <div className="space-y-4">
                        <Input label="Description" required value={formData.description} onChange={e => setFormData({ ...formData, description: e.target.value })} placeholder="Ex: Loyer" />
                        
                        <div className="grid grid-cols-2 gap-4">
                            <Input label="Montant" type="number" step="0.01" required value={formData.amount} onChange={e => setFormData({ ...formData, amount: e.target.value })} rightElement="€" placeholder="0.00" />
                            <SearchableSelect
                                label="Type"
                                value={formData.type}
                                onChange={(value) => setFormData({ ...formData, type: value })}
                                options={[
                                    { id: 'expense', label: 'Dépense', icon: 'TrendingDown', color: '#ef4444' },
                                    { id: 'income', label: 'Revenu', icon: 'TrendingUp', color: '#10b981' },
                                    { id: 'transfer', label: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' }
                                ]}
                                placeholder="Sélectionner un type"
                            />
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {formData.type === 'transfer' ? 'Compte source' : 'Compte'}
                                </label>
                                <SearchableSelect
                                    value={formData.accountId}
                                    onChange={(value) => setFormData({ ...formData, accountId: value })}
                                    options={accounts.map(acc => ({
                                        id: acc.id,
                                        label: acc.name,
                                        icon: acc.icon || 'Wallet',
                                        color: acc.color
                                    }))}
                                    placeholder="Sélectionner un compte"
                                />
                            </div>
                            <Input label="Date" type="date" required value={formData.date} onChange={e => setFormData({ ...formData, date: e.target.value })} />
                        </div>

                        <div>
                            {formData.type === 'transfer' ? (
                                <>
                                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Compte destination</label>
                                    <SearchableSelect
                                        value={formData.toAccountId || ''}
                                        onChange={(value) => setFormData({ ...formData, toAccountId: value })}
                                        options={accounts
                                            .filter(acc => acc.id !== formData.accountId)
                                            .map(acc => ({
                                                id: acc.id,
                                                label: acc.name,
                                                icon: acc.icon || 'Wallet',
                                                color: acc.color
                                            }))}
                                        placeholder="Sélectionner un compte"
                                    />
                                </>
                            ) : (
                                <SearchableSelect 
                                    label="Catégorie" 
                                    value={formData.categoryId} 
                                    onChange={val => setFormData({ ...formData, categoryId: val })} 
                                    options={categories.filter(c => c.id !== 'transfer').map(c => ({ id: c.id, label: c.name, icon: c.icon, color: c.color }))} 
                                />
                            )}
                        </div>

                        <div className="flex gap-3 pt-4">
                            <Button type="button" variant="secondary" fullWidth onClick={() => setIsModalOpen(false)}>Annuler</Button>
                            <Button type="submit" fullWidth>Enregistrer</Button>
                        </div>
                    </div>
                </form>
            </FormPopup>

            <ConfirmModal
                isOpen={isDeleteModalOpen}
                onClose={() => setIsDeleteModalOpen(false)}
                onConfirm={handleConfirmDelete}
                title="Supprimer"
                message="Voulez-vous vraiment supprimer cette transaction ?"
                confirmLabel="Supprimer"
                isDangerous
            />

            <ConfirmModal
                isOpen={isGroupDeleteModalOpen}
                onClose={() => setIsGroupDeleteModalOpen(false)}
                onConfirm={handleGroupDelete}
                title="Supprimer la sélection"
                message={`Voulez-vous vraiment supprimer les ${selectedIds.size} transactions sélectionnées ?`}
                confirmLabel="Tout supprimer"
                isDangerous
            />
        </div>
    );
};

export default Transactions;
