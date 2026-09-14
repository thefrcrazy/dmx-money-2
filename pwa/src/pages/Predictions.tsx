import React, { useMemo, useState } from 'react';
import { useBank } from '../context/BankContext';
import { useToast } from '../context/ToastContext';
import { useSettings } from '../context/SettingsContext';
import { format, addMonths, endOfMonth, startOfMonth } from 'date-fns';
import { fr } from 'date-fns/locale';
import { ArrowRightLeft, Edit2, Plus, ShieldAlert, Trash2, TrendingDown, TrendingUp, ChevronDown } from 'lucide-react';
import { v4 as uuidv4 } from 'uuid';
import { ComposedChart, Area, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend, ReferenceLine } from 'recharts';
import Button from '../components/ui/Button';
import FormPopup from '../components/ui/FormPopup';
import Input from '../components/ui/Input';
import SearchableSelect from '../components/ui/SearchableSelect';
import { PredictionFakeTransaction, PredictionTimeRange, TransactionType } from '../types';
import { formatCurrency } from '../utils/format';
import {
    addDayFlow,
    applyDayFlow,
    type CrossingSeverity,
    type DayFlow,
    detectClosingCrossing,
    detectIntradayRisk,
    emptyDayFlow,
    highestSeverity,
} from '../utils/predictions';

interface FakeTransactionFormData {
    date: string;
    description: string;
    amount: string;
    type: TransactionType;
    accountId: string;
    toAccountId: string;
    categoryId: string;
}

interface AlertCrossingMarker {
    date: string;
    fullDate: string;
    labels: string[];
    crossingNames: string[];
    severity: CrossingSeverity | null;
    /**
     * Days where the balance only stays safe because an income lands after the
     * withdrawals. The end-of-day curve looks fine, so without this the risk of
     * an overdraft fee would be invisible.
     */
    intradayNames: string[];
    intradaySeverity: CrossingSeverity | null;
    /** End-of-day balance and same-day low, for the series flagged intraday. */
    intradayBalances: Array<{
        name: string;
        low: number;
        value: number;
    }>;
    balances: Array<{
        name: string;
        color: string;
        value: number;
    }>;
}

const lowKey = (key: string) => `${key}__low`;

const markerStroke = (marker: AlertCrossingMarker) => {
    if (marker.severity === 'danger') return '#ef4444';
    if (marker.severity === 'warning') return '#f97316';
    return marker.intradaySeverity === 'danger' ? '#a855f7' : '#eab308';
};

const PREDICTION_TIME_RANGES: PredictionTimeRange[] = ['week', 'month', '2months', '3months', '6months', '9months', 'year', 'custom'];

const PREDICTION_TIME_RANGE_LABELS: Record<PredictionTimeRange, string> = {
    week: 'Semaine',
    month: 'Mois',
    '2months': '2 Mois',
    '3months': '3 Mois',
    '6months': '6 Mois',
    '9months': '9 Mois',
    year: '1 An',
    custom: 'Personnalisé'
};

const PREDICTION_TITLE_LABELS: Record<PredictionTimeRange, string> = {
    week: '1 semaine',
    month: '1 mois',
    '2months': '2 mois',
    '3months': '3 mois',
    '6months': '6 mois',
    '9months': '9 mois',
    year: '1 an',
    custom: 'personnalisée'
};

const getDefaultCustomEndDate = () => format(addMonths(new Date(), 1), 'yyyy-MM-dd');

const parseLocalDate = (value: string) => {
    const [year, month, day] = value.split('-').map(Number);
    const date = new Date(year, (month || 1) - 1, day || 1);
    date.setHours(0, 0, 0, 0);
    return date;
};

const getProjectionEndDate = (timeRange: PredictionTimeRange, customEndDate: string) => {
    const today = new Date();
    today.setHours(0, 0, 0, 0);

    if (timeRange === 'custom') {
        const [year, month, day] = customEndDate.split('-').map(Number);
        const endDate = new Date(year, (month || 1) - 1, day || 1);
        endDate.setHours(0, 0, 0, 0);
        return endDate < today ? today : endDate;
    }

    if (timeRange === 'week') {
        const endDate = new Date(today);
        endDate.setDate(today.getDate() + 7);
        return endDate;
    }

    const monthsByRange: Record<Exclude<PredictionTimeRange, 'week' | 'custom'>, number> = {
        month: 1,
        '2months': 2,
        '3months': 3,
        '6months': 6,
        '9months': 9,
        year: 12
    };

    return addMonths(today, monthsByRange[timeRange]);
};

const CustomTooltip = ({ active, payload, negativeMarkerByDate, alertThreshold }: any) => {
    if (active && payload && payload.length) {
        const point = payload[0]?.payload;
        const date = point?.date;
        const negativeMarker: AlertCrossingMarker | undefined = date ? negativeMarkerByDate?.get(date) : undefined;
        const highlightedNames = new Set(negativeMarker?.crossingNames || []);
        const intradayNames = new Set(negativeMarker?.intradayNames || []);
        // The low-point series is read through each account's own row, so it must
        // not add a duplicate line of its own.
        const entries = (payload as any[]).filter(entry => !String(entry.dataKey).endsWith('__low'));
        if (entries.length === 0) return null;

        return (
            <div className="app-card p-3 shadow-lg">
                {point?.fullDate && <p className="font-medium text-gray-900 dark:text-gray-200 mb-2">{point.fullDate}</p>}
                <div className="space-y-1">
                    {[...entries].sort((a: any, b: any) => b.value - a.value).map((entry: any, index: number) => {
                        const entryValue = Number(entry.value);
                        const isDangerValue = entryValue < 0;
                        const isWarningValue = !isDangerValue && entryValue < alertThreshold;
                        const isNegativeCrossing = highlightedNames.has(entry.name);
                        const shouldHighlightDanger = isDangerValue || (isNegativeCrossing && negativeMarker?.severity === 'danger');
                        const shouldHighlightWarning = !shouldHighlightDanger && (isWarningValue || isNegativeCrossing);
                        const lowValue = Number(point?.[lowKey(entry.dataKey)] ?? entryValue);
                        const showLow = intradayNames.has(entry.name) && lowValue < entryValue;

                        return (
                            <div
                                key={index}
                                className={`rounded-md px-2 py-1 text-sm font-medium ${
                                    shouldHighlightDanger
                                        ? 'bg-red-500/15 ring-1 ring-red-500/25'
                                        : shouldHighlightWarning
                                            ? 'bg-orange-500/15 ring-1 ring-orange-500/25'
                                            : showLow
                                                ? 'bg-yellow-500/15 ring-1 ring-yellow-500/25'
                                                : ''
                                }`}
                                style={{
                                    color: shouldHighlightDanger
                                        ? '#ef4444'
                                        : shouldHighlightWarning
                                            ? '#f97316'
                                            : entry.color || entry.stroke
                                }}
                            >
                                <div>{entry.name}: {formatCurrency(entryValue)}</div>
                                {showLow && (
                                    <div className={`text-[11px] font-normal ${lowValue < 0 ? 'text-purple-500 dark:text-purple-300' : 'text-yellow-600 dark:text-yellow-300'}`}>
                                        Point bas du jour : {formatCurrency(lowValue)} (retraits avant revenus)
                                    </div>
                                )}
                            </div>
                        );
                    })}
                </div>
                {negativeMarker?.intradaySeverity && (
                    <p className="mt-2 max-w-[15rem] text-[11px] leading-snug text-gray-500 dark:text-gray-400">
                        Le solde de fin de journée reste au-dessus du seuil, mais il passe en dessous avant l’arrivée des revenus.
                    </p>
                )}
            </div>
        );
    }
    return null;
};

const Predictions: React.FC = () => {
    const { accounts: allAccounts, scheduled: allScheduled, transactions: allTransactions, categories, filterAccount } = useBank();
    const { showToast } = useToast();
    const { settings, updateSettings } = useSettings();
    const timeRange = settings.predictionTimeRange || 'year';
    const customEndDate = settings.predictionCustomEndDate || getDefaultCustomEndDate();
    const alertThreshold = settings.predictionAlertThreshold || 0;
    const monthStartsOnFirst = settings.predictionMonthStartsOnFirst ?? true;
    const fakeTransactions = settings.predictionFakeTransactions || [];
    const [isTimeRangeDropdownOpen, setIsTimeRangeDropdownOpen] = useState(false);
    const [showIntradayLow, setShowIntradayLow] = useState(true);
    const [isFakeTransactionModalOpen, setIsFakeTransactionModalOpen] = useState(false);
    const [editingFakeTransaction, setEditingFakeTransaction] = useState<PredictionFakeTransaction | null>(null);
    const [fakeTransactionForm, setFakeTransactionForm] = useState<FakeTransactionFormData>({
        date: format(new Date(), 'yyyy-MM-dd'),
        description: '',
        amount: '',
        type: 'expense',
        accountId: '',
        toAccountId: '',
        categoryId: ''
    });

    const accounts = useMemo(() => filterAccount.length === 0 ? allAccounts : allAccounts.filter(a => filterAccount.includes(a.id)), [allAccounts, filterAccount]);
    const transactions = useMemo(() => filterAccount.length === 0 ? allTransactions : allTransactions.filter(t => filterAccount.includes(t.accountId)), [allTransactions, filterAccount]);
    const scheduled = useMemo(() => filterAccount.length === 0 ? allScheduled : allScheduled.filter(s =>
        filterAccount.includes(s.accountId) || (s.type === 'transfer' && s.toAccountId && filterAccount.includes(s.toAccountId))
    ), [allScheduled, filterAccount]);
    const appliedFakeTransactions = useMemo(() => filterAccount.length === 0 ? fakeTransactions : fakeTransactions.filter(transaction =>
        filterAccount.includes(transaction.accountId) || (transaction.type === 'transfer' && !!transaction.toAccountId && filterAccount.includes(transaction.toAccountId))
    ), [fakeTransactions, filterAccount]);
    const enabledFakeTransactions = useMemo(() => appliedFakeTransactions.filter(transaction => transaction.enabled), [appliedFakeTransactions]);
    const accountMap = useMemo(() => new Map(allAccounts.map(account => [account.id, account])), [allAccounts]);
    const categoryMap = useMemo(() => new Map(categories.map(category => [category.id, category])), [categories]);
    const categoryOptions = useMemo(() => categories
        .filter(category => category.id !== 'transfer')
        .map(category => ({ id: category.id, label: category.name, icon: category.icon, color: category.color }))
    , [categories]);
    const visibleAccountOptions = useMemo(() => accounts.map(account => ({
        id: account.id,
        label: account.name,
        icon: account.icon || 'Wallet',
        color: account.color
    })), [accounts]);
    const allAccountOptions = useMemo(() => allAccounts.map(account => ({
        id: account.id,
        label: account.name,
        icon: account.icon || 'Wallet',
        color: account.color
    })), [allAccounts]);

    const setTimeRange = (predictionTimeRange: PredictionTimeRange) => {
        void updateSettings({ predictionTimeRange });
    };

    const setCustomEndDate = (predictionCustomEndDate: string) => {
        void updateSettings({ predictionCustomEndDate });
    };

    const setAlertThreshold = (predictionAlertThreshold: number) => {
        void updateSettings({ predictionAlertThreshold });
    };

    const setMonthStartsOnFirst = (predictionMonthStartsOnFirst: boolean) => {
        void updateSettings({ predictionMonthStartsOnFirst });
    };

    const setFakeTransactions = (
        updater: PredictionFakeTransaction[] | ((current: PredictionFakeTransaction[]) => PredictionFakeTransaction[])
    ) => {
        const predictionFakeTransactions = typeof updater === 'function'
            ? updater(fakeTransactions)
            : updater;
        void updateSettings({ predictionFakeTransactions });
    };

    // Calculate current total balance
    const currentTotalBalance = useMemo(() => {
        const initialBalanceSum = accounts.reduce((sum, acc) => sum + acc.initialBalance, 0);
        const transactionsSum = transactions.reduce((sum, t) => sum + (t.type === 'income' ? t.amount : -t.amount), 0);
        return initialBalanceSum + transactionsSum;
    }, [accounts, transactions]);

    const todayInputValue = useMemo(() => format(new Date(), 'yyyy-MM-dd'), []);

    const projectionRange = useMemo(() => {
        const today = new Date();
        today.setHours(0, 0, 0, 0);

        const startDate = timeRange !== 'custom' && monthStartsOnFirst ? startOfMonth(today) : new Date(today);
        startDate.setHours(0, 0, 0, 0);

        const endDate = timeRange === 'month' && monthStartsOnFirst ? endOfMonth(today) : getProjectionEndDate(timeRange, customEndDate);
        endDate.setHours(0, 0, 0, 0);

        const millisecondsPerDay = 1000 * 60 * 60 * 24;
        const daysToProject = Math.max(0, Math.round((endDate.getTime() - startDate.getTime()) / millisecondsPerDay));

        return { today, startDate, endDate, daysToProject };
    }, [timeRange, customEndDate, monthStartsOnFirst]);

    const predictionData = useMemo(() => {
        const data = [];
        // Use integers (cents) for calculations to avoid floating point errors
        const currentBalances: Record<string, number> = {};
        // accountId -> date (YYYY-MM-DD) -> the day's outgoing and incoming cents,
        // kept apart so the worst moment of the day can be replayed: banks debit
        // before they credit, and that is where overdraft fees are triggered.
        const dailyImpacts: Record<string, Record<string, DayFlow>> = {};

        const { today, startDate, endDate, daysToProject } = projectionRange;

        const addImpact = (accountId: string, dateStr: string, amountCents: number) => {
            const accountImpacts = dailyImpacts[accountId];
            if (!accountImpacts || amountCents === 0) return;

            addDayFlow(accountImpacts[dateStr] || (accountImpacts[dateStr] = emptyDayFlow()), amountCents);
        };

        // Initialize balances at the beginning of the projection range.
        accounts.forEach(acc => {
            const accountTransactions = transactions.filter(t => (
                t.accountId === acc.id && parseLocalDate(t.date) < startDate
            ));
            const balanceCents = Math.round(acc.initialBalance * 100) + accountTransactions.reduce((sum, t) => sum + Math.round((t.type === 'income' ? t.amount : -t.amount) * 100), 0);
            currentBalances[acc.id] = balanceCents;
            dailyImpacts[acc.id] = {};
        });

        transactions.forEach(transaction => {
            const transactionDate = parseLocalDate(transaction.date);
            if (transactionDate < startDate || transactionDate > endDate) return;

            const dateStr = format(transactionDate, 'yyyy-MM-dd');
            const amount = Math.round((transaction.type === 'income' ? transaction.amount : -transaction.amount) * 100);
            addImpact(transaction.accountId, dateStr, amount);
        });

        enabledFakeTransactions.forEach(transaction => {
            const transactionDate = parseLocalDate(transaction.date);
            if (transactionDate < startDate || transactionDate > endDate) return;

            const dateStr = format(transactionDate, 'yyyy-MM-dd');
            const amountCents = Math.round(transaction.amount * 100);

            if (transaction.type === 'transfer') {
                addImpact(transaction.accountId, dateStr, -amountCents);
                if (transaction.toAccountId) addImpact(transaction.toAccountId, dateStr, amountCents);
                return;
            }

            addImpact(transaction.accountId, dateStr, transaction.type === 'income' ? amountCents : -amountCents);
        });

        // Pre-calculate impacts for all scheduled transactions
        scheduled.forEach(item => {
            // Parse date manually to ensure local time at 00:00:00
            let nextDate = parseLocalDate(item.nextDate);

            const amountCents = Math.round(item.amount * 100);

            // Parse end date if exists
            let itemEndDate: Date | null = null;
            if (item.endDate) {
                itemEndDate = parseLocalDate(item.endDate);
            }

            while (nextDate <= endDate) {
                // Stop if we passed the end date
                if (itemEndDate && nextDate > itemEndDate) break;

                // Only process if date is in the future (or today)
                if (nextDate >= today) {
                    const dateStr = format(nextDate, 'yyyy-MM-dd');

                    // Handle different transaction types
                    if (item.type === 'transfer' && item.toAccountId) {
                        // Transfer: Debit from source, credit to destination
                        addImpact(item.accountId, dateStr, -amountCents);
                        addImpact(item.toAccountId, dateStr, amountCents);
                    } else {
                        // Standard income or expense
                        addImpact(item.accountId, dateStr, item.type === 'income' ? amountCents : -amountCents);
                    }
                }

                // Advance date based on frequency
                const currentDate = new Date(nextDate);
                let newDate: Date;

                switch (item.frequency) {
                    case 'once':
                        // Move past end date to stop loop
                        newDate = new Date(endDate);
                        newDate.setDate(newDate.getDate() + 1);
                        break;
                    case 'daily':
                        newDate = new Date(currentDate.setDate(currentDate.getDate() + 1));
                        break;
                    case 'weekly':
                        newDate = new Date(currentDate.setDate(currentDate.getDate() + 7));
                        break;
                    case 'biweekly':
                        newDate = new Date(currentDate.setDate(currentDate.getDate() + 14));
                        break;
                    case 'bimonthly':
                        newDate = new Date(currentDate.setDate(currentDate.getDate() + 15));
                        break;
                    case 'fourweekly':
                        newDate = new Date(currentDate.setDate(currentDate.getDate() + 28));
                        break;
                    case 'monthly':
                        newDate = addMonths(currentDate, 1);
                        break;
                    case 'bimestrial':
                        newDate = addMonths(currentDate, 2);
                        break;
                    case 'quarterly':
                        newDate = addMonths(currentDate, 3);
                        break;
                    case 'fourmonthly':
                        newDate = addMonths(currentDate, 4);
                        break;
                    case 'semiannual':
                        newDate = addMonths(currentDate, 6);
                        break;
                    case 'annual':
                        newDate = new Date(currentDate.setFullYear(currentDate.getFullYear() + 1));
                        break;
                    case 'biennial':
                        newDate = new Date(currentDate.setFullYear(currentDate.getFullYear() + 2));
                        break;
                    default:
                        newDate = addMonths(currentDate, 1);
                }
                nextDate = newDate;
                nextDate.setHours(0, 0, 0, 0);
            }
        });

        // Generate points for the graph
        for (let i = 0; i <= daysToProject; i++) {
            const targetDate = new Date(startDate);
            targetDate.setDate(startDate.getDate() + i);
            const dateStr = format(targetDate, 'yyyy-MM-dd');

            const dayData: any = {
                date: targetDate.toISOString(),
                displayDate: format(targetDate, 'd MMM', { locale: fr }),
                fullDate: format(targetDate, 'd MMMM yyyy', { locale: fr }),
                total: 0
            };

            let totalCents = 0;
            let totalLowCents = 0;

            // Apply impacts and calculate totals
            accounts.forEach(acc => {
                const { low, close } = applyDayFlow(currentBalances[acc.id], dailyImpacts[acc.id]?.[dateStr]);
                currentBalances[acc.id] = close;

                // Convert back to float for display
                dayData[acc.id] = close / 100;
                dayData[lowKey(acc.id)] = low / 100;
                totalCents += close;
                totalLowCents += low;
            });

            // Convert totals to float
            dayData.total = totalCents / 100;
            dayData[lowKey('total')] = totalLowCents / 100;

            data.push(dayData);
        }
        return data;
    }, [accounts, transactions, enabledFakeTransactions, scheduled, projectionRange]);

    const midpointPrediction = predictionData[Math.floor((predictionData.length - 1) / 2)]?.total ?? currentTotalBalance;
    const finalPrediction = predictionData[predictionData.length - 1]?.total ?? currentTotalBalance;
    const fakeTransactionsImpact = useMemo(() => {
        const visibleAccountIds = new Set(accounts.map(account => account.id));

        return enabledFakeTransactions.reduce((sum, transaction) => {
            const transactionDate = parseLocalDate(transaction.date);
            if (transactionDate < projectionRange.startDate || transactionDate > projectionRange.endDate) return sum;

            if (transaction.type === 'transfer') {
                const sourceImpact = visibleAccountIds.has(transaction.accountId) ? -transaction.amount : 0;
                const destinationImpact = transaction.toAccountId && visibleAccountIds.has(transaction.toAccountId) ? transaction.amount : 0;
                return sum + sourceImpact + destinationImpact;
            }

            if (!visibleAccountIds.has(transaction.accountId)) return sum;
            return sum + (transaction.type === 'income' ? transaction.amount : -transaction.amount);
        }, 0);
    }, [accounts, enabledFakeTransactions, projectionRange.endDate, projectionRange.startDate]);

    const sortedAppliedFakeTransactions = useMemo(() => [...appliedFakeTransactions].sort((a, b) => {
        const dateDiff = parseLocalDate(a.date).getTime() - parseLocalDate(b.date).getTime();
        if (dateDiff !== 0) return dateDiff;
        return a.description.localeCompare(b.description);
    }), [appliedFakeTransactions]);

    const negativeCrossingMarkers = useMemo(() => {
        const watchedKeys = ['total', ...accounts.map(account => account.id)];
        const markers = new Map<string, AlertCrossingMarker>();

        predictionData.forEach((point, index) => {
            const crossings: Array<{ name: string; severity: CrossingSeverity }> = [];
            const intradayCrossings: Array<{
                name: string;
                severity: CrossingSeverity;
                low: number;
                value: number;
            }> = [];

            watchedKeys.forEach(key => {
                const value = Number(point[key] ?? 0);
                const low = Number(point[lowKey(key)] ?? value);
                const previousValue = index === 0
                    ? 0
                    : Number(predictionData[index - 1]?.[key] ?? 0);
                const account = accounts.find(item => item.id === key);
                const name = account?.name || 'Total';

                const closing = detectClosingCrossing(value, previousValue, alertThreshold);
                if (closing) crossings.push({ name, severity: closing });

                // Both can hold on the same day: the balance can end under the
                // safety threshold *and* have dipped below zero earlier, before the
                // income landed. Flagging only the first would hide the fee risk.
                const intraday = detectIntradayRisk(low, value, alertThreshold);
                if (intraday) intradayCrossings.push({ name, severity: intraday, low, value });
            });

            if (crossings.length === 0 && intradayCrossings.length === 0) return;

            const crossingLabels = crossings.map(crossing => crossing.name);
            const intradayLabels = intradayCrossings.map(crossing => crossing.name);

            markers.set(point.date, {
                date: point.date,
                fullDate: point.fullDate,
                labels: [...crossingLabels, ...intradayLabels],
                crossingNames: crossingLabels,
                severity: highestSeverity(crossings),
                intradayNames: intradayLabels,
                intradaySeverity: highestSeverity(intradayCrossings),
                intradayBalances: intradayCrossings.map(({ name, low, value }) => ({ name, low, value })),
                balances: accounts.length > 0
                    ? accounts.map(account => ({
                        name: account.name,
                        color: account.color || '#3b82f6',
                        value: Number(point[account.id] ?? 0)
                    }))
                    : [{ name: 'Total', color: '#ef4444', value: Number(point.total ?? 0) }]
            });
        });

        return Array.from(markers.values());
    }, [accounts, alertThreshold, predictionData]);

    /** Days that only look safe on the end-of-day curve. */
    const intradayRiskDays = useMemo(
        () => negativeCrossingMarkers.filter(marker => marker.intradaySeverity !== null),
        [negativeCrossingMarkers],
    );

    const negativeMarkerByDate = useMemo(() => new Map(negativeCrossingMarkers.map(marker => [marker.date, marker])), [negativeCrossingMarkers]);

    // Keep the projection axis readable while still showing day labels.
    const formatXAxis = (tickItem: string) => {
        return format(new Date(tickItem), 'dd MMM', { locale: fr });
    };

    const resetFakeTransactionForm = () => {
        const defaultAccountId = accounts[0]?.id || allAccounts[0]?.id || '';
        const defaultToAccountId = allAccounts.find(account => account.id !== defaultAccountId)?.id || '';

        setFakeTransactionForm({
            date: todayInputValue,
            description: '',
            amount: '',
            type: 'expense',
            accountId: defaultAccountId,
            toAccountId: defaultToAccountId,
            categoryId: categoryOptions[0]?.id || ''
        });
    };

    const openFakeTransactionModal = () => {
        setEditingFakeTransaction(null);
        resetFakeTransactionForm();
        setIsFakeTransactionModalOpen(true);
    };

    const openEditFakeTransactionModal = (transaction: PredictionFakeTransaction) => {
        setEditingFakeTransaction(transaction);
        setFakeTransactionForm({
            date: transaction.date,
            description: transaction.description,
            amount: String(transaction.amount),
            type: transaction.type,
            accountId: transaction.accountId,
            toAccountId: transaction.toAccountId || '',
            categoryId: transaction.type === 'transfer' ? 'transfer' : transaction.category
        });
        setIsFakeTransactionModalOpen(true);
    };

    const closeFakeTransactionModal = () => {
        setIsFakeTransactionModalOpen(false);
        setEditingFakeTransaction(null);
    };

    const handleFakeTransactionTypeChange = (type: string) => {
        const nextType = type as TransactionType;
        setFakeTransactionForm(prev => {
            const nextAccountId = prev.accountId || accounts[0]?.id || allAccounts[0]?.id || '';
            const nextToAccountId = prev.toAccountId && prev.toAccountId !== nextAccountId
                ? prev.toAccountId
                : allAccounts.find(account => account.id !== nextAccountId)?.id || '';

            return {
                ...prev,
                type: nextType,
                accountId: nextAccountId,
                toAccountId: nextType === 'transfer' ? nextToAccountId : prev.toAccountId,
                categoryId: nextType === 'transfer' ? 'transfer' : (prev.categoryId === 'transfer' ? categoryOptions[0]?.id || '' : prev.categoryId)
            };
        });
    };

    const handleFakeTransactionAccountChange = (accountId: string) => {
        setFakeTransactionForm(prev => ({
            ...prev,
            accountId,
            toAccountId: prev.type === 'transfer' && prev.toAccountId === accountId
                ? allAccounts.find(account => account.id !== accountId)?.id || ''
                : prev.toAccountId
        }));
    };

    const handleSubmitFakeTransaction = (event: React.FormEvent) => {
        event.preventDefault();

        const amount = Number(fakeTransactionForm.amount);
        if (!/^\d{4}-\d{2}-\d{2}$/.test(fakeTransactionForm.date)) {
            showToast("Sélectionnez une date valide", "error");
            return;
        }

        const selectedDate = parseLocalDate(fakeTransactionForm.date);
        const today = parseLocalDate(todayInputValue);
        if (selectedDate < today) {
            showToast("La date doit être aujourd'hui ou dans le futur", "error");
            return;
        }

        if (!fakeTransactionForm.accountId) {
            showToast("Sélectionnez un compte", "error");
            return;
        }

        if (!Number.isFinite(amount) || amount <= 0) {
            showToast("Saisissez un montant valide", "error");
            return;
        }

        if (fakeTransactionForm.type !== 'transfer' && !fakeTransactionForm.categoryId) {
            showToast("Sélectionnez une catégorie", "error");
            return;
        }

        if (fakeTransactionForm.type === 'transfer' && (!fakeTransactionForm.toAccountId || fakeTransactionForm.toAccountId === fakeTransactionForm.accountId)) {
            showToast("Sélectionnez un compte destination différent", "error");
            return;
        }

        const nextTransaction: PredictionFakeTransaction = {
            id: editingFakeTransaction?.id || uuidv4(),
            date: fakeTransactionForm.date,
            accountId: fakeTransactionForm.accountId,
            type: fakeTransactionForm.type,
            amount,
            category: fakeTransactionForm.type === 'transfer' ? 'transfer' : fakeTransactionForm.categoryId,
            description: fakeTransactionForm.description.trim() || 'Transaction fictive',
            enabled: editingFakeTransaction?.enabled ?? true,
            toAccountId: fakeTransactionForm.type === 'transfer' ? fakeTransactionForm.toAccountId : undefined
        };

        setFakeTransactions(prev => editingFakeTransaction
            ? prev.map(transaction => transaction.id === editingFakeTransaction.id ? nextTransaction : transaction)
            : [...prev, nextTransaction]
        );
        closeFakeTransactionModal();
        showToast(editingFakeTransaction ? "Transaction fictive mise à jour" : "Transaction fictive ajoutée", "success");
    };

    const toggleFakeTransactionEnabled = (id: string) => {
        setFakeTransactions(prev => prev.map(transaction => transaction.id === id
            ? { ...transaction, enabled: !transaction.enabled }
            : transaction
        ));
    };

    const removeFakeTransaction = (id: string) => {
        setFakeTransactions(prev => prev.filter(transaction => transaction.id !== id));
        showToast("Transaction fictive retirée", "success");
    };

    const clearAppliedFakeTransactions = () => {
        const appliedIds = new Set(appliedFakeTransactions.map(transaction => transaction.id));
        setFakeTransactions(prev => prev.filter(transaction => !appliedIds.has(transaction.id)));
        showToast("Transactions fictives retirées", "success");
    };

    const getFakeTransactionTypeMeta = (transaction: PredictionFakeTransaction) => {
        if (transaction.type === 'income') {
            return {
                label: 'Revenu',
                icon: TrendingUp,
                amountPrefix: '+',
                amountClassName: 'text-emerald-600 dark:text-emerald-400',
                iconClassName: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
            };
        }

        if (transaction.type === 'transfer') {
            return {
                label: 'Virement',
                icon: ArrowRightLeft,
                amountPrefix: '',
                amountClassName: 'text-blue-600 dark:text-blue-400',
                iconClassName: 'bg-blue-500/10 text-blue-600 dark:text-blue-400'
            };
        }

        return {
            label: 'Dépense',
            icon: TrendingDown,
            amountPrefix: '-',
            amountClassName: 'text-red-600 dark:text-red-400',
            iconClassName: 'bg-red-500/10 text-red-600 dark:text-red-400'
        };
    };

    return (
        <div className="space-y-6">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4">
                <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-200">Prédictions Financières</h2>

                <div className="hidden md:flex flex-wrap gap-2 period-selector justify-end w-full md:w-auto">
                    {PREDICTION_TIME_RANGES.map((range) => (
                        <Button
                            key={range}
                            onClick={() => setTimeRange(range)}
                            variant="ghost"
                            size="sm"
                            className={`transition-colors flex-1 sm:flex-initial text-center ${timeRange === range
                                ? 'bg-primary-100 text-primary-700 hover:bg-primary-200 dark:bg-primary-900/20 dark:text-primary-400 dark:hover:bg-primary-900/30'
                                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700'
                                }`}
                        >
                            {PREDICTION_TIME_RANGE_LABELS[range]}
                        </Button>
                    ))}
                </div>

                <div className="relative md:hidden w-full z-30">
                    <button
                        onClick={() => setIsTimeRangeDropdownOpen(!isTimeRangeDropdownOpen)}
                        className="w-full flex items-center justify-between px-4 py-3 rounded-2xl border border-black/[0.06] dark:border-white/[0.08] bg-white/80 dark:bg-neutral-900/60 text-sm font-semibold text-gray-800 dark:text-gray-200 shadow-sm active:scale-[0.98] transition-all"
                    >
                        <span>{PREDICTION_TIME_RANGE_LABELS[timeRange]}</span>
                        <ChevronDown className={`w-4 h-4 opacity-60 transition-transform duration-200 ${isTimeRangeDropdownOpen ? 'rotate-180' : ''}`} />
                    </button>
                    
                    {isTimeRangeDropdownOpen && (
                        <>
                            <div 
                                className="fixed inset-0 z-40 bg-transparent" 
                                onClick={() => setIsTimeRangeDropdownOpen(false)}
                            />
                            <div className="absolute left-0 right-0 w-full mt-1.5 z-50 rounded-2xl border border-black/[0.08] dark:border-white/[0.12] bg-white dark:bg-neutral-900 shadow-xl p-1.5 flex flex-col gap-0.5 animate-in fade-in-50 slide-in-from-top-2 duration-150">
                                {PREDICTION_TIME_RANGES.map(range => (
                                    <button
                                        key={range}
                                        onClick={() => {
                                            setTimeRange(range);
                                            setIsTimeRangeDropdownOpen(false);
                                        }}
                                        className={`w-full text-left px-3.5 py-2.5 rounded-xl text-sm font-medium transition-colors flex items-center justify-between ${
                                            timeRange === range
                                                ? 'bg-primary-500/10 text-primary-600 dark:text-primary-400'
                                                : 'text-gray-700 dark:text-gray-300 active:bg-black/5 dark:active:bg-white/5'
                                        }`}
                                    >
                                        <span>{PREDICTION_TIME_RANGE_LABELS[range]}</span>
                                        {timeRange === range && (
                                            <span className="w-1.5 h-1.5 rounded-full bg-primary-500" />
                                        )}
                                    </button>
                                ))}
                            </div>
                        </>
                    )}
                </div>
            </div>

            <div className={`transition-all duration-300 ease-in-out overflow-hidden ${timeRange === 'custom' ? 'max-h-48 opacity-100 pointer-events-auto' : 'max-h-0 opacity-0 pointer-events-none'}`}>
                <div className="flex gap-4 app-card p-4 w-fit ml-auto shadow-none">
                    <div>
                        <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Jusqu'au</label>
                        <input
                            type="date"
                            value={customEndDate}
                            min={todayInputValue}
                            onChange={(e) => setCustomEndDate(e.target.value)}
                            className="bg-transparent border border-gray-200 dark:border-neutral-700 rounded-lg px-3 py-2 text-sm text-gray-900 dark:text-gray-200 focus:ring-2 focus:ring-primary-500/20 focus:border-primary-500 outline-none"
                        />
                    </div>
                </div>
            </div>

            <div className="app-card p-6">
                <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">Transactions fictives</h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400">
                            Elles modifient uniquement cette projection et ne sont pas ajoutées au journal.
                        </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                        {appliedFakeTransactions.length > 0 && (
                            <Button variant="secondary" size="sm" icon={Trash2} onClick={clearAppliedFakeTransactions}>
                                Tout retirer
                            </Button>
                        )}
                        <Button size="sm" icon={Plus} onClick={openFakeTransactionModal} disabled={accounts.length === 0}>
                            Ajouter
                        </Button>
                    </div>
                </div>

                {appliedFakeTransactions.length > 0 ? (
                    <div className="mt-4">
                        <div className="mb-3 flex flex-col gap-1 text-sm text-gray-600 dark:text-gray-400 sm:flex-row sm:items-center sm:justify-between">
                            <span>
                                {enabledFakeTransactions.length}/{appliedFakeTransactions.length} simulation{appliedFakeTransactions.length > 1 ? 's' : ''} active{enabledFakeTransactions.length === 1 ? '' : 's'}
                            </span>
                            <span className={`font-semibold tabular-nums ${fakeTransactionsImpact >= 0 ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'}`}>
                                Impact période : {fakeTransactionsImpact >= 0 ? '+' : ''}{formatCurrency(fakeTransactionsImpact)}
                            </span>
                        </div>

                        <div className="overflow-hidden rounded-lg border border-gray-100 dark:border-neutral-800">
                            <div className="divide-y divide-gray-100 dark:divide-neutral-800">
                                {sortedAppliedFakeTransactions.map(transaction => {
                                    const sourceAccount = accountMap.get(transaction.accountId);
                                    const destinationAccount = transaction.toAccountId ? accountMap.get(transaction.toAccountId) : undefined;
                                    const category = categoryMap.get(transaction.category);
                                    const meta = getFakeTransactionTypeMeta(transaction);
                                    const Icon = meta.icon;

                                    return (
                                        <div key={transaction.id} className={`flex flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between ${transaction.enabled ? '' : 'bg-gray-50/70 opacity-70 dark:bg-neutral-900/50'}`}>
                                            <div className="flex min-w-0 items-center gap-3">
                                                <input
                                                    type="checkbox"
                                                    checked={transaction.enabled}
                                                    onChange={() => toggleFakeTransactionEnabled(transaction.id)}
                                                    className="h-4 w-4 flex-none rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-neutral-600"
                                                    aria-label={transaction.enabled ? "Désactiver cette transaction fictive" : "Activer cette transaction fictive"}
                                                    title={transaction.enabled ? "Désactiver cette transaction fictive" : "Activer cette transaction fictive"}
                                                />
                                                <div className={`flex h-9 w-9 flex-none items-center justify-center rounded-lg ${meta.iconClassName}`}>
                                                    <Icon className="h-4 w-4" />
                                                </div>
                                                <div className="min-w-0">
                                                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                                                        <span className="truncate text-sm font-semibold text-gray-900 dark:text-gray-200">
                                                            {transaction.description}
                                                        </span>
                                                        <span className="rounded-md bg-gray-100 px-2 py-0.5 text-[11px] font-medium text-gray-600 dark:bg-neutral-800 dark:text-gray-300">
                                                            {meta.label}
                                                        </span>
                                                        {!transaction.enabled && (
                                                            <span className="rounded-md bg-gray-200 px-2 py-0.5 text-[11px] font-medium text-gray-500 dark:bg-neutral-700 dark:text-gray-400">
                                                                Désactivée
                                                            </span>
                                                        )}
                                                    </div>
                                                    <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-gray-500 dark:text-gray-400">
                                                        <span>{format(parseLocalDate(transaction.date), 'dd MMM yyyy', { locale: fr })}</span>
                                                        <span>•</span>
                                                        <span>{sourceAccount?.name || 'Compte supprimé'}</span>
                                                        {transaction.type === 'transfer' && (
                                                            <>
                                                                <span>→</span>
                                                                <span>{destinationAccount?.name || 'Compte supprimé'}</span>
                                                            </>
                                                        )}
                                                        {transaction.type !== 'transfer' && category && (
                                                            <>
                                                                <span>•</span>
                                                                <span>{category.name}</span>
                                                            </>
                                                        )}
                                                    </div>
                                                </div>
                                            </div>

                                            <div className="flex items-center justify-between gap-3 sm:justify-end">
                                                <span className={`text-sm font-semibold tabular-nums ${transaction.enabled ? meta.amountClassName : 'text-gray-400 dark:text-gray-500'}`}>
                                                    {meta.amountPrefix}{formatCurrency(transaction.amount)}
                                                </span>
                                                <div className="flex items-center gap-1">
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        icon={Edit2}
                                                        onClick={() => openEditFakeTransactionModal(transaction)}
                                                        className="h-8 w-8"
                                                        title="Modifier cette transaction fictive"
                                                    />
                                                    <Button
                                                        variant="ghost"
                                                        size="icon"
                                                        icon={Trash2}
                                                        onClick={() => removeFakeTransaction(transaction.id)}
                                                        className="h-8 w-8 text-red-500 hover:text-red-600"
                                                        title="Retirer cette transaction fictive"
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="mt-4 rounded-lg border border-dashed border-gray-200 px-4 py-6 text-center text-sm text-gray-500 dark:border-neutral-800 dark:text-gray-400">
                        Aucune transaction fictive appliquée à cette projection.
                    </div>
                )}
            </div>

            <div className="app-card p-6">
                <div className="mb-6 flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">
                            Projection sur {PREDICTION_TITLE_LABELS[timeRange]} (Journalière)
                        </h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400">Visualisation de la trésorerie jour par jour.</p>
                    </div>
                    <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
                        {timeRange !== 'custom' && (
                            <label className="flex w-fit items-center gap-3 rounded-lg border border-gray-200 dark:border-neutral-700 px-3 py-2 text-sm text-gray-700 dark:text-gray-300">
                                <input
                                    type="checkbox"
                                    checked={monthStartsOnFirst}
                                    onChange={(event) => setMonthStartsOnFirst(event.target.checked)}
                                    className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-neutral-600"
                                />
                                Démarrer au 1er du mois
                            </label>
                        )}
                        <label
                            className="flex w-fit items-center gap-3 rounded-lg border border-gray-200 dark:border-neutral-700 px-3 py-2 text-sm text-gray-700 dark:text-gray-300"
                            title="Trace le solde de chaque compte une fois les retraits du jour passés, avant l’arrivée des revenus."
                        >
                            <input
                                type="checkbox"
                                checked={showIntradayLow}
                                onChange={(event) => setShowIntradayLow(event.target.checked)}
                                className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500 dark:border-neutral-600"
                            />
                            Point bas journalier
                        </label>
                    </div>
                </div>

                <div className="h-96 min-w-0" style={{ minHeight: '384px' }}>
                    <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                        <ComposedChart data={predictionData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                            <defs>
                                {accounts.map(acc => (
                                    <linearGradient key={acc.id} id={`color-${acc.id}`} x1="0" y1="0" x2="0" y2="1">
                                        <stop offset="5%" stopColor={acc.color || '#3b82f6'} stopOpacity={0.8} />
                                        <stop offset="95%" stopColor={acc.color || '#3b82f6'} stopOpacity={0} />
                                    </linearGradient>
                                ))}
                            </defs>
                            <CartesianGrid strokeDasharray="3 3" vertical={false} />
                            <XAxis
                                dataKey="date"
                                tickFormatter={formatXAxis}
                                minTickGap={30}
                            />
                            <YAxis />
                            <Tooltip content={<CustomTooltip negativeMarkerByDate={negativeMarkerByDate} alertThreshold={alertThreshold} />} />
                            <Legend />
                            <ReferenceLine y={0} stroke="#ef4444" strokeWidth={1} ifOverflow="extendDomain" />
                            {alertThreshold > 0 && (
                                <ReferenceLine
                                    y={alertThreshold}
                                    stroke="#f97316"
                                    strokeWidth={1}
                                    strokeDasharray="6 4"
                                    ifOverflow="extendDomain"
                                />
                            )}
                            {negativeCrossingMarkers.map(marker => (
                                <ReferenceLine
                                    key={marker.date}
                                    x={marker.date}
                                    stroke={markerStroke(marker)}
                                    strokeWidth={2}
                                    strokeDasharray="3 3"
                                    ifOverflow="extendDomain"
                                />
                            ))}
                            {accounts.map(acc => (
                                <Area
                                    key={acc.id}
                                    type="stepAfter" // Use stepAfter for clearer cash flow jumps
                                    dataKey={acc.id}
                                    name={acc.name}
                                    stroke={acc.color || '#3b82f6'}
                                    fill={`url(#color-${acc.id})`}
                                    fillOpacity={1}
                                />
                            ))}
                            {showIntradayLow && accounts.map(acc => (
                                <Line
                                    key={lowKey(acc.id)}
                                    type="stepAfter"
                                    dataKey={lowKey(acc.id)}
                                    name={`${acc.name} — point bas`}
                                    stroke={acc.color || '#3b82f6'}
                                    strokeWidth={1.5}
                                    strokeDasharray="4 3"
                                    strokeOpacity={0.85}
                                    dot={false}
                                    activeDot={false}
                                    legendType="none"
                                    tooltipType="none"
                                />
                            ))}
                        </ComposedChart>
                    </ResponsiveContainer>
                </div>

                <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-gray-500 dark:text-gray-400">
                    {[
                        { color: '#ef4444', label: 'Solde négatif en fin de journée' },
                        { color: '#f97316', label: 'Passage sous le seuil d’alerte' },
                        { color: '#a855f7', label: 'Point bas négatif, rattrapé par un revenu' },
                        { color: '#eab308', label: 'Point bas sous le seuil, rattrapé par un revenu' }
                    ].map(item => (
                        <span key={item.label} className="flex items-center gap-1.5">
                            <span className="h-0.5 w-4 rounded-full" style={{ backgroundColor: item.color }} />
                            {item.label}
                        </span>
                    ))}
                </div>
            </div>

            {intradayRiskDays.length > 0 && (
                <div className="app-card p-6">
                    <div className="flex items-start gap-3">
                        <div className="flex h-9 w-9 flex-none items-center justify-center rounded-lg bg-yellow-500/10 text-yellow-600 dark:text-yellow-400">
                            <ShieldAlert className="h-4 w-4" />
                        </div>
                        <div className="min-w-0 flex-1">
                            <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">
                                Jours à surveiller ({intradayRiskDays.length})
                            </h3>
                            <p className="text-sm text-gray-500 dark:text-gray-400">
                                Ces jours cumulent un retrait et un revenu. Le solde de fin de journée reste au-dessus du seuil,
                                mais il passe en dessous avant l’arrivée du revenu — de quoi déclencher des frais côté banque.
                            </p>

                            <div className="mt-4 overflow-hidden rounded-lg border border-gray-100 dark:border-neutral-800">
                                <div className="divide-y divide-gray-100 dark:divide-neutral-800">
                                    {intradayRiskDays.slice(0, 8).map(marker => {
                                        const isBelowZero = marker.intradaySeverity === 'danger';

                                        return (
                                            <div key={marker.date} className="flex flex-col gap-2 px-4 py-3">
                                                <p className="text-sm font-semibold text-gray-900 dark:text-gray-200">{marker.fullDate}</p>
                                                {marker.intradayBalances.map(balance => (
                                                    <div key={balance.name} className="flex flex-col gap-x-3 gap-y-0.5 text-xs sm:flex-row sm:items-center sm:justify-between">
                                                        <span className="text-gray-500 dark:text-gray-400">{balance.name}</span>
                                                        <span className="flex flex-wrap items-center gap-x-2">
                                                            <span className={`font-semibold tabular-nums ${isBelowZero ? 'text-purple-600 dark:text-purple-400' : 'text-yellow-600 dark:text-yellow-400'}`}>
                                                                Point bas {formatCurrency(balance.low)}
                                                            </span>
                                                            <span className="text-gray-400">→</span>
                                                            <span className="font-semibold tabular-nums text-emerald-600 dark:text-emerald-400">
                                                                Fin de journée {formatCurrency(balance.value)}
                                                            </span>
                                                        </span>
                                                    </div>
                                                ))}
                                            </div>
                                        );
                                    })}
                                </div>
                            </div>
                            {intradayRiskDays.length > 8 && (
                                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                                    +{intradayRiskDays.length - 8} autre{intradayRiskDays.length - 8 > 1 ? 's' : ''} jour{intradayRiskDays.length - 8 > 1 ? 's' : ''} sur la période.
                                </p>
                            )}
                        </div>
                    </div>
                </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <div className="app-card p-6">
                    <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-2">Solde Actuel Total</h3>
                    <div className="text-2xl font-bold text-gray-900 dark:text-gray-200">
                        {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(currentTotalBalance)}
                    </div>
                </div>
                <div className="app-card p-6">
                    <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-2">Projection mi-période</h3>
                    <div className={`text-2xl font-bold ${midpointPrediction >= currentTotalBalance ? 'text-emerald-600' : 'text-red-600'}`}>
                        {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(midpointPrediction)}
                    </div>
                </div>
                <div className="app-card p-6">
                    <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-2">
                        Projection au {format(projectionRange.endDate, 'dd MMM yyyy', { locale: fr })}
                    </h3>
                    <div className={`text-2xl font-bold ${finalPrediction >= currentTotalBalance ? 'text-emerald-600' : 'text-red-600'}`}>
                        {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(finalPrediction)}
                    </div>
                </div>
            </div>

            <div className="app-card p-6">
                <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
                    <div>
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">Seuil d'alerte</h3>
                        <p className="text-sm text-gray-500 dark:text-gray-400">
                            Le seuil personnalisé s'affiche en orange. Le rouge reste réservé aux soldes négatifs.
                            Les jours où un retrait passe avant un revenu sont signalés à part, même si la journée se termine au-dessus du seuil.
                        </p>
                    </div>
                    <div className="w-full md:w-56">
                        <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Montant</label>
                        <div className="relative">
                            <input
                                type="number"
                                step="1"
                                value={alertThreshold}
                                onChange={(event) => {
                                    const nextValue = Number(event.target.value);
                                    setAlertThreshold(Number.isFinite(nextValue) ? nextValue : 0);
                                }}
                                className="w-full bg-transparent border border-gray-200 dark:border-neutral-700 rounded-lg px-3 py-2 pr-8 text-sm text-gray-900 dark:text-gray-200 focus:ring-2 focus:ring-primary-500/20 focus:border-primary-500 outline-none"
                            />
                            <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-gray-400">€</span>
                        </div>
                    </div>
                </div>
            </div>

            <FormPopup
                isOpen={isFakeTransactionModalOpen}
                onClose={closeFakeTransactionModal}
                title={editingFakeTransaction ? "Modifier la transaction fictive" : "Nouvelle transaction fictive"}
                onSubmit={handleSubmitFakeTransaction}
                submitLabel={editingFakeTransaction ? "Enregistrer" : "Ajouter"}
                maxWidth="xl"
            >
                <div className="space-y-4">
                    <Input
                        label="Description"
                        value={fakeTransactionForm.description}
                        onChange={(event) => setFakeTransactionForm(prev => ({ ...prev, description: event.target.value }))}
                        placeholder="Ex: Réparation voiture"
                    />

                    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        <Input
                            label="Montant"
                            type="number"
                            step="0.01"
                            min="0"
                            required
                            value={fakeTransactionForm.amount}
                            onChange={(event) => setFakeTransactionForm(prev => ({ ...prev, amount: event.target.value }))}
                            rightElement="€"
                            placeholder="0.00"
                        />
                        <SearchableSelect
                            label="Type"
                            value={fakeTransactionForm.type}
                            onChange={handleFakeTransactionTypeChange}
                            options={[
                                { id: 'expense', label: 'Dépense', icon: 'TrendingDown', color: '#ef4444' },
                                { id: 'income', label: 'Revenu', icon: 'TrendingUp', color: '#10b981' },
                                { id: 'transfer', label: 'Virement', icon: 'ArrowRightLeft', color: '#6366f1' }
                            ]}
                        />
                    </div>

                    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                        <SearchableSelect
                            label={fakeTransactionForm.type === 'transfer' ? 'Compte source' : 'Compte'}
                            value={fakeTransactionForm.accountId}
                            onChange={handleFakeTransactionAccountChange}
                            options={fakeTransactionForm.type === 'transfer' ? allAccountOptions : visibleAccountOptions}
                            placeholder="Sélectionner un compte"
                        />
                        <Input
                            label="Date"
                            type="date"
                            min={todayInputValue}
                            required
                            value={fakeTransactionForm.date}
                            onChange={(event) => setFakeTransactionForm(prev => ({ ...prev, date: event.target.value }))}
                        />
                    </div>

                    {fakeTransactionForm.type === 'transfer' ? (
                        <SearchableSelect
                            label="Compte destination"
                            value={fakeTransactionForm.toAccountId}
                            onChange={(value) => setFakeTransactionForm(prev => ({ ...prev, toAccountId: value }))}
                            options={allAccountOptions.filter(account => account.id !== fakeTransactionForm.accountId)}
                            placeholder="Sélectionner un compte"
                        />
                    ) : (
                        <SearchableSelect
                            label="Catégorie"
                            value={fakeTransactionForm.categoryId}
                            onChange={(value) => setFakeTransactionForm(prev => ({ ...prev, categoryId: value }))}
                            options={categoryOptions}
                            placeholder="Sélectionner une catégorie"
                        />
                    )}
                </div>
            </FormPopup>
        </div>
    );
};

export default Predictions;
