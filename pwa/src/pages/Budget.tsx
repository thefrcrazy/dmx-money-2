import React, { useEffect, useMemo, useState } from 'react';
import { useBank } from '../context/BankContext';
import { useSettings } from '../context/SettingsContext';
import { endOfMonth, format, startOfMonth, subMonths } from 'date-fns';
import { fr } from 'date-fns/locale';
import { AlertCircle, ArrowRightLeft, CalendarClock, CheckCircle2, Edit2, Plus, Search, Sparkles, Tag, Trash2, TrendingDown, Wallet } from 'lucide-react';
import Button from '../components/ui/Button';
import FormPopup from '../components/ui/FormPopup';
import ConfirmModal from '../components/ui/ConfirmModal';
import Input from '../components/ui/Input';
import SearchableSelect, { SelectOption } from '../components/ui/SearchableSelect';
import MultiSelect from '../components/ui/MultiSelect';
import { ICONS } from '../constants/icons';
import { formatCurrency } from '../utils/format';
import { Budget as BudgetModel, Category } from '../types';
import { useToast } from '../context/ToastContext';

const parseLocalDate = (value: string) => {
    const [year, month, day] = value.split('-').map(Number);
    return new Date(year, (month || 1) - 1, day || 1);
};

const isDateInRange = (date: Date, start: Date, end: Date) => date >= start && date <= end;
const clampPercent = (value: number) => Math.max(0, Math.min(value, 100));
const normalizeSearchValue = (value: unknown) => String(value ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '');

const getCategoryFallback = (id: string): Category => ({
    id,
    name: 'Inconnu',
    icon: 'Tag',
    color: '#9ca3af'
});

interface BudgetSuggestion {
    suggestionKey: string;
    name: string;
    amount: number;
    category: string;
    accountId?: string;
    monthCount: number;
    currentMonthSpent: number;
}

interface BudgetDetail {
    budget: BudgetModel;
    accountName: string;
    spent: number;
    remaining: number;
    progress: number;
    linkedCount: number;
}

const mergeSuggestionKeys = (...sources: Array<string[] | undefined>) => (
    Array.from(new Set(sources.flatMap(source => source || [])))
);

const Budget: React.FC = () => {
    const {
        accounts,
        transactions,
        scheduled,
        categories,
        budgets,
        addBudget,
        updateBudget,
        deleteBudget,
        filterAccount
    } = useBank();
    const { settings, updateDismissedBudgetSuggestions } = useSettings();
    const { showToast } = useToast();

    const [isBudgetModalOpen, setIsBudgetModalOpen] = useState(false);
    const [editingBudget, setEditingBudget] = useState<BudgetModel | null>(null);
    const [budgetToDelete, setBudgetToDelete] = useState<string | null>(null);
    const [isSuggestionsOpen, setIsSuggestionsOpen] = useState(false);
    const [searchTerm, setSearchTerm] = useState('');
    const [filterCategories, setFilterCategories] = useState<string[]>([]);
    const dismissedSuggestionKeys = useMemo(
        () => new Set(settings.dismissedBudgetSuggestions || []),
        [settings.dismissedBudgetSuggestions]
    );
    const [formData, setFormData] = useState({
        name: '',
        amount: '',
        category: '',
        accountId: 'all'
    });

    const now = new Date();
    const monthStart = startOfMonth(now);
    const monthEnd = endOfMonth(now);
    const daysInMonth = monthEnd.getDate();
    const currentDay = Math.min(now.getDate(), daysInMonth);
    const remainingDays = Math.max(daysInMonth - currentDay + 1, 1);

    const categoryMap = useMemo(() => new Map(categories.map(category => [category.id, category])), [categories]);
    const accountMap = useMemo(() => new Map(accounts.map(account => [account.id, account])), [accounts]);

    const visibleBudgets = useMemo(() => {
        return filterAccount.length === 0
            ? budgets
            : budgets.filter(budget => !budget.accountId || filterAccount.includes(budget.accountId));
    }, [budgets, filterAccount]);

    const filteredTransactions = useMemo(() => {
        return filterAccount.length === 0
            ? transactions
            : transactions.filter(transaction => filterAccount.includes(transaction.accountId));
    }, [transactions, filterAccount]);

    const currentMonthExpenses = useMemo(() => {
        return filteredTransactions.filter(transaction => {
            if (transaction.type !== 'expense' || transaction.category === 'transfer') return false;
            return isDateInRange(parseLocalDate(transaction.date), monthStart, monthEnd);
        });
    }, [filteredTransactions, monthStart, monthEnd]);

    const budgetByCategory = useMemo(() => {
        return visibleBudgets.reduce((acc, budget) => {
            acc[budget.category] = (acc[budget.category] || 0) + budget.amount;
            return acc;
        }, {} as Record<string, number>);
    }, [visibleBudgets]);

    const spentByCategory = useMemo(() => {
        return currentMonthExpenses.reduce((acc, transaction) => {
            acc[transaction.category] = (acc[transaction.category] || 0) + transaction.amount;
            return acc;
        }, {} as Record<string, number>);
    }, [currentMonthExpenses]);

    const allCategoryRows = useMemo(() => {
        const categoryIds = Array.from(new Set([
            ...Object.keys(budgetByCategory),
            ...Object.keys(spentByCategory)
        ]));

        return categoryIds
            .map(id => {
                const category = categoryMap.get(id) || getCategoryFallback(id);
                const budgeted = budgetByCategory[id] || 0;
                const spent = spentByCategory[id] || 0;
                const remaining = budgeted - spent;
                const progress = budgeted > 0 ? (spent / budgeted) * 100 : spent > 0 ? 100 : 0;

                return {
                    id,
                    category,
                    budgeted,
                    spent,
                    remaining,
                    progress,
                    isOverBudget: budgeted > 0 && spent > budgeted,
                    isUnbudgeted: budgeted === 0 && spent > 0
                };
            })
            .sort((a, b) => {
                if (a.isOverBudget !== b.isOverBudget) return a.isOverBudget ? -1 : 1;
                if (a.isUnbudgeted !== b.isUnbudgeted) return a.isUnbudgeted ? -1 : 1;
                return Math.max(b.budgeted, b.spent) - Math.max(a.budgeted, a.spent);
            });
    }, [budgetByCategory, spentByCategory, categoryMap]);

    const categoryRows = useMemo(() => allCategoryRows.filter(row => row.budgeted > 0), [allCategoryRows]);

    const totals = useMemo(() => {
        const totalBudgeted = visibleBudgets.reduce((sum, budget) => sum + budget.amount, 0);
        const totalSpent = currentMonthExpenses.reduce((sum, transaction) => sum + transaction.amount, 0);
        const remaining = totalBudgeted - totalSpent;
        const expectedSpend = totalBudgeted * (currentDay / daysInMonth);
        const paceDelta = totalSpent - expectedSpend;

        return {
            totalBudgeted,
            totalSpent,
            remaining,
            progress: totalBudgeted > 0 ? (totalSpent / totalBudgeted) * 100 : 0,
            expectedSpend,
            paceDelta,
            remainingPerDay: remaining / remainingDays
        };
    }, [visibleBudgets, currentMonthExpenses, currentDay, daysInMonth, remainingDays]);

    const unbudgetedRows = allCategoryRows.filter(row => row.isUnbudgeted);
    const overBudgetRows = categoryRows.filter(row => row.isOverBudget);

    const linkedScheduledCountByBudget = useMemo(() => {
        return scheduled.reduce((acc, item) => {
            if (item.budgetId) {
                acc[item.budgetId] = (acc[item.budgetId] || 0) + 1;
            }
            return acc;
        }, {} as Record<string, number>);
    }, [scheduled]);

    const linkedScheduledByBudgetId = useMemo(() => {
        return scheduled.reduce((acc, item) => {
            if (item.budgetId) {
                acc[item.budgetId] = [...(acc[item.budgetId] || []), item].sort((a, b) => (
                    parseLocalDate(a.nextDate).getTime() - parseLocalDate(b.nextDate).getTime()
                ));
            }
            return acc;
        }, {} as Record<string, typeof scheduled>);
    }, [scheduled]);

    const budgetDetailsByCategory = useMemo(() => {
        return visibleBudgets.reduce((acc, budget) => {
            const spent = currentMonthExpenses
                .filter(transaction => transaction.category === budget.category && (!budget.accountId || transaction.accountId === budget.accountId))
                .reduce((sum, transaction) => sum + transaction.amount, 0);
            const account = budget.accountId ? accountMap.get(budget.accountId) : undefined;
            const detail: BudgetDetail = {
                budget,
                accountName: account?.name || 'Tous les comptes',
                spent,
                remaining: budget.amount - spent,
                progress: budget.amount > 0 ? (spent / budget.amount) * 100 : 0,
                linkedCount: linkedScheduledCountByBudget[budget.id] || 0
            };

            acc[budget.category] = [...(acc[budget.category] || []), detail].sort((a, b) => {
                const remainingDiff = a.remaining - b.remaining;
                return remainingDiff || a.budget.name.localeCompare(b.budget.name, 'fr');
            });
            return acc;
        }, {} as Record<string, BudgetDetail[]>);
    }, [accountMap, currentMonthExpenses, linkedScheduledCountByBudget, visibleBudgets]);

    const filteredCategoryRows = useMemo(() => {
        const searchTokens = normalizeSearchValue(searchTerm).split(/\s+/).filter(Boolean);

        return categoryRows.filter(row => {
            if (filterCategories.length > 0 && !filterCategories.includes(row.id)) return false;
            if (searchTokens.length === 0) return true;

            const details = budgetDetailsByCategory[row.id] || [];
            const linkedScheduledItems = details.flatMap(detail => linkedScheduledByBudgetId[detail.budget.id] || []);
            const searchableText = [
                row.category.name,
                row.budgeted,
                row.spent,
                row.remaining,
                ...details.flatMap(detail => [
                    detail.budget.name,
                    detail.accountName,
                    detail.budget.amount,
                    detail.spent,
                    detail.remaining
                ]),
                ...linkedScheduledItems.flatMap(item => [
                    item.description,
                    item.amount,
                    item.nextDate,
                    format(parseLocalDate(item.nextDate), 'dd MMM yyyy', { locale: fr })
                ])
            ].map(normalizeSearchValue).join(' ');

            return searchTokens.every(token => searchableText.includes(token));
        });
    }, [budgetDetailsByCategory, categoryRows, filterCategories, linkedScheduledByBudgetId, searchTerm]);

    const categoryOptions: SelectOption[] = categories
        .filter(category => category.id !== 'transfer')
        .map(category => ({
            id: category.id,
            label: category.name,
            icon: category.icon,
            color: category.color
        }));

    const accountOptions: SelectOption[] = [
        { id: 'all', label: 'Tous les comptes', icon: 'Wallet' },
        ...accounts.map(account => ({
            id: account.id,
            label: account.name,
            icon: account.icon || 'Wallet',
            color: account.color
        }))
    ];

    const budgetSuggestions = useMemo<BudgetSuggestion[]>(() => {
        const groupBySingleAccount = filterAccount.length === 1;
        const startDate = startOfMonth(subMonths(now, 5));
        const currentMonthKey = `${monthStart.getFullYear()}-${monthStart.getMonth()}`;

        const hasExistingBudget = (categoryId: string, accountId?: string) => budgets.some(budget => {
            if (budget.category !== categoryId) return false;
            if (!budget.accountId) return true;
            return !accountId || budget.accountId === accountId;
        });

        const groups = new Map<string, {
            category: string;
            accountId?: string;
            total: number;
            currentMonthSpent: number;
            months: Set<string>;
        }>();

        filteredTransactions.forEach(transaction => {
            if (transaction.type !== 'expense' || transaction.category === 'transfer') return;

            const transactionDate = parseLocalDate(transaction.date);
            if (transactionDate < startDate || transactionDate > monthEnd) return;

            const suggestionAccountId = groupBySingleAccount ? transaction.accountId : undefined;
            if (hasExistingBudget(transaction.category, suggestionAccountId)) return;

            const suggestionKey = `${transaction.category}|${suggestionAccountId || 'all'}`;
            if (dismissedSuggestionKeys.has(suggestionKey)) return;

            const monthKey = `${transactionDate.getFullYear()}-${transactionDate.getMonth()}`;
            const group = groups.get(suggestionKey) || {
                category: transaction.category,
                accountId: suggestionAccountId,
                total: 0,
                currentMonthSpent: 0,
                months: new Set<string>()
            };

            group.total += transaction.amount;
            group.months.add(monthKey);
            if (monthKey === currentMonthKey) {
                group.currentMonthSpent += transaction.amount;
            }

            groups.set(suggestionKey, group);
        });

        return Array.from(groups.entries())
            .map(([suggestionKey, group]) => {
                const category = categoryMap.get(group.category) || getCategoryFallback(group.category);
                const averageMonthly = group.total / Math.max(group.months.size, 1);
                const suggestedAmount = Math.ceil(Math.max(averageMonthly, group.currentMonthSpent) * 100) / 100;

                return {
                    suggestionKey,
                    name: category.name,
                    amount: suggestedAmount,
                    category: group.category,
                    accountId: group.accountId,
                    monthCount: group.months.size,
                    currentMonthSpent: group.currentMonthSpent
                };
            })
            .filter(suggestion => suggestion.amount > 0)
            .sort((a, b) => b.amount - a.amount);
    }, [budgets, categoryMap, dismissedSuggestionKeys, filterAccount.length, filteredTransactions, monthEnd, monthStart, now]);

    useEffect(() => {
        if (budgetSuggestions.length === 0) {
            setIsSuggestionsOpen(false);
        }
    }, [budgetSuggestions.length]);

    const renderCategoryIcon = (iconName: string, className: string = "w-4 h-4") => {
        if (iconName === 'ArrowRightLeft') return <ArrowRightLeft className={className} />;
        if (iconName === 'AlertCircle') return <AlertCircle className={className} />;
        const Icon = ICONS[iconName] || Tag;
        return <Icon className={className} />;
    };

    const handleOpenBudgetModal = (budget?: BudgetModel) => {
        if (budget) {
            setEditingBudget(budget);
            setFormData({
                name: budget.name,
                amount: budget.amount.toString(),
                category: budget.category,
                accountId: budget.accountId || 'all'
            });
        } else {
            setEditingBudget(null);
            setFormData({
                name: '',
                amount: '',
                category: categoryOptions[0]?.id || '',
                accountId: filterAccount.length === 1 ? filterAccount[0] : 'all'
            });
        }
        setIsBudgetModalOpen(true);
    };

    const handleAddSuggestion = async (suggestion: BudgetSuggestion) => {
        try {
            await addBudget({
                name: suggestion.name,
                amount: suggestion.amount,
                category: suggestion.category,
                accountId: suggestion.accountId
            });
            showToast("Budget ajouté", "success");
        } catch {
            showToast("Erreur lors de l'ajout du budget", "error");
        }
    };

    const handleDismissSuggestion = (suggestion: BudgetSuggestion) => {
        updateDismissedBudgetSuggestions(
            mergeSuggestionKeys(settings.dismissedBudgetSuggestions, [suggestion.suggestionKey])
        ).catch(() => {
            showToast("Erreur lors de la synchronisation de la suggestion", "error");
        });
        showToast("Suggestion supprimée", "success");
    };

    const handleSubmitBudget = async (event: React.FormEvent) => {
        event.preventDefault();
        const budgetData = {
            name: formData.name.trim(),
            amount: parseFloat(formData.amount),
            category: formData.category,
            accountId: formData.accountId === 'all' ? undefined : formData.accountId
        };

        if (!budgetData.name || isNaN(budgetData.amount) || budgetData.amount <= 0 || !budgetData.category) return;

        if (editingBudget) {
            await updateBudget({ ...budgetData, id: editingBudget.id });
        } else {
            await addBudget(budgetData);
        }

        setIsBudgetModalOpen(false);
    };

    const handleConfirmDeleteBudget = async () => {
        if (budgetToDelete) {
            await deleteBudget(budgetToDelete);
            setBudgetToDelete(null);
        }
    };

    const budgetState = totals.totalBudgeted === 0
        ? { label: 'À configurer', color: 'text-gray-500', icon: CalendarClock }
        : totals.remaining >= 0
            ? { label: 'Sous contrôle', color: 'text-emerald-600', icon: CheckCircle2 }
            : { label: 'Dépassement', color: 'text-red-600', icon: AlertCircle };
    const BudgetStateIcon = budgetState.icon;

    return (
        <div className="space-y-6" style={{ gap: `${settings.componentSpacing * 4}px` }}>
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 w-full">
                <div className="hidden md:block">
                    <h2 className="text-2xl font-bold text-gray-900 dark:text-gray-200">Budget</h2>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                        {format(now, 'MMMM yyyy', { locale: fr })} · budgets configurés et dépenses du Journal
                    </p>
                </div>
                <div className="flex items-center gap-2 w-full sm:w-auto">
                    {budgetSuggestions.length > 0 && (
                        <div className="flex-1 sm:flex-none">
                            <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => setIsSuggestionsOpen(true)}
                                icon={Sparkles}
                                className="w-full"
                            >
                                Suggestions ({budgetSuggestions.length})
                            </Button>
                        </div>
                    )}
                    <Button variant="primary" size="sm" onClick={() => handleOpenBudgetModal()} icon={Plus} className="flex-1 sm:flex-none">
                        Nouveau budget
                    </Button>
                </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4" style={{ gap: `${settings.componentSpacing * 3}px` }}>
                <div className="app-card p-5" style={{ padding: `${settings.componentPadding * 3}px` }}>
                    <div className="flex items-center justify-between gap-3 mb-3">
                        <span className="text-xs font-bold uppercase tracking-widest text-gray-500 dark:text-gray-400">Prévu</span>
                        <CalendarClock className="w-4 h-4 text-primary-500" />
                    </div>
                    <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">{formatCurrency(totals.totalBudgeted, 0)}</div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{visibleBudgets.length} budget{visibleBudgets.length > 1 ? 's' : ''} configuré{visibleBudgets.length > 1 ? 's' : ''}</p>
                </div>

                <div className="app-card p-5" style={{ padding: `${settings.componentPadding * 3}px` }}>
                    <div className="flex items-center justify-between gap-3 mb-3">
                        <span className="text-xs font-bold uppercase tracking-widest text-gray-500 dark:text-gray-400">Dépensé</span>
                        <TrendingDown className="w-4 h-4 text-red-500" />
                    </div>
                    <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">{formatCurrency(totals.totalSpent, 0)}</div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{currentMonthExpenses.length} dépense{currentMonthExpenses.length > 1 ? 's' : ''} ce mois-ci</p>
                </div>

                <div className="app-card p-5" style={{ padding: `${settings.componentPadding * 3}px` }}>
                    <div className="flex items-center justify-between gap-3 mb-3">
                        <span className="text-xs font-bold uppercase tracking-widest text-gray-500 dark:text-gray-400">Restant</span>
                        <Wallet className={`w-4 h-4 ${totals.remaining >= 0 ? 'text-emerald-500' : 'text-red-500'}`} />
                    </div>
                    <div className={`text-2xl font-bold ${totals.remaining >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>{formatCurrency(totals.remaining, 0)}</div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{formatCurrency(totals.remainingPerDay, 0)} / jour restant</p>
                </div>

                <div className="app-card p-5" style={{ padding: `${settings.componentPadding * 3}px` }}>
                    <div className="flex items-center justify-between gap-3 mb-3">
                        <span className="text-xs font-bold uppercase tracking-widest text-gray-500 dark:text-gray-400">État</span>
                        <BudgetStateIcon className={`w-4 h-4 ${budgetState.color}`} />
                    </div>
                    <div className={`text-2xl font-bold ${budgetState.color}`}>{budgetState.label}</div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                        {totals.paceDelta > 0 ? `${formatCurrency(totals.paceDelta, 0)} au-dessus du rythme` : `${formatCurrency(Math.abs(totals.paceDelta), 0)} sous le rythme`}
                    </p>
                </div>
            </div>

            <div className="app-card p-5" style={{ padding: `${settings.componentPadding * 3}px` }}>
                <div className="flex flex-col md:flex-row md:items-end justify-between gap-3 mb-4">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">Consommation du mois</h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400">Le budget suit les enveloppes configurées et les dépenses du Journal.</p>
                    </div>
                    <div className="text-sm text-gray-500 dark:text-gray-400">
                        <span className="font-semibold text-gray-900 dark:text-gray-200">{Math.round(totals.progress)}%</span> utilisé
                    </div>
                </div>
                <div className="h-3 rounded-full bg-gray-100 dark:bg-neutral-800 overflow-hidden">
                    <div
                        className={`h-full rounded-full transition-all duration-300 ${totals.progress > 100 ? 'bg-red-500' : 'bg-emerald-500'}`}
                        style={{ width: `${clampPercent(totals.progress)}%` }}
                    />
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-2 text-xs text-gray-500 dark:text-gray-400">
                    <span>{formatCurrency(totals.totalSpent)} dépensés</span>
                    <span>{formatCurrency(totals.totalBudgeted)} prévus</span>
                    <span>{formatCurrency(totals.expectedSpend)} théoriques au {format(now, 'dd MMM', { locale: fr })}</span>
                    {overBudgetRows.length > 0 && <span className="text-red-600">{overBudgetRows.length} dépassement{overBudgetRows.length > 1 ? 's' : ''}</span>}
                    {unbudgetedRows.length > 0 && <span className="text-primary-600 dark:text-primary-400">{unbudgetedRows.length} catégorie{unbudgetedRows.length > 1 ? 's' : ''} non budgétée{unbudgetedRows.length > 1 ? 's' : ''}</span>}
                </div>
            </div>

            <div className="app-card overflow-hidden">
                <div className="px-5 py-4 border-b border-black/[0.05] dark:border-white/10 flex items-center justify-between gap-3">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">Budget par catégorie</h3>
                        <p className="text-xs text-gray-500 dark:text-gray-400">Catégories configurées, enveloppes et suivi détaillé.</p>
                    </div>
                    <span className="text-xs font-semibold text-gray-500 dark:text-gray-400">
                        {filteredCategoryRows.length}{filteredCategoryRows.length !== categoryRows.length ? ` / ${categoryRows.length}` : ''} catégorie{categoryRows.length > 1 ? 's' : ''} budgétée{categoryRows.length > 1 ? 's' : ''}
                    </span>
                </div>

                <div className="px-5 py-3 border-b border-black/[0.05] dark:border-white/10 flex flex-col lg:flex-row gap-3">
                    <div className="w-full lg:max-w-sm">
                        <Input
                            placeholder="Rechercher un budget..."
                            value={searchTerm}
                            onChange={(event) => setSearchTerm(event.target.value)}
                            icon={Search}
                        />
                    </div>
                    <MultiSelect
                        value={filterCategories}
                        onChange={setFilterCategories}
                        options={categoryOptions}
                        placeholder="Toutes les catégories"
                        className="w-full sm:w-56"
                    />
                </div>

                {filteredCategoryRows.length > 0 ? (
                    <div className="divide-y divide-gray-100 dark:divide-neutral-800">
                        {filteredCategoryRows.map(row => {
                            const details = budgetDetailsByCategory[row.id] || [];
                            const linkedScheduledItems = details.flatMap(detail => (
                                (linkedScheduledByBudgetId[detail.budget.id] || []).map(item => ({
                                    item,
                                    budget: detail.budget,
                                    accountName: detail.accountName
                                }))
                            ));

                            return (
                                <div key={row.id} className="px-5 py-4">
                                    <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-3 mb-3">
                                        <div className="flex items-center gap-3 min-w-0">
                                            <div
                                                className="w-9 h-9 rounded-lg flex items-center justify-center flex-none"
                                                style={{ backgroundColor: `${row.category.color}18`, color: row.category.color }}
                                            >
                                                {renderCategoryIcon(row.category.icon, "w-4 h-4")}
                                            </div>
                                            <div className="min-w-0">
                                                <div className="font-semibold text-gray-900 dark:text-gray-100 truncate">{row.category.name}</div>
                                                <div className="text-xs text-gray-500 dark:text-gray-400">
                                                    {details.length} enveloppe{details.length > 1 ? 's' : ''} · {linkedScheduledItems.length} échéance{linkedScheduledItems.length > 1 ? 's' : ''} liée{linkedScheduledItems.length > 1 ? 's' : ''}
                                                </div>
                                            </div>
                                        </div>
                                        {row.isOverBudget ? (
                                            <span className="inline-flex items-center gap-1 text-xs font-semibold text-red-600 flex-none">
                                                <AlertCircle className="w-3.5 h-3.5" />
                                                Dépassé
                                            </span>
                                        ) : (
                                            <span className="inline-flex items-center gap-1 text-xs font-semibold text-emerald-600 flex-none">
                                                <CheckCircle2 className="w-3.5 h-3.5" />
                                                OK
                                            </span>
                                        )}
                                    </div>

                                    <div className="rounded-lg border border-black/[0.05] dark:border-white/10 overflow-hidden">
                                        <div className="hidden lg:grid grid-cols-[minmax(0,1fr)_95px_95px_95px_74px] gap-3 px-4 py-2 text-[11px] font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-neutral-900/60">
                                            <span>Budget</span>
                                            <span>Prévu</span>
                                            <span>Dépensé</span>
                                            <span>Restant</span>
                                            <span className="text-right">Actions</span>
                                        </div>
                                        <div className="divide-y divide-gray-100 dark:divide-neutral-800">
                                            {details.map(detail => {
                                                const detailOverBudget = detail.remaining < 0;
                                                const detailBarColor = detailOverBudget ? 'bg-red-500' : 'bg-emerald-500';
                                                const linkedForBudget = linkedScheduledByBudgetId[detail.budget.id] || [];

                                                return (
                                                    <div key={detail.budget.id} className="px-4 py-3">
                                                        <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_95px_95px_95px_74px] gap-3 lg:items-center text-sm">
                                                            <div className="min-w-0">
                                                                <div className="font-medium text-gray-900 dark:text-gray-100 truncate">{detail.budget.name}</div>
                                                                <div className="text-xs text-gray-500 dark:text-gray-400 truncate">{detail.accountName}</div>
                                                            </div>
                                                            <div className="grid grid-cols-3 gap-3 lg:contents">
                                                                <div>
                                                                    <div className="lg:hidden text-[11px] text-gray-500 dark:text-gray-400">Prévu</div>
                                                                    <div className="font-semibold text-gray-900 dark:text-gray-100 tabular-nums">{formatCurrency(detail.budget.amount)}</div>
                                                                </div>
                                                                <div>
                                                                    <div className="lg:hidden text-[11px] text-gray-500 dark:text-gray-400">Dépensé</div>
                                                                    <div className="font-semibold text-gray-900 dark:text-gray-100 tabular-nums">{formatCurrency(detail.spent)}</div>
                                                                </div>
                                                                <div>
                                                                    <div className="lg:hidden text-[11px] text-gray-500 dark:text-gray-400">Restant</div>
                                                                    <div className={`font-semibold tabular-nums ${detailOverBudget ? 'text-red-600' : 'text-emerald-600'}`}>
                                                                        {formatCurrency(detail.remaining)}
                                                                    </div>
                                                                </div>
                                                            </div>
                                                            <div className="flex items-center justify-end gap-1">
                                                                <Button variant="ghost" size="sm" icon={Edit2} onClick={() => handleOpenBudgetModal(detail.budget)} className="h-8 w-8 p-0" />
                                                                <Button variant="ghost" size="sm" icon={Trash2} onClick={() => setBudgetToDelete(detail.budget.id)} className="h-8 w-8 p-0 text-red-500 hover:text-red-600" />
                                                            </div>
                                                        </div>

                                                        <div className="mt-2">
                                                            <div className="h-1.5 rounded-full bg-gray-100 dark:bg-neutral-800 overflow-hidden">
                                                                <div
                                                                    className={`h-full rounded-full ${detailBarColor}`}
                                                                    style={{ width: `${clampPercent(detail.progress)}%` }}
                                                                />
                                                            </div>
                                                            <div className="mt-1 flex justify-between text-[11px] text-gray-500 dark:text-gray-400">
                                                                <span>{Math.round(detail.progress)}% utilisé</span>
                                                                {detailOverBudget && <span className="text-red-600">{formatCurrency(Math.abs(detail.remaining))} au-dessus</span>}
                                                            </div>
                                                        </div>

                                                        {linkedForBudget.length > 0 && (
                                                            <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-black/[0.04] dark:border-white/10 pt-2 text-xs text-gray-500 dark:text-gray-400">
                                                                <span className="inline-flex items-center gap-1 font-medium text-gray-600 dark:text-gray-300">
                                                                    <CalendarClock className="w-3.5 h-3.5 text-primary-500" />
                                                                    Échéances
                                                                </span>
                                                                {linkedForBudget.map(item => (
                                                                    <span key={item.id} className="inline-flex items-center gap-1 min-w-0">
                                                                        <span className="truncate max-w-[14rem]">{item.description}</span>
                                                                        <span className={item.type === 'income' ? 'text-emerald-600' : item.type === 'transfer' ? 'text-blue-600 dark:text-blue-400' : 'text-red-600'}>
                                                                            {item.type === 'income' ? '+' : item.type === 'transfer' ? '' : '-'}{formatCurrency(item.amount)}
                                                                        </span>
                                                                        <span>·</span>
                                                                        <span>{format(parseLocalDate(item.nextDate), 'dd MMM', { locale: fr })}</span>
                                                                    </span>
                                                                ))}
                                                            </div>
                                                        )}
                                                    </div>
                                                );
                                            })}
                                        </div>
                                    </div>
                                </div>
                            );
                        })}
                    </div>
                ) : (
                    <div className="px-6 py-14 text-center">
                        <div className="w-12 h-12 rounded-full bg-gray-100 dark:bg-neutral-900 flex items-center justify-center mx-auto mb-3">
                            {categoryRows.length === 0 ? <Wallet className="w-6 h-6 text-gray-400" /> : <Search className="w-6 h-6 text-gray-400" />}
                        </div>
                        {categoryRows.length === 0 ? (
                            <>
                                <p className="font-medium text-gray-900 dark:text-gray-200">Aucun budget configuré</p>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">Crée une enveloppe mensuelle, même sans échéance liée.</p>
                                <Button className="mt-4" variant="secondary" size="sm" onClick={() => handleOpenBudgetModal()} icon={Plus}>
                                    Nouveau budget
                                </Button>
                            </>
                        ) : (
                            <>
                                <p className="font-medium text-gray-900 dark:text-gray-200">Aucun budget ne correspond aux filtres.</p>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">Modifie la recherche ou les catégories sélectionnées.</p>
                            </>
                        )}
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
                    {budgetSuggestions.map(suggestion => {
                        const category = categoryMap.get(suggestion.category) || getCategoryFallback(suggestion.category);
                        const account = suggestion.accountId ? accountMap.get(suggestion.accountId) : undefined;

                        return (
                            <div
                                key={suggestion.suggestionKey}
                                className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 rounded-xl border border-black/[0.05] dark:border-white/10 bg-gray-50 dark:bg-neutral-900/60 px-3 py-3"
                            >
                                <div className="flex items-center gap-3 min-w-0 flex-1 w-full">
                                    <div
                                        className="w-10 h-10 rounded-xl flex items-center justify-center flex-none"
                                        style={{ backgroundColor: `${category.color}18`, color: category.color }}
                                    >
                                        {renderCategoryIcon(category.icon, "w-4 h-4")}
                                    </div>
                                    <div className="min-w-0">
                                        <div className="flex items-center gap-2 min-w-0">
                                            <span className="font-semibold text-sm text-gray-900 dark:text-gray-100 truncate">{suggestion.name}</span>
                                        </div>
                                        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-gray-500 dark:text-gray-400">
                                            <span>{account?.name || 'Tous les comptes'}</span>
                                            <span>•</span>
                                            <span>{suggestion.monthCount} mois observé{suggestion.monthCount > 1 ? 's' : ''}</span>
                                            {suggestion.currentMonthSpent > 0 && (
                                                <>
                                                    <span>•</span>
                                                    <span>{formatCurrency(suggestion.currentMonthSpent)} ce mois-ci</span>
                                                </>
                                            )}
                                        </div>
                                    </div>
                                </div>
                                <div className="flex flex-wrap items-center justify-between sm:justify-end gap-2 flex-none w-full sm:w-auto">
                                    <span className="text-sm font-semibold tabular-nums whitespace-nowrap text-primary-600 dark:text-primary-400">
                                        {formatCurrency(suggestion.amount)}
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

            <FormPopup isOpen={isBudgetModalOpen} onClose={() => setIsBudgetModalOpen(false)}>
                <form onSubmit={handleSubmitBudget} className="p-6 space-y-5">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">
                        {editingBudget ? 'Modifier le budget' : 'Nouveau budget'}
                    </h3>

                    <Input
                        label="Nom"
                        required
                        value={formData.name}
                        onChange={event => setFormData({ ...formData, name: event.target.value })}
                        placeholder="Ex: Courses, Essence, Loisirs"
                    />

                    <div className="grid grid-cols-2 gap-4">
                        <Input
                            label="Montant mensuel"
                            type="number"
                            step="0.01"
                            required
                            value={formData.amount}
                            onChange={event => setFormData({ ...formData, amount: event.target.value })}
                            rightElement="€"
                            placeholder="0.00"
                        />
                        <SearchableSelect
                            label="Catégorie"
                            value={formData.category}
                            onChange={value => setFormData({ ...formData, category: value })}
                            options={categoryOptions}
                            placeholder="Sélectionner"
                        />
                    </div>

                    <SearchableSelect
                        label="Compte"
                        value={formData.accountId}
                        onChange={value => setFormData({ ...formData, accountId: value })}
                        options={accountOptions}
                        placeholder="Tous les comptes"
                    />

                    <div className="flex gap-3 pt-2">
                        <Button type="button" variant="secondary" fullWidth onClick={() => setIsBudgetModalOpen(false)}>
                            Annuler
                        </Button>
                        <Button type="submit" fullWidth>
                            {editingBudget ? 'Modifier' : 'Créer'}
                        </Button>
                    </div>
                </form>
            </FormPopup>

            <ConfirmModal
                isOpen={!!budgetToDelete}
                onClose={() => setBudgetToDelete(null)}
                onConfirm={handleConfirmDeleteBudget}
                title="Supprimer le budget"
                message="Ce budget sera supprimé et les échéances liées seront simplement déliées."
                confirmLabel="Supprimer"
                isDangerous
            />
        </div>
    );
};

export default Budget;
