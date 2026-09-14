import React, { createContext, useContext, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { v4 as uuidv4 } from 'uuid';
import { Account, Transaction, Category, ScheduledTransaction, BankContextType, AppData, Budget } from '../types';
import { dbService } from '../services/db';
import { hasTauriRuntime, isMobileCompanion } from '../utils/runtime';

const BankContext = createContext<BankContextType | undefined>(undefined);
const SETTINGS_REFRESH_EVENT = 'dmxmoney-settings-refresh';

const notifySettingsRefresh = () => {
    window.dispatchEvent(new Event(SETTINGS_REFRESH_EVENT));
};

const DEFAULT_DATA: AppData = {
    accounts: [],
    transactions: [],
    categories: [
        // Logement
        { id: '1', name: 'Loyer / Prêt', icon: 'Home', color: '#1e3a8a' },
        { id: '2', name: 'Charges / Énergie', icon: 'Zap', color: '#f59e0b' },
        { id: '3', name: 'Eau', icon: 'Droplets', color: '#0ea5e9' },
        { id: '4', name: 'Assurance Habitation', icon: 'Shield', color: '#4b5563' },
        
        // Vie Quotidienne
        { id: '5', name: 'Alimentation', icon: 'ShoppingBag', color: '#ef4444' },
        { id: '6', name: 'Restaurants / Cafés', icon: 'Utensils', color: '#ea580c' },
        { id: '7', name: 'Shopping / Vêtements', icon: 'Tag', color: '#ec4899' },
        { id: '8', name: 'Hygiène / Beauté', icon: 'Smile', color: '#f472b6' },
        
        // Transport
        { id: '9', name: 'Carburant', icon: 'Fuel', color: '#b45309' },
        { id: '10', name: 'Transport en commun', icon: 'Bus', color: '#d97706' },
        { id: '11', name: 'Entretien Voiture', icon: 'Hammer', color: '#6b7280' },
        { id: '12', name: 'Parking / Péage', icon: 'MapPin', color: '#4b5563' },
        
        // Santé
        { id: '13', name: 'Médecin / Santé', icon: 'Heart', color: '#dc2626' },
        { id: '14', name: 'Pharmacie', icon: 'Pill', color: '#f87171' },
        
        // Loisirs & Culture
        { id: '15', name: 'Loisirs / Cinéma', icon: 'Gamepad2', color: '#8b5cf6' },
        { id: '16', name: 'Abonnements (VOD/Musique)', icon: 'Tv', color: '#6366f1' },
        { id: '17', name: 'Sport / Bien-être', icon: 'Dumbbell', color: '#06b6d4' },
        { id: '18', name: 'Voyages / Vacances', icon: 'Plane', color: '#2563eb' },
        
        // Technologie
        { id: '19', name: 'Téléphonie / Internet', icon: 'Wifi', color: '#3b82f6' },
        { id: '20', name: 'High-Tech / Logiciels', icon: 'Monitor', color: '#1e40af' },
        
        // Revenus
        { id: '21', name: 'Salaire', icon: 'Banknote', color: '#16a34a' },
        { id: '22', name: 'Primes / Bonus', icon: 'Award', color: '#d9f99d' },
        { id: '23', name: 'Cadeaux reçus', icon: 'Gift', color: '#db2777' },
        { id: '24', name: 'Remboursements', icon: 'TrendingUp', color: '#4ade80' },
        
        // Autre
        { id: '25', name: 'Cadeaux offerts', icon: 'Gift', color: '#fca5a5' },
        { id: '26', name: 'Frais Bancaires', icon: 'Landmark', color: '#1f2937' },
        { id: '27', name: 'Impôts / Taxes', icon: 'Briefcase', color: '#7c2d12' },
        { id: '28', name: 'Divers', icon: 'MoreHorizontal', color: '#6b7280' },
        
        { id: 'transfer', name: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' }
    ],
    scheduled: [],
    budgets: []
};

interface ScheduledProcessingResult {
    processedScheduled: ScheduledTransaction[];
    newTransactions: Transaction[];
    hasScheduledChanges: boolean;
}

const getScheduledTransactionId = (
    scheduledId: string,
    occurrenceDate: string,
    side: 'single' | 'from' | 'to',
) => `scheduled:${scheduledId}:${occurrenceDate}:${side}`;

const addMonths = (date: Date, months: number) => {
    const nextDate = new Date(date);
    nextDate.setMonth(nextDate.getMonth() + months);
    return nextDate;
};

const getNextScheduledDate = (date: Date, frequency: ScheduledTransaction['frequency']) => {
    const currentDate = new Date(date);

    switch (frequency) {
        case 'once':
            return new Date(currentDate.setFullYear(currentDate.getFullYear() + 100));
        case 'daily':
            return new Date(currentDate.setDate(currentDate.getDate() + 1));
        case 'weekly':
            return new Date(currentDate.setDate(currentDate.getDate() + 7));
        case 'biweekly':
            return new Date(currentDate.setDate(currentDate.getDate() + 14));
        case 'bimonthly':
            return new Date(currentDate.setDate(currentDate.getDate() + 15));
        case 'fourweekly':
            return new Date(currentDate.setDate(currentDate.getDate() + 28));
        case 'monthly':
            return addMonths(currentDate, 1);
        case 'bimestrial':
            return addMonths(currentDate, 2);
        case 'quarterly':
            return addMonths(currentDate, 3);
        case 'fourmonthly':
            return addMonths(currentDate, 4);
        case 'semiannual':
            return addMonths(currentDate, 6);
        case 'annual':
            return new Date(currentDate.setFullYear(currentDate.getFullYear() + 1));
        case 'biennial':
            return new Date(currentDate.setFullYear(currentDate.getFullYear() + 2));
        default:
            return addMonths(currentDate, 1);
    }
};

const processDueScheduledItems = async (
    sourceScheduled: ScheduledTransaction[],
    existingTransactions: Transaction[] = [],
): Promise<ScheduledProcessingResult> => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    const processedScheduled = sourceScheduled.map(scheduledTx => ({ ...scheduledTx }));
    const newTransactions: Transaction[] = [];
    const existingTransactionIds = new Set(existingTransactions.map(transaction => transaction.id));
    let hasScheduledChanges = false;

    for (let i = 0; i < processedScheduled.length; i++) {
        const scheduledTx = processedScheduled[i];
        let nextDate = new Date(scheduledTx.nextDate);
        nextDate.setHours(0, 0, 0, 0);

        let modified = false;

        while (nextDate <= today) {
            modified = true;

            if (scheduledTx.endDate) {
                const endDate = new Date(scheduledTx.endDate);
                endDate.setHours(0, 0, 0, 0);
                if (nextDate > endDate) {
                    break;
                }
            }

            const occurrenceDate = scheduledTx.nextDate;

            if (scheduledTx.type === 'transfer' && scheduledTx.toAccountId) {
                const txId = getScheduledTransactionId(scheduledTx.id, occurrenceDate, 'from');
                const linkedId = getScheduledTransactionId(scheduledTx.id, occurrenceDate, 'to');

                const sourceTx: Transaction = {
                    id: txId,
                    date: occurrenceDate,
                    accountId: scheduledTx.accountId,
                    type: 'expense',
                    amount: scheduledTx.amount,
                    category: 'transfer',
                    description: scheduledTx.description,
                    checked: false,
                    isTransfer: true,
                    linkedTransactionId: linkedId
                };

                const destTx: Transaction = {
                    id: linkedId,
                    date: occurrenceDate,
                    accountId: scheduledTx.toAccountId,
                    type: 'income',
                    amount: scheduledTx.amount,
                    category: 'transfer',
                    description: scheduledTx.description,
                    checked: false,
                    isTransfer: true,
                    linkedTransactionId: txId
                };

                const inserts = [];
                if (!existingTransactionIds.has(sourceTx.id)) {
                    newTransactions.push(sourceTx);
                    existingTransactionIds.add(sourceTx.id);
                    inserts.push(dbService.addTransaction(sourceTx));
                }
                if (!existingTransactionIds.has(destTx.id)) {
                    newTransactions.push(destTx);
                    existingTransactionIds.add(destTx.id);
                    inserts.push(dbService.addTransaction(destTx));
                }
                await Promise.all(inserts);
            } else {
                const txId = getScheduledTransactionId(scheduledTx.id, occurrenceDate, 'single');
                const newTx: Transaction = {
                    id: txId,
                    date: occurrenceDate,
                    accountId: scheduledTx.accountId,
                    type: scheduledTx.type,
                    amount: scheduledTx.amount,
                    category: scheduledTx.category,
                    description: scheduledTx.description,
                    checked: false
                };

                if (!existingTransactionIds.has(newTx.id)) {
                    newTransactions.push(newTx);
                    existingTransactionIds.add(newTx.id);
                    await dbService.addTransaction(newTx);
                }
            }

            const newDate = getNextScheduledDate(new Date(scheduledTx.nextDate), scheduledTx.frequency);
            scheduledTx.nextDate = newDate.toISOString().split('T')[0];
            nextDate = new Date(scheduledTx.nextDate);
            nextDate.setHours(0, 0, 0, 0);
        }

        if (modified) {
            hasScheduledChanges = true;
            if (scheduledTx.frequency === 'once') {
                await dbService.deleteScheduled(scheduledTx.id);
                processedScheduled.splice(i, 1);
                i--;
            } else {
                await dbService.updateScheduled(scheduledTx);
            }
        }
    }

    return { processedScheduled, newTransactions, hasScheduledChanges };
};

export const BankProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [accounts, setAccounts] = useState<Account[]>([]);
    const [transactions, setTransactions] = useState<Transaction[]>([]);
    const [categories, setCategories] = useState<Category[]>([]);
    const [scheduled, setScheduled] = useState<ScheduledTransaction[]>([]);
    const [budgets, setBudgets] = useState<Budget[]>([]);
    const [filterAccount, setFilterAccount] = useState<string[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [mobileConnectionState, setMobileConnectionState] = useState<BankContextType['mobileConnectionState']>('idle');
    const [mobileConnectionError, setMobileConnectionError] = useState<string | null>(null);
    const isProcessingScheduledRef = useRef(false);
    const lastDataVersionRef = useRef<number | null>(null);

    const loadBankData = useCallback(async (options: { processScheduled?: boolean } = {}) => {
        const shouldProcessScheduled = options.processScheduled ?? false;
        const mobileMode = isMobileCompanion();

        if (shouldProcessScheduled && mobileMode) {
            try {
                await dbService.processDueScheduled();
            } catch (error) {
                if (!dbService.isOfflineError(error)) throw error;
            }
        }

        const [loadedAccounts, loadedTransactions, loadedCategories, loadedScheduled, loadedBudgets] = await Promise.all([
            dbService.getAccounts(),
            dbService.getTransactions(),
            dbService.getCategories(),
            dbService.getScheduled(),
            dbService.getBudgets()
        ]);

        let currentAccounts = loadedAccounts;
        let currentTransactions = loadedTransactions;
        let currentCategories = loadedCategories;
        let currentScheduled = loadedScheduled;
        let currentBudgets = loadedBudgets;

        if (loadedAccounts.length === 0 && loadedTransactions.length === 0 && loadedCategories.length === 0 && loadedScheduled.length === 0 && loadedBudgets.length === 0) {
            currentAccounts = DEFAULT_DATA.accounts;
            currentTransactions = DEFAULT_DATA.transactions;
            currentCategories = DEFAULT_DATA.categories;
            currentScheduled = DEFAULT_DATA.scheduled;
            currentBudgets = DEFAULT_DATA.budgets;

            await Promise.all([
                ...DEFAULT_DATA.accounts.map(acc => dbService.addAccount(acc)),
                ...DEFAULT_DATA.categories.map(cat => dbService.addCategory(cat)),
                ...DEFAULT_DATA.budgets.map(budget => dbService.addBudget(budget))
            ]);
        } else {
            const transferExists = currentCategories.some(c => c.id === 'transfer');
            if (!transferExists) {
                const transferCategory = { id: 'transfer', name: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' };
                await dbService.addCategory(transferCategory);
                currentCategories = [...currentCategories, transferCategory];
            }
        }

        const migratedScheduled = [...currentScheduled];
        const migratedBudgets = [...currentBudgets];
        const legacyScheduledToMigrate = migratedScheduled.filter(item => item.includeInForecast && !item.budgetId && item.type === 'expense' && item.category !== 'transfer');

        if (legacyScheduledToMigrate.length > 0) {
            for (const scheduledTx of legacyScheduledToMigrate) {
                const newBudget: Budget = {
                    id: uuidv4(),
                    name: scheduledTx.description,
                    amount: scheduledTx.amount,
                    category: scheduledTx.category,
                    accountId: scheduledTx.accountId
                };

                scheduledTx.budgetId = newBudget.id;
                scheduledTx.includeInForecast = true;
                migratedBudgets.push(newBudget);

                await dbService.addBudget(newBudget);
                await dbService.updateScheduled(scheduledTx);
            }
            currentScheduled = migratedScheduled;
            currentBudgets = migratedBudgets;
        }

        const scheduledResult = shouldProcessScheduled && !mobileMode
            ? await processDueScheduledItems(currentScheduled, currentTransactions)
            : { processedScheduled: currentScheduled, newTransactions: [] as Transaction[] };

        setAccounts(currentAccounts);
        setTransactions([...scheduledResult.newTransactions, ...currentTransactions]);
        setCategories(currentCategories);
        setScheduled(scheduledResult.processedScheduled);
        setBudgets(currentBudgets);
    }, []);

    // Initialize DB and load data
    useEffect(() => {
        const init = async () => {
            const mobileMode = isMobileCompanion();

            try {
                await dbService.init();
                await loadBankData({ processScheduled: true });
                if (mobileMode) {
                    try {
                        const status = await dbService.getSyncStatus();
                        lastDataVersionRef.current = status.dataVersion;
                        notifySettingsRefresh();
                        setMobileConnectionState('connected');
                    } catch (error) {
                        if (!dbService.isOfflineError(error)) throw error;
                        setMobileConnectionState('offline');
                    }
                } else {
                    setMobileConnectionState('connected');
                }
            } catch (error) {
                console.error("Failed to initialize database:", error);
                if (mobileMode) {
                    setMobileConnectionError(error instanceof Error ? error.message : String(error));
                    setMobileConnectionState('error');
                }
            } finally {
                setIsLoading(false);
            }
        };
        init();
    }, [loadBankData]);

    const connectMobileCompanion = useCallback(async () => {
        if (!isMobileCompanion()) return;

        setIsLoading(true);
        setMobileConnectionState('connecting');
        setMobileConnectionError(null);
        try {
            await dbService.connectMobileCompanion();
            await loadBankData({ processScheduled: true });
            const status = await dbService.getSyncStatus();
            lastDataVersionRef.current = status.dataVersion;
            notifySettingsRefresh();
            setMobileConnectionState('connected');
        } catch (error) {
            console.error("Failed to connect mobile companion:", error);
            setMobileConnectionError(error instanceof Error ? error.message : String(error));
            setMobileConnectionState('error');
        } finally {
            setIsLoading(false);
        }
    }, [loadBankData]);

    const unlinkMobileCompanion = useCallback(async () => {
        if (!isMobileCompanion()) return;

        setIsLoading(true);
        try {
            await dbService.unlinkMobileCompanion();
            setAccounts([]);
            setTransactions([]);
            setCategories([]);
            setScheduled([]);
            setBudgets([]);
            setFilterAccount([]);
            lastDataVersionRef.current = null;
            setMobileConnectionError(null);
            setMobileConnectionState('disconnected');
        } finally {
            setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        if (isLoading) return;

        let disposed = false;
        let unlisten: (() => void) | undefined;
        let syncTimeout: ReturnType<typeof setTimeout> | undefined;
        let consecutiveSyncFailures = 0;

        if (hasTauriRuntime()) {
            import('@tauri-apps/api/event')
                .then(({ listen }) => listen('bank-data-changed', async () => {
                    if (!disposed) {
                        notifySettingsRefresh();
                        await loadBankData({ processScheduled: false });
                    }
                }))
                .then(removeListener => {
                    unlisten = removeListener;
                })
                .catch(() => { });
        }

        if (isMobileCompanion() && (mobileConnectionState === 'connected' || mobileConnectionState === 'offline')) {
            const checkForRemoteChanges = async () => {
                if (disposed) return;

                try {
                    const status = await dbService.getSyncStatus();
                    consecutiveSyncFailures = 0;
                    if (!disposed) setMobileConnectionState('connected');
                    if (lastDataVersionRef.current === null) {
                        lastDataVersionRef.current = status.dataVersion;
                        return;
                    }
                    if (status.dataVersion !== lastDataVersionRef.current) {
                        lastDataVersionRef.current = status.dataVersion;
                        notifySettingsRefresh();
                        await loadBankData({ processScheduled: false });
                    }
                } catch (error) {
                    consecutiveSyncFailures += 1;
                    if (dbService.isOfflineError(error) && !disposed) {
                        setMobileConnectionState('offline');
                    }
                    console.warn('Mobile sync status unavailable:', error);
                } finally {
                    if (!disposed) {
                        const delay = consecutiveSyncFailures === 0
                            ? 2500
                            : Math.min(30000, 5000 * consecutiveSyncFailures);
                        syncTimeout = setTimeout(checkForRemoteChanges, delay);
                    }
                }
            };

            syncTimeout = setTimeout(checkForRemoteChanges, 2500);
        }

        return () => {
            disposed = true;
            if (unlisten) unlisten();
            if (syncTimeout) clearTimeout(syncTimeout);
        };
    }, [isLoading, loadBankData, mobileConnectionState]);

    const getAccountDefaults = (type: string) => {
        switch (type) {
            case 'Épargne': return { icon: 'PiggyBank', color: '#10b981' };
            case 'Espèces': return { icon: 'Banknote', color: '#f59e0b' };
            case 'Investissement': return { icon: 'TrendingUp', color: '#8b5cf6' };
            default: return { icon: 'Wallet', color: '#3b82f6' };
        }
    };

    // --- Accounts ---
    const addAccount = useCallback(async (account: Omit<Account, 'id'>) => {
        const newAccount: Account = {
            ...account,
            id: uuidv4(),
            icon: account.icon || getAccountDefaults(account.type).icon,
            color: account.color || getAccountDefaults(account.type).color
        };
        await dbService.addAccount(newAccount);
        setAccounts(prev => [...prev, newAccount]);
        return newAccount.id;
    }, []);

    const updateAccount = useCallback(async (account: Account) => {
        await dbService.updateAccount(account);
        setAccounts(prev => prev.map(a => a.id === account.id ? account : a));
    }, []);

    const deleteAccount = useCallback(async (id: string) => {
        // Rust delete_account already deletes associated transactions and scheduled in a SQL transaction
        await dbService.deleteAccount(id);
        setAccounts(prev => prev.filter(a => a.id !== id));
        setTransactions(prev => prev.filter(t => t.accountId !== id));
        setScheduled(prev => prev.filter(s => s.accountId !== id));
        setBudgets(prev => prev.map(b => b.accountId === id ? { ...b, accountId: undefined } : b));
        setFilterAccount(prev => prev.includes(id) ? prev.filter(a => a !== id) : prev);
    }, []);

    // --- Transactions ---
    const addTransaction = useCallback(async (transaction: Omit<Transaction, 'id'>) => {
        const newTransaction: Transaction = { ...transaction, id: uuidv4() };
        await dbService.addTransaction(newTransaction);
        setTransactions(prev => [newTransaction, ...prev]);
        return newTransaction.id;
    }, []);

    const addTransfer = useCallback(async (fromAccountId: string, toAccountId: string, amount: number, date: string, description: string) => {
        const fromTxId = uuidv4();
        const toTxId = uuidv4();

        const fromTx: Transaction = {
            id: fromTxId, date, accountId: fromAccountId, type: 'expense', amount,
            category: 'transfer', description, checked: false, isTransfer: true, linkedTransactionId: toTxId
        };

        const toTx: Transaction = {
            id: toTxId, date, accountId: toAccountId, type: 'income', amount,
            category: 'transfer', description, checked: false, isTransfer: true, linkedTransactionId: fromTxId
        };

        await dbService.addTransfer(fromTx, toTx);

        setTransactions(prev => [fromTx, toTx, ...prev]);
    }, []);

    const updateTransaction = useCallback(async (transaction: Transaction) => {
        await dbService.updateTransaction(transaction);
        setTransactions(prev => prev.map(t => t.id === transaction.id ? transaction : t));
    }, []);

    const deleteTransaction = useCallback(async (id: string) => {
        const transaction = transactions.find(t => t.id === id);
        const idsToRemove = new Set([id]);
        if (transaction?.linkedTransactionId) {
            idsToRemove.add(transaction.linkedTransactionId);
        }

        await dbService.deleteTransaction(id);
        setTransactions(prev => prev.filter(t => !idsToRemove.has(t.id)));
    }, [transactions]);

    const toggleTransactionCheck = useCallback(async (id: string) => {
        const transaction = transactions.find(t => t.id === id);
        if (!transaction) return;

        const updated = { ...transaction, checked: !transaction.checked };
        await dbService.updateTransaction(updated);
        setTransactions(prev => prev.map(t => t.id === id ? updated : t));
    }, [transactions]);

    const processDueScheduledTransactions = useCallback(async () => {
        if (isLoading || isProcessingScheduledRef.current) return 0;

        isProcessingScheduledRef.current = true;
        try {
            const { processedScheduled, newTransactions, hasScheduledChanges } = await processDueScheduledItems(scheduled, transactions);

            if (newTransactions.length > 0) {
                setTransactions(prev => [...newTransactions, ...prev]);
            }
            if (hasScheduledChanges) {
                setScheduled(processedScheduled);
            }

            return newTransactions.length;
        } catch (error) {
            console.error("Failed to process scheduled transactions:", error);
            return 0;
        } finally {
            isProcessingScheduledRef.current = false;
        }
    }, [isLoading, scheduled, transactions]);

    // --- Categories ---
    const addCategory = useCallback(async (category: Omit<Category, 'id'>) => {
        const newCategory: Category = { ...category, id: uuidv4() };
        await dbService.addCategory(newCategory);
        setCategories(prev => [...prev, newCategory]);
        return newCategory.id;
    }, []);

    const updateCategory = useCallback(async (category: Category) => {
        await dbService.updateCategory(category);
        setCategories(prev => prev.map(c => c.id === category.id ? category : c));
    }, []);

    const deleteCategory = useCallback(async (id: string) => {
        await dbService.deleteCategory(id);
        setCategories(prev => prev.filter(c => c.id !== id));
    }, []);

    // --- Scheduled ---
    const addScheduled = useCallback(async (scheduledTx: Omit<ScheduledTransaction, 'id'>) => {
        const newScheduled: ScheduledTransaction = { ...scheduledTx, id: uuidv4() };
        await dbService.addScheduled(newScheduled);
        setScheduled(prev => [...prev, newScheduled]);
    }, []);

    const updateScheduled = useCallback(async (scheduledTx: ScheduledTransaction) => {
        await dbService.updateScheduled(scheduledTx);
        setScheduled(prev => prev.map(s => s.id === scheduledTx.id ? scheduledTx : s));
    }, []);

    const deleteScheduled = useCallback(async (id: string) => {
        await dbService.deleteScheduled(id);
        setScheduled(prev => prev.filter(s => s.id !== id));
    }, []);

    // --- Budgets ---
    const addBudget = useCallback(async (budget: Omit<Budget, 'id'>) => {
        const newBudget: Budget = { ...budget, id: uuidv4() };
        await dbService.addBudget(newBudget);
        setBudgets(prev => [newBudget, ...prev]);
        return newBudget.id;
    }, []);

    const updateBudget = useCallback(async (budget: Budget) => {
        await dbService.updateBudget(budget);
        setBudgets(prev => prev.map(b => b.id === budget.id ? budget : b));
    }, []);

    const deleteBudget = useCallback(async (id: string) => {
        await dbService.deleteBudget(id);
        setBudgets(prev => prev.filter(b => b.id !== id));
        setScheduled(prev => prev.map(s => s.budgetId === id ? { ...s, budgetId: undefined, includeInForecast: false } : s));
    }, []);

    const contextValue = useMemo(() => ({
        accounts,
        transactions,
        categories,
        scheduled,
        budgets,
        addAccount,
        updateAccount,
        deleteAccount,
        addTransaction,
        addTransfer,
        updateTransaction,
        deleteTransaction,
        toggleTransactionCheck,
        addCategory,
        updateCategory,
        deleteCategory,
        addScheduled,
        updateScheduled,
        deleteScheduled,
        processDueScheduledTransactions,
        addBudget,
        updateBudget,
        deleteBudget,
        filterAccount,
        setFilterAccount,
        isLoading,
        mobileConnectionState,
        mobileConnectionError,
        connectMobileCompanion,
        unlinkMobileCompanion
    }), [
        accounts, transactions, categories, scheduled, budgets, filterAccount, isLoading,
        mobileConnectionState, mobileConnectionError, connectMobileCompanion, unlinkMobileCompanion,
        addAccount, updateAccount, deleteAccount,
        addTransaction, addTransfer, updateTransaction, deleteTransaction, toggleTransactionCheck,
        addCategory, updateCategory, deleteCategory,
        addScheduled, updateScheduled, deleteScheduled, processDueScheduledTransactions,
        addBudget, updateBudget, deleteBudget
    ]);

    return (
        <BankContext.Provider value={contextValue}>
            {children}
        </BankContext.Provider>
    );
};

export const useBank = () => {
    const context = useContext(BankContext);
    if (context === undefined) {
        throw new Error('useBank must be used within a BankProvider');
    }
    return context;
};
