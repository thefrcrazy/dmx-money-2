
import React, { useState, useMemo, useEffect } from 'react';
import { Plus, Calendar, Trash2, Edit2, Clock, X, Tag, Sparkles, Search, ChevronDown, ArrowRightLeft } from 'lucide-react';
import Button from '../components/ui/Button';
import { useBank } from '../context/BankContext';
import { useToast } from '../context/ToastContext';
import { useSettings } from '../context/SettingsContext';
import { format } from 'date-fns';
import { fr } from 'date-fns/locale';
import FormPopup from '../components/ui/FormPopup';
import ConfirmModal from '../components/ui/ConfirmModal';
import SearchableSelect, { SelectOption } from '../components/ui/SearchableSelect';
import MultiSelect from '../components/ui/MultiSelect';
import { ScheduledDueRange, ScheduledTransaction, Transaction, TransactionType, Periodicity } from '../types';
import { ICONS } from '../constants/icons';
import Table from '../components/ui/Table';
import Input from '../components/ui/Input';

type RecurrenceIdentityInput = {
    description: string;
    type: TransactionType;
    category: string;
    accountId: string;
    toAccountId?: string;
};

type SuggestionBase = Pick<ScheduledTransaction, 'description' | 'amount' | 'type' | 'category' | 'accountId' | 'toAccountId'>;

interface SuggestionGroup {
    identity: string;
    base: SuggestionBase;
    records: Transaction[];
}

interface MonthlySuggestion extends Omit<ScheduledTransaction, 'id'> {
    suggestionKey: string;
    occurrenceCount: number;
}

const SCHEDULED_DUE_RANGES: ScheduledDueRange[] = ['all', 'month', '2months', '3months', '6months', 'year'];

const SCHEDULED_DUE_RANGE_LABELS: Record<ScheduledDueRange, string> = {
    all: 'Toutes',
    month: 'Mois',
    '2months': '2 Mois',
    '3months': '3 Mois',
    '6months': '6 Mois',
    year: '1 An'
};

const SCHEDULED_DUE_RANGE_MONTHS: Record<Exclude<ScheduledDueRange, 'all'>, number> = {
    month: 1,
    '2months': 2,
    '3months': 3,
    '6months': 6,
    year: 12
};

const normalizeDescription = (value: string) => value.trim().replace(/\s+/g, ' ').toLowerCase();
const normalizeSearchValue = (value: unknown) => String(value ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');

const FREQUENCY_LABELS: Record<Periodicity, string> = {
    once: 'Une seule fois',
    daily: 'Journalier',
    weekly: 'Hebdomadaire',
    biweekly: 'Toutes les 2 semaines',
    bimonthly: 'Bimensuel',
    fourweekly: 'Toutes les 4 semaines',
    monthly: 'Mensuel',
    bimestrial: 'Bimestriel',
    quarterly: 'Trimestriel',
    fourmonthly: 'Tous les 4 mois',
    semiannual: 'Semestriel',
    annual: 'Annuel',
    biennial: 'Bisannuel'
};

const parseLocalDate = (value: string) => {
    const [year, month, day] = value.split('-').map(Number);
    return new Date(year, (month || 1) - 1, day || 1);
};

const formatLocalDate = (date: Date) => {
    const year = date.getFullYear();
    const month = `${date.getMonth() + 1}`.padStart(2, '0');
    const day = `${date.getDate()}`.padStart(2, '0');
    return `${year}-${month}-${day}`;
};

const addMonthsSafely = (date: Date, months: number) => {
    const targetMonth = date.getMonth() + months;
    const daysInTargetMonth = new Date(date.getFullYear(), targetMonth + 1, 0).getDate();
    return new Date(date.getFullYear(), targetMonth, Math.min(date.getDate(), daysInTargetMonth));
};

const mergeSuggestionKeys = (...sources: Array<string[] | undefined>) => (
    Array.from(new Set(sources.flatMap(source => source || [])))
);

const getMonthIndex = (date: string) => {
    const parsedDate = parseLocalDate(date);
    return parsedDate.getFullYear() * 12 + parsedDate.getMonth();
};

const getAmountKey = (amount: number) => Math.round(amount * 100).toString();

const getRecurrenceIdentity = (item: RecurrenceIdentityInput) => [
    item.accountId,
    item.toAccountId || '',
    item.type,
    item.category,
    normalizeDescription(item.description)
].join('|');

const Scheduled: React.FC = () => {
    const { accounts, transactions, scheduled, categories, budgets, addScheduled, updateScheduled, deleteScheduled, filterAccount } = useBank();
    const { showToast } = useToast();
    const { settings, updateDismissedScheduledSuggestions, updateSettings } = useSettings();
    const dueRange = settings.scheduledDueRange || 'all';
    const [isModalOpen, setIsModalOpen] = useState(false);
    const [editingTransaction, setEditingTransaction] = useState<ScheduledTransaction | null>(null);
    const [isSuggestionsOpen, setIsSuggestionsOpen] = useState(false);
    const [searchTerm, setSearchTerm] = useState('');
    const [isDueRangeDropdownOpen, setIsDueRangeDropdownOpen] = useState(false);
    const [filterCategories, setFilterCategories] = useState<string[]>([]);
    const [filterFrequencies, setFilterFrequencies] = useState<string[]>([]);
    const dismissedSuggestionKeys = useMemo(
        () => new Set(settings.dismissedScheduledSuggestions || []),
        [settings.dismissedScheduledSuggestions]
    );

    const setDueRange = (scheduledDueRange: ScheduledDueRange) => {
        void updateSettings({ scheduledDueRange });
    };

    const accountMap = useMemo(() => new Map(accounts.map(account => [account.id, account])), [accounts]);
    const categoryMap = useMemo(() => new Map(categories.map(category => [category.id, category])), [categories]);
    const budgetMap = useMemo(() => new Map(budgets.map(budget => [budget.id, budget])), [budgets]);

    const scheduledTransactions = useMemo(() => {
        let filtered = filterAccount.length === 0
            ? scheduled
            : scheduled.filter(t => filterAccount.includes(t.accountId));

        if (dueRange !== 'all') {
            const today = new Date();
            today.setHours(0, 0, 0, 0);
            const endDate = addMonthsSafely(today, SCHEDULED_DUE_RANGE_MONTHS[dueRange]);
            endDate.setHours(23, 59, 59, 999);

            filtered = filtered.filter(transaction => {
                const dueDate = parseLocalDate(transaction.nextDate);
                return dueDate >= today && dueDate <= endDate;
            });
        }

        if (filterCategories.length > 0) {
            filtered = filtered.filter(transaction => filterCategories.includes(transaction.category));
        }

        if (filterFrequencies.length > 0) {
            filtered = filtered.filter(transaction => filterFrequencies.includes(transaction.frequency));
        }

        const searchTokens = normalizeSearchValue(searchTerm).split(/\s+/).filter(Boolean);
        if (searchTokens.length > 0) {
            filtered = filtered.filter(transaction => {
                const account = accountMap.get(transaction.accountId);
                const toAccount = transaction.toAccountId ? accountMap.get(transaction.toAccountId) : undefined;
                const category = transaction.type === 'transfer'
                    ? { name: 'Virement' }
                    : categoryMap.get(transaction.category);
                const typeLabel = transaction.type === 'income'
                    ? 'Revenu'
                    : transaction.type === 'transfer'
                        ? 'Virement'
                        : 'Dépense';
                const amountPrefix = transaction.type === 'income' ? '+' : transaction.type === 'transfer' ? '' : '-';
                const budgetLabel = transaction.budgetId ? budgetMap.get(transaction.budgetId)?.name || 'Budget lié' : 'Hors budget';
                const statusLabel = transaction.endDate && new Date(transaction.endDate) < new Date() ? 'Terminé' : 'Actif';
                const searchableText = [
                    account?.name,
                    toAccount?.name,
                    format(parseLocalDate(transaction.nextDate), 'dd MMM yyyy', { locale: fr }),
                    transaction.nextDate,
                    transaction.endDate,
                    transaction.endDate ? format(parseLocalDate(transaction.endDate), 'dd MMM yyyy', { locale: fr }) : '',
                    FREQUENCY_LABELS[transaction.frequency],
                    category?.name,
                    typeLabel,
                    transaction.description,
                    transaction.amount,
                    `${amountPrefix}${new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(transaction.amount)} €`,
                    budgetLabel,
                    statusLabel
                ].map(normalizeSearchValue).join(' ');

                return searchTokens.every(token => searchableText.includes(token));
            });
        }

        return [...filtered].sort((a, b) => new Date(a.nextDate).getTime() - new Date(b.nextDate).getTime());
    }, [
        scheduled,
        filterAccount,
        dueRange,
        filterCategories,
        filterFrequencies,
        searchTerm,
        accountMap,
        categoryMap,
        budgetMap
    ]);

    const monthlySuggestions = useMemo<MonthlySuggestion[]>(() => {
        const transactionById = new Map(transactions.map(transaction => [transaction.id, transaction]));
        const scheduledIdentities = new Set(scheduled.map(item => getRecurrenceIdentity({
            description: item.description,
            type: item.type,
            category: item.category,
            accountId: item.accountId,
            toAccountId: item.toAccountId
        })));

        const groups = new Map<string, SuggestionGroup>();

        transactions.forEach(transaction => {
            if (filterAccount.length > 0 && !filterAccount.includes(transaction.accountId)) return;

            const description = (transaction.description || '').trim();
            if (!description || transaction.amount <= 0) return;

            const isTransfer = transaction.category === 'transfer' || transaction.isTransfer;
            let type = transaction.type;
            let category = transaction.category;
            let toAccountId: string | undefined;

            if (isTransfer) {
                if (transaction.type !== 'expense') return;
                const linkedTransaction = transaction.linkedTransactionId ? transactionById.get(transaction.linkedTransactionId) : undefined;
                toAccountId = linkedTransaction?.accountId;
                if (!toAccountId) return;
                type = 'transfer';
                category = 'transfer';
            }

            const identity = getRecurrenceIdentity({
                description,
                type,
                category,
                accountId: transaction.accountId,
                toAccountId
            });

            if (scheduledIdentities.has(identity)) return;

            const groupKey = `${identity}|${getAmountKey(transaction.amount)}`;
            const group = groups.get(groupKey);

            if (group) {
                group.records.push(transaction);
            } else {
                groups.set(groupKey, {
                    identity,
                    base: {
                        description,
                        amount: transaction.amount,
                        type,
                        category,
                        accountId: transaction.accountId,
                        toAccountId
                    },
                    records: [transaction]
                });
            }
        });

        const today = new Date();
        today.setHours(0, 0, 0, 0);

        return Array.from(groups.values())
            .reduce<MonthlySuggestion[]>((suggestions, group) => {
                const monthIndexes = Array.from(new Set(group.records.map(record => getMonthIndex(record.date))))
                    .sort((a, b) => a - b);

                if (monthIndexes.length < 2) return suggestions;

                let currentRun = 1;
                let longestRun = 1;
                for (let index = 1; index < monthIndexes.length; index++) {
                    currentRun = monthIndexes[index] === monthIndexes[index - 1] + 1 ? currentRun + 1 : 1;
                    longestRun = Math.max(longestRun, currentRun);
                }

                if (longestRun < 2) return suggestions;

                const latestRecord = group.records.reduce((latest, record) => (
                    parseLocalDate(record.date).getTime() > parseLocalDate(latest.date).getTime() ? record : latest
                ), group.records[0]);

                let nextDate = addMonthsSafely(parseLocalDate(latestRecord.date), 1);
                while (nextDate <= today) {
                    nextDate = addMonthsSafely(nextDate, 1);
                }

                suggestions.push({
                    ...group.base,
                    amount: latestRecord.amount,
                    frequency: 'monthly',
                    nextDate: formatLocalDate(nextDate),
                    includeInForecast: false,
                    suggestionKey: `${group.identity}|${getAmountKey(latestRecord.amount)}`,
                    occurrenceCount: monthIndexes.length
                });

                return suggestions;
            }, [])
            .sort((a, b) => {
                const dateDiff = new Date(a.nextDate).getTime() - new Date(b.nextDate).getTime();
                return dateDiff || a.description.localeCompare(b.description, 'fr');
            })
            .filter(suggestion => !dismissedSuggestionKeys.has(suggestion.suggestionKey));
    }, [transactions, scheduled, filterAccount, dismissedSuggestionKeys]);

    useEffect(() => {
        if (monthlySuggestions.length === 0) {
            setIsSuggestionsOpen(false);
        }
    }, [monthlySuggestions.length]);

    // Delete Confirmation State
    const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);
    const [transactionToDelete, setTransactionToDelete] = useState<string | null>(null);

    const [formData, setFormData] = useState({
        description: '',
        amount: '',
        type: 'expense' as TransactionType,
        categoryId: '',
        accountId: '',
        toAccountId: '',
        frequency: 'monthly' as Periodicity,
        nextDate: new Date().toISOString().split('T')[0],
        includeInForecast: false,
        linkToBudget: false,
        budgetId: '',
        endDate: ''
    });

    const handleOpenModal = (transaction?: ScheduledTransaction) => {
        if (transaction) {
            setEditingTransaction(transaction);
            setFormData({
                description: transaction.description,
                amount: transaction.amount.toString(),
                type: transaction.type,
                categoryId: transaction.category,
                accountId: transaction.accountId,
                toAccountId: transaction.toAccountId || '',
                frequency: transaction.frequency,
                nextDate: transaction.nextDate,
                includeInForecast: !!transaction.budgetId,
                linkToBudget: !!transaction.budgetId,
                budgetId: transaction.budgetId || '',
                endDate: transaction.endDate || ''
            });
        } else {
            setEditingTransaction(null);
            setFormData({
                description: '',
                amount: '',
                type: 'expense',
                categoryId: '',
                accountId: accounts[0]?.id || '',
                toAccountId: '',
                frequency: 'monthly',
                nextDate: new Date().toISOString().split('T')[0],
                includeInForecast: false,
                linkToBudget: false,
                budgetId: '',
                endDate: ''
            });
        }
        setIsModalOpen(true);
    };

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        const transactionData = {
            description: formData.description,
            amount: parseFloat(formData.amount),
            type: formData.type,
            category: formData.type === 'transfer' ? 'transfer' : formData.categoryId,
            accountId: formData.accountId,
            toAccountId: formData.toAccountId || undefined,
            frequency: formData.frequency,
            nextDate: formData.nextDate,
            includeInForecast: formData.type === 'expense' && formData.linkToBudget && !!formData.budgetId,
            budgetId: formData.type === 'expense' && formData.linkToBudget ? formData.budgetId || undefined : undefined,
            endDate: formData.endDate || undefined
        };

        if (editingTransaction) {
            updateScheduled({ ...transactionData, id: editingTransaction.id });
        } else {
            addScheduled(transactionData);
        }
        setIsModalOpen(false);
    };

    const handleDeleteClick = (id: string) => {
        setTransactionToDelete(id);
        setIsDeleteModalOpen(true);
    };

    const handleConfirmDelete = () => {
        if (transactionToDelete) {
            deleteScheduled(transactionToDelete);
            setTransactionToDelete(null);
        }
        setIsDeleteModalOpen(false);
    };

    const getCategoryDetails = (id: string) => {
        const cat = categories.find(c => c.id === id);
        return cat || { name: 'Inconnu', color: '#9ca3af', icon: 'Tag' };
    };

    const renderCategoryIcon = (iconName: string, className: string = "w-4 h-4") => {
        const Icon = ICONS[iconName] || Tag;
        return <Icon className={className} />;
    };

    const handleAddSuggestion = async (suggestion: MonthlySuggestion) => {
        try {
            await addScheduled({
                description: suggestion.description,
                amount: suggestion.amount,
                type: suggestion.type,
                frequency: suggestion.frequency,
                accountId: suggestion.accountId,
                toAccountId: suggestion.toAccountId,
                nextDate: suggestion.nextDate,
                category: suggestion.category,
                includeInForecast: suggestion.includeInForecast,
                endDate: suggestion.endDate
            });
            showToast("Suggestion ajoutée à l'échéancier", "success");
        } catch {
            showToast("Erreur lors de l'ajout de la suggestion", "error");
        }
    };

    const handleDismissSuggestion = (suggestion: MonthlySuggestion) => {
        updateDismissedScheduledSuggestions(
            mergeSuggestionKeys(settings.dismissedScheduledSuggestions, [suggestion.suggestionKey])
        ).catch(() => {
            showToast("Erreur lors de la synchronisation de la suggestion", "error");
        });
        showToast("Suggestion supprimée", "success");
    };

    const categoryOptions: SelectOption[] = categories.map(c => ({
        id: c.id,
        label: c.name,
        icon: c.icon,
        color: c.color
    }));

    const categoryFilterOptions: SelectOption[] = [
        { id: 'transfer', label: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' },
        ...categories
            .filter(category => category.id !== 'transfer')
            .map(category => ({ id: category.id, label: category.name, icon: category.icon, color: category.color }))
    ];

    const frequencyFilterOptions: SelectOption[] = (Object.entries(FREQUENCY_LABELS) as Array<[Periodicity, string]>).map(([id, label]) => ({
        id,
        label,
        icon: 'Clock'
    }));

    const budgetOptions: SelectOption[] = budgets.map(budget => {
        const category = getCategoryDetails(budget.category);
        return {
            id: budget.id,
            label: `${budget.name} · ${new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 0, maximumFractionDigits: 2 }).format(budget.amount)} €`,
            icon: category.icon,
            color: category.color
        };
    });

    const getBudgetFormPatch = (budgetId: string) => {
        const budget = budgets.find(item => item.id === budgetId);
        if (!budget) return { budgetId };

        return {
            budgetId,
            categoryId: budget.category,
            accountId: budget.accountId || formData.accountId
        };
    };

    return (
        <div className="flex flex-col space-y-6">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full">
                <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-200">Transactions Récurrentes</h2>
                <div className="flex items-center gap-2 w-full sm:w-auto">
                    {monthlySuggestions.length > 0 && (
                        <div className="flex-1 sm:flex-initial">
                            <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => setIsSuggestionsOpen(true)}
                                icon={Sparkles}
                                className="w-full"
                            >
                                Suggestions ({monthlySuggestions.length})
                            </Button>
                        </div>
                    )}
                    <Button
                        onClick={() => handleOpenModal()}
                        size="sm"
                        icon={Plus}
                        className="flex-1 sm:flex-none"
                    >
                        Nouvelle transaction
                    </Button>
                </div>
            </div>

            <div className="flex flex-col gap-3 px-1 flex-none">
                <div className="hidden md:flex flex-wrap gap-2 period-selector">
                    {SCHEDULED_DUE_RANGES.map(range => (
                        <Button
                            key={range}
                            variant="ghost"
                            size="sm"
                            onClick={() => setDueRange(range)}
                            className={`transition-colors ${dueRange === range
                                ? 'bg-primary-100 text-primary-700 hover:bg-primary-200 dark:bg-primary-900/20 dark:text-primary-400 dark:hover:bg-primary-900/30'
                                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700'
                                }`}
                        >
                            {SCHEDULED_DUE_RANGE_LABELS[range]}
                        </Button>
                    ))}
                </div>

                <div className="relative md:hidden w-full z-30">
                    <button
                        onClick={() => setIsDueRangeDropdownOpen(!isDueRangeDropdownOpen)}
                        className="w-full flex items-center justify-between px-4 py-3 rounded-2xl border border-black/[0.06] dark:border-white/[0.08] bg-white/80 dark:bg-neutral-900/60 text-sm font-semibold text-gray-800 dark:text-gray-200 shadow-sm active:scale-[0.98] transition-all"
                    >
                        <span>{SCHEDULED_DUE_RANGE_LABELS[dueRange]}</span>
                        <ChevronDown className={`w-4 h-4 opacity-60 transition-transform duration-200 ${isDueRangeDropdownOpen ? 'rotate-180' : ''}`} />
                    </button>
                    
                    {isDueRangeDropdownOpen && (
                        <>
                            <div 
                                className="fixed inset-0 z-40 bg-transparent" 
                                onClick={() => setIsDueRangeDropdownOpen(false)}
                            />
                            <div className="absolute left-0 right-0 w-full mt-1.5 z-50 rounded-2xl border border-black/[0.08] dark:border-white/[0.12] bg-white dark:bg-neutral-900 shadow-xl p-1.5 flex flex-col gap-0.5 animate-in fade-in-50 slide-in-from-top-2 duration-150">
                                {SCHEDULED_DUE_RANGES.map(range => (
                                    <button
                                        key={range}
                                        onClick={() => {
                                            setDueRange(range);
                                            setIsDueRangeDropdownOpen(false);
                                        }}
                                        className={`w-full text-left px-3.5 py-2.5 rounded-xl text-sm font-medium transition-colors flex items-center justify-between ${
                                            dueRange === range
                                                ? 'bg-primary-500/10 text-primary-600 dark:text-primary-400'
                                                : 'text-gray-700 dark:text-gray-300 active:bg-black/5 dark:active:bg-white/5'
                                        }`}
                                    >
                                        <span>{SCHEDULED_DUE_RANGE_LABELS[range]}</span>
                                        {dueRange === range && (
                                            <span className="w-1.5 h-1.5 rounded-full bg-primary-500" />
                                        )}
                                    </button>
                                ))}
                            </div>
                        </>
                    )}
                </div>
            </div>

            <div className="flex flex-col lg:flex-row gap-3 px-1 flex-none">
                <div className="w-full lg:max-w-sm">
                    <Input
                        placeholder="Rechercher dans l'échéancier..."
                        value={searchTerm}
                        onChange={(event) => setSearchTerm(event.target.value)}
                        icon={Search}
                    />
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 w-full lg:w-auto">
                    <MultiSelect
                        value={filterCategories}
                        onChange={setFilterCategories}
                        options={categoryFilterOptions}
                        placeholder="Toutes les catégories"
                        className="w-full sm:w-56"
                    />
                    <MultiSelect
                        value={filterFrequencies}
                        onChange={setFilterFrequencies}
                        options={frequencyFilterOptions}
                        placeholder="Toutes les fréquences"
                        className="w-full sm:w-56"
                    />
                </div>
            </div>

            <div className="hidden md:flex flex-1 bg-white dark:bg-[#121212] rounded-xl border border-black/[0.05] dark:border-white/10 shadow-sm overflow-hidden flex-col min-h-[calc(100vh-170px)] max-h-[calc(100vh-170px)]">
                <Table
                    data={scheduledTransactions}
                    keyExtractor={(t) => t.id}
                    rowHeight={64}
                    emptyMessage={searchTerm || dueRange !== 'all' || filterCategories.length > 0 || filterFrequencies.length > 0
                        ? "Aucune transaction récurrente ne correspond aux filtres."
                        : "Aucune transaction récurrente configurée"
                    }
                    columns={[
                        {
                            header: 'Compte',
                            render: (transaction) => {
                                const account = accounts.find(a => a.id === transaction.accountId);
                                const isEnded = transaction.endDate && new Date(transaction.endDate) < new Date();
                                return (
                                    <div className="flex items-center gap-3 min-w-0">
                                        <div
                                            className={`w-1 h-6 rounded-full flex-none ${isEnded ? 'bg-red-50' : ''}`}
                                            style={{ backgroundColor: isEnded ? '#ef4444' : (account?.color || '#3b82f6') }}
                                        />
                                        <span className="font-medium truncate">{account?.name}</span>
                                    </div>
                                );
                            },
                            className: "text-gray-900 dark:text-gray-200 group-[.retro]:text-black"
                        },
                        {
                            header: 'ÉCHÉANCE',
                            render: (transaction) => (
                                <>
                                    <div className="flex items-center gap-2">
                                        <Calendar className="w-4 h-4 flex-none" />
                                        <span className="truncate">
                                            {format(new Date(transaction.nextDate), 'dd MMM yyyy', { locale: fr })}
                                        </span>
                                        {transaction.endDate && (
                                            <span className="text-xs text-gray-400 ml-1 truncate">
                                                → {format(new Date(transaction.endDate), 'dd MMM yyyy', { locale: fr })}
                                            </span>
                                        )}
                                    </div>
                                    {transaction.budgetId && (!transaction.endDate || new Date(transaction.endDate) >= new Date()) && (
                                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200 mt-1">
                                            Budget
                                        </span>
                                    )}
                                    {transaction.endDate && new Date(transaction.endDate) < new Date() && (
                                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200 mt-1">
                                            Terminé
                                        </span>
                                    )}
                                </>
                            ),
                            className: "text-gray-500 dark:text-gray-400 group-[.retro]:text-black whitespace-nowrap",
                            truncate: true
                        },
                        {
                            header: 'Fréquence',
                            render: (transaction) => (
                                <div className="flex items-center gap-2">
                                    <Clock className="w-4 h-4 text-gray-400 dark:text-gray-500 group-[.retro]:text-black" />
                                    {FREQUENCY_LABELS[transaction.frequency] || transaction.frequency}
                                </div>
                            ),
                            className: "text-gray-500 dark:text-gray-400 group-[.retro]:text-black whitespace-nowrap"
                        },
                        {
                            header: 'Catégorie',
                            render: (transaction) => {
                                const category = getCategoryDetails(transaction.category);
                                return transaction.type === 'transfer' ? (
                                    <div
                                        className="inline-flex items-center gap-2 px-2.5 py-0.5 rounded-full text-xs font-medium group-[.retro]:bg-transparent group-[.retro]:text-black group-[.retro]:border group-[.retro]:border-black group-[.retro]:rounded-none"
                                        style={{ backgroundColor: '#6366f120', color: '#6366f1' }}
                                    >
                                        {renderCategoryIcon('ArrowRightLeft', "w-3 h-3")}
                                        Virement
                                    </div>
                                ) : (
                                    <div
                                        className="inline-flex items-center gap-2 px-2.5 py-0.5 rounded-full text-xs font-medium group-[.retro]:bg-transparent group-[.retro]:text-black group-[.retro]:border group-[.retro]:border-black group-[.retro]:rounded-none"
                                        style={{ backgroundColor: `${category.color}20`, color: category.color }}
                                    >
                                        {renderCategoryIcon(category.icon, "w-3 h-3")}
                                        {category.name}
                                    </div>
                                );
                            },
                            className: "whitespace-nowrap"
                        },
                        {
                            header: 'Description',
                            render: (transaction) => (
                                <div className="flex flex-col">
                                    <span>{transaction.description}</span>
                                    {transaction.budgetId && (
                                        <span className="text-xs text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                                            <Tag className="w-3 h-3" />
                                            {budgetMap.get(transaction.budgetId)?.name || 'Budget lié'}
                                        </span>
                                    )}
                                </div>
                            ),
                            className: "text-gray-900 dark:text-gray-200 group-[.retro]:text-black"
                        },
                        {
                            header: 'Montant',
                            align: 'right',
                            render: (transaction) => (
                                <span className={`text-sm font-medium ${transaction.type === 'income' ? 'text-emerald-600' :
                                    transaction.type === 'transfer' ? 'text-blue-600 dark:text-blue-400' : 'text-red-600'
                                    } group-[.retro]:text-black`}>
                                    {transaction.type === 'income' ? '+' : transaction.type === 'transfer' ? '' : '-'}{new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(transaction.amount)} €
                                </span>
                            ),
                            className: "whitespace-nowrap"
                        },
                        {
                            header: 'Actions',
                            align: 'right',
                            render: (transaction) => (
                                <div className="flex items-center justify-end gap-2">
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => handleOpenModal(transaction)}
                                        className="text-primary-600 hover:text-primary-900 hover:bg-primary-50 dark:text-primary-400 dark:hover:text-primary-300 dark:hover:bg-primary-900/20"
                                        icon={Edit2}
                                    />
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => handleDeleteClick(transaction.id)}
                                        className="text-red-600 hover:text-red-900 hover:bg-red-50 dark:text-red-400 dark:hover:text-red-300 dark:hover:bg-red-900/20"
                                        icon={Trash2}
                                    />
                                </div>
                            ),
                            className: "whitespace-nowrap"
                        }
                    ]}
                    rowClassName={(transaction) => {
                        const isEnded = transaction.endDate && new Date(transaction.endDate) < new Date();
                        return isEnded ? 'bg-red-5 dark:bg-red-900/10' : '';
                    }}
                />
            </div>

            <div className="md:hidden space-y-3 pb-4">
                {scheduledTransactions.length > 0 ? (
                    scheduledTransactions.map(transaction => {
                        const account = accounts.find(a => a.id === transaction.accountId);
                        const category = getCategoryDetails(transaction.category);
                        const isIncome = transaction.type === 'income';
                        const isTransfer = transaction.type === 'transfer';
                        const isEnded = transaction.endDate && new Date(transaction.endDate) < new Date();

                        return (
                            <article 
                                key={transaction.id} 
                                className={`p-4 rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] shadow-sm relative overflow-hidden transition-all ${
                                    isEnded ? 'opacity-60 border-red-200 dark:border-red-900/30' : ''
                                }`}
                            >
                                <div className="flex items-start justify-between gap-3">
                                    <div className="flex items-start gap-3 min-w-0 flex-1">
                                        <div
                                            className="mt-0.5 h-10 w-10 shrink-0 rounded-2xl flex items-center justify-center"
                                            style={{ backgroundColor: isTransfer ? '#6366f116' : `${category.color}16`, color: isTransfer ? '#6366f1' : category.color }}
                                        >
                                            {isTransfer ? <ArrowRightLeft className="w-5 h-5" /> : renderCategoryIcon(category.icon, 'w-5 h-5')}
                                        </div>
                                        <div className="min-w-0 flex-1">
                                            <div className="flex items-start justify-between gap-2">
                                                <p className="text-[15px] font-semibold leading-tight text-gray-950 dark:text-white truncate">
                                                    {transaction.description}
                                                </p>
                                                <p className={`shrink-0 text-[15px] font-bold tabular-nums ${
                                                    isIncome ? 'text-emerald-600' : isTransfer ? 'text-blue-600 dark:text-blue-400' : 'text-red-600'
                                                }`}>
                                                    {isIncome ? '+' : isTransfer ? '' : '-'}{new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(transaction.amount)} €
                                                </p>
                                            </div>
                                            <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-gray-500 dark:text-neutral-400">
                                                <span className="truncate max-w-[120px]">{account?.name || 'Compte'}</span>
                                                <span className="h-1 w-1 rounded-full bg-gray-300 dark:bg-neutral-700" />
                                                <span className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[9px] font-bold uppercase tracking-tight" style={{ 
                                                    backgroundColor: isTransfer ? '#6366f114' : `${category.color}14`, 
                                                    color: isTransfer ? '#6366f1' : category.color 
                                                }}>
                                                    {isTransfer ? 'Virement' : category.name}
                                                </span>
                                                <span className="h-1 w-1 rounded-full bg-gray-300 dark:bg-neutral-700" />
                                                <span className="font-semibold text-primary-600 dark:text-primary-400">
                                                    {FREQUENCY_LABELS[transaction.frequency]}
                                                </span>
                                            </div>
                                            <div className="mt-2 pt-2 border-t border-black/[0.03] dark:border-white/[0.03] flex items-center justify-between text-[11px] text-gray-400 dark:text-neutral-500">
                                                <span className="flex items-center gap-1">
                                                    <Calendar className="w-3.5 h-3.5" />
                                                    Échéance : {format(new Date(transaction.nextDate), 'dd-MM-yyyy', { locale: fr })}
                                                </span>
                                                {transaction.endDate && (
                                                    <span className="text-xs">
                                                        Fin : {format(new Date(transaction.endDate), 'dd-MM-yyyy', { locale: fr })}
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                    
                                    <div className="flex shrink-0 flex-col items-center gap-1.5">
                                        <button
                                            type="button"
                                            onClick={() => handleOpenModal(transaction)}
                                            className="rounded-full p-1.5 text-gray-400 bg-gray-50 dark:bg-white/[0.04] hover:text-primary-500 transition-colors"
                                            aria-label="Modifier"
                                        >
                                            <Edit2 className="w-4 h-4" />
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => handleDeleteClick(transaction.id)}
                                            className="rounded-full p-1.5 text-red-500 bg-red-55 dark:bg-red-500/10 hover:bg-red-55/20 transition-colors"
                                            aria-label="Supprimer"
                                        >
                                            <Trash2 className="w-4 h-4" />
                                        </button>
                                    </div>
                                </div>
                            </article>
                        );
                    })
                ) : (
                    <div className="rounded-2xl border border-black/[0.05] dark:border-white/10 bg-white dark:bg-[#121212] p-8 text-center shadow-sm">
                        <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-gray-50 dark:bg-neutral-900">
                            <Search className="h-8 w-8 text-gray-300 dark:text-neutral-700" />
                        </div>
                        <p className="text-base font-bold text-gray-950 dark:text-white">Aucun échéancier</p>
                        <p className="mt-1 text-sm text-gray-500 dark:text-neutral-400">
                            {searchTerm || filterCategories.length > 0 || filterFrequencies.length > 0
                                ? 'Aucune transaction récurrente pour les filtres actuels.'
                                : 'Ajoute une transaction récurrente pour commencer.'}
                        </p>
                    </div>
                )}
            </div>

            <FormPopup
                isOpen={isSuggestionsOpen}
                onClose={() => setIsSuggestionsOpen(false)}
                title="Suggestions du journal"
                maxWidth="2xl"
            >
                <div className="p-4 space-y-2">
                    {monthlySuggestions.map(suggestion => {
                        const account = accounts.find(a => a.id === suggestion.accountId);
                        const category = getCategoryDetails(suggestion.category);

                        return (
                            <div
                                key={suggestion.suggestionKey}
                                className="rounded-xl border border-black/[0.05] dark:border-white/10 bg-gray-50 dark:bg-neutral-900/60 p-3 flex flex-col sm:flex-row sm:items-center gap-3 min-w-0"
                            >
                                <div className="flex items-center gap-3 min-w-0 flex-1 w-full">
                                    <div className="w-1 h-10 rounded-full flex-none" style={{ backgroundColor: account?.color || '#3b82f6' }} />
                                    <div className="min-w-0 flex-1">
                                        <div className="flex items-center gap-2 min-w-0">
                                            <span className="font-semibold text-sm text-gray-900 dark:text-gray-100 truncate">{suggestion.description}</span>
                                            <span
                                                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-tight flex-none max-w-[8rem] min-w-0"
                                                style={{ backgroundColor: `${category.color}15`, color: category.color }}
                                            >
                                                {renderCategoryIcon(category.icon, "w-3 h-3 flex-none")}
                                                <span className="truncate">{category.name}</span>
                                            </span>
                                        </div>
                                        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400 mt-1 min-w-0">
                                            <span className="truncate">{account?.name || 'Compte inconnu'}</span>
                                            <span className="text-gray-300 dark:text-gray-700">•</span>
                                            <span className="whitespace-nowrap">{format(parseLocalDate(suggestion.nextDate), 'dd MMM yyyy', { locale: fr })}</span>
                                            <span className="text-gray-300 dark:text-gray-700">•</span>
                                            <span className="whitespace-nowrap">{suggestion.occurrenceCount} mois</span>
                                        </div>
                                    </div>
                                </div>
                                <div className="flex flex-wrap items-center justify-between sm:justify-end gap-2 w-full sm:w-auto flex-none">
                                    <span className={`text-sm font-semibold tabular-nums whitespace-nowrap ${suggestion.type === 'income' ? 'text-emerald-600' :
                                        suggestion.type === 'transfer' ? 'text-blue-600 dark:text-blue-400' : 'text-red-600'
                                        }`}>
                                        {suggestion.type === 'income' ? '+' : suggestion.type === 'transfer' ? '' : '-'}{new Intl.NumberFormat('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(suggestion.amount)} €
                                    </span>
                                    <Button
                                        variant="secondary"
                                        size="sm"
                                        icon={Plus}
                                        onClick={() => handleAddSuggestion(suggestion)}
                                    >
                                        Ajouter
                                    </Button>
                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        icon={Trash2}
                                        onClick={() => handleDismissSuggestion(suggestion)}
                                        className="text-red-500 hover:text-red-600 hover:bg-red-500/10"
                                    >
                                        Supprimer
                                    </Button>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </FormPopup>

            <FormPopup
                isOpen={isModalOpen}
                onClose={() => setIsModalOpen(false)}
            >
                <form onSubmit={handleSubmit} className="p-6 space-y-6">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">{editingTransaction ? "Modifier la transaction" : "Nouvelle transaction récurrente"}</h3>
                    <div className="space-y-4">
                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date de début</label>
                                <Input
                                    type="date"
                                    required
                                    value={formData.nextDate}
                                    onChange={e => setFormData({ ...formData, nextDate: e.target.value })}
                                    className="focus:ring-primary-500 focus:border-primary-500"
                                />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Date de fin (optionnel)</label>
                                <div className="relative">
                                    <Input
                                        type="date"
                                        value={formData.endDate}
                                        onChange={e => setFormData({ ...formData, endDate: e.target.value })}
                                        min={formData.nextDate}
                                        className="focus:ring-primary-500 focus:border-primary-500"
                                    />
                                    {formData.endDate && (
                                        <button
                                            type="button"
                                            onClick={() => setFormData({ ...formData, endDate: '' })}
                                            className="absolute right-8 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
                                            title="Effacer la date de fin"
                                        >
                                            <X className="w-4 h-4" />
                                        </button>
                                    )}
                                </div>
                            </div>
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Fréquence</label>
                            <SearchableSelect
                                value={formData.frequency}
                                onChange={(value) => setFormData({ ...formData, frequency: value as Periodicity })}
                                options={[
                                    { id: 'once', label: 'Une seule fois', icon: 'Clock' },
                                    { id: 'daily', label: 'Journalière', icon: 'Clock' },
                                    { id: 'weekly', label: 'Hebdomadaire', icon: 'Clock' },
                                    { id: 'biweekly', label: 'Toutes les 2 semaines', icon: 'Clock' },
                                    { id: 'bimonthly', label: 'Bimensuelle (2x/mois)', icon: 'Clock' },
                                    { id: 'fourweekly', label: 'Toutes les 4 semaines', icon: 'Clock' },
                                    { id: 'monthly', label: 'Mensuelle', icon: 'Clock' },
                                    { id: 'bimestrial', label: 'Bimestrielle (tous les 2 mois)', icon: 'Clock' },
                                    { id: 'quarterly', label: 'Trimestrielle (tous les 3 mois)', icon: 'Clock' },
                                    { id: 'fourmonthly', label: 'Quadrimestrielle (tous les 4 mois)', icon: 'Clock' },
                                    { id: 'semiannual', label: 'Semestrielle (tous les 6 mois)', icon: 'Clock' },
                                    { id: 'annual', label: 'Annuelle', icon: 'Clock' },
                                    { id: 'biennial', label: 'Biennale (tous les 2 ans)', icon: 'Clock' }
                                ]}
                                placeholder="Sélectionner une fréquence"
                                size="md"
                            />
                        </div>

                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description</label>
                            <Input
                                type="text"
                                value={formData.description}
                                onChange={e => setFormData({ ...formData, description: e.target.value })}
                                className="focus:ring-primary-500 focus:border-primary-500"
                                placeholder="Ex: Loyer"
                            />
                        </div>

                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Montant</label>
                                <Input
                                    type="number"
                                    step="0.01"
                                    required
                                    value={formData.amount}
                                    onChange={e => setFormData({ ...formData, amount: e.target.value })}
                                    placeholder="0.00"
                                    rightElement="€"
                                    className="focus:ring-primary-500 focus:border-primary-500"
                                />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Type</label>
                                <SearchableSelect
                                    value={formData.type}
                                    onChange={(value) => setFormData({
                                        ...formData,
                                        type: value as TransactionType,
                                        linkToBudget: value === 'expense' ? formData.linkToBudget : false,
                                        budgetId: value === 'expense' ? formData.budgetId : ''
                                    })}
                                    options={[
                                        { id: 'expense', label: 'Dépense', icon: 'TrendingDown', color: '#ef4444' },
                                        { id: 'income', label: 'Revenu', icon: 'TrendingUp', color: '#10b981' },
                                        { id: 'transfer', label: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' }
                                    ]}
                                    placeholder="Sélectionner un type"
                                />
                            </div>
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
                                    <>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Catégorie</label>
                                        <SearchableSelect
                                            value={formData.categoryId}
                                            onChange={(value) => setFormData({ ...formData, categoryId: value })}
                                            options={categoryOptions}
                                            placeholder="Sélectionner une catégorie"
                                            disabled={formData.linkToBudget && !!formData.budgetId}
                                        />
                                    </>
                                )}
                            </div>
                        </div>

                        {formData.type === 'expense' && (
                            <div className="space-y-3">
                                <div className="flex items-center gap-2">
                                    <input
                                        type="checkbox"
                                        id="linkToBudget"
                                        checked={formData.linkToBudget}
                                        onChange={e => {
                                            const nextBudgetId = e.target.checked ? (formData.budgetId || budgetOptions[0]?.id || '') : '';
                                            setFormData({
                                                ...formData,
                                                ...(e.target.checked ? getBudgetFormPatch(nextBudgetId) : { budgetId: '' }),
                                                linkToBudget: e.target.checked
                                            });
                                        }}
                                        className="w-4 h-4 text-primary-600 rounded border-gray-300 focus:ring-primary-500"
                                    />
                                    <label htmlFor="linkToBudget" className="text-sm text-gray-700 dark:text-gray-300">
                                        Lier à un budget
                                    </label>
                                </div>

                                {formData.linkToBudget && (
                                    budgetOptions.length > 0 ? (
                                        <SearchableSelect
                                            value={formData.budgetId}
                                            onChange={(value) => setFormData({ ...formData, ...getBudgetFormPatch(value) })}
                                            options={budgetOptions}
                                            placeholder="Sélectionner un budget"
                                        />
                                    ) : (
                                        <div className="text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-neutral-900 border border-black/[0.05] dark:border-white/10 rounded-lg p-3">
                                            Aucun budget configuré. Crée un budget depuis la page Budget pour pouvoir le lier.
                                        </div>
                                    )
                                )}
                            </div>
                        )}

                        <div className="pt-4 flex gap-3">
                            <Button
                                type="button"
                                variant="secondary"
                                onClick={() => setIsModalOpen(false)}
                                className="flex-1"
                            >
                                Annuler
                            </Button>
                            <Button
                                type="submit"
                                className="flex-1"
                            >
                                {editingTransaction ? 'Modifier' : 'Ajouter'}
                            </Button>
                        </div>
                    </div>
                </form>
            </FormPopup>

            <ConfirmModal
                isOpen={isDeleteModalOpen}
                onClose={() => setIsDeleteModalOpen(false)}
                onConfirm={handleConfirmDelete}
                title="Supprimer la transaction récurrente"
                message="Êtes-vous sûr de vouloir supprimer cette transaction récurrente ?"
                confirmLabel="Supprimer"
                isDangerous={true}
            />
        </div >
    );
};

export default Scheduled;
