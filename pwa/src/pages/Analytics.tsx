import React, { useMemo, useState } from 'react';
import Button from '../components/ui/Button';
import CategoryPieChart from '../features/analytics/CategoryPieChart';
import { AreaChart, Area, ResponsiveContainer, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import { useBank } from '../context/BankContext';
import { useSettings } from '../context/SettingsContext';
import { format, subMonths, eachMonthOfInterval, subWeeks, subYears, isWithinInterval, startOfMonth } from 'date-fns';
import { fr } from 'date-fns/locale';
import { ChevronDown } from 'lucide-react';
import { AnalyticsTimeRange } from '../types';

const COLORS = ['#0088FE', '#00C49F', '#FFBB28', '#FF8042', '#8884d8', '#82ca9d', '#ffc658', '#ff7300'];

const ANALYTICS_TIME_RANGES: AnalyticsTimeRange[] = ['week', 'month', '2months', '3months', '6months', '9months', 'year', 'custom'];

const ANALYTICS_TIME_RANGE_LABELS: Record<AnalyticsTimeRange, string> = {
    week: 'Semaine',
    month: 'Mois',
    '2months': '2 Mois',
    '3months': '3 Mois',
    '6months': '6 Mois',
    '9months': '9 Mois',
    year: '1 An',
    custom: 'Personnalisé'
};

const getDefaultCustomStartDate = () => format(subMonths(new Date(), 1), 'yyyy-MM-dd');
const getDefaultCustomEndDate = () => format(new Date(), 'yyyy-MM-dd');

const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
        return (
            <div className="app-card p-4 shadow-xl" style={{
                borderRadius: '12px',
                boxShadow: '0 4px 20px rgba(0, 0, 0, 0.15)'
            }}>
                <p className="font-semibold text-gray-900 dark:text-gray-200 mb-3 text-sm">{label}</p>
                {[...payload].sort((a: any, b: any) => b.value - a.value).map((entry: any, index: number) => (
                    <div key={index} className="flex items-center justify-between gap-4 text-sm py-0.5">
                        <div className="flex items-center gap-2">
                            <div className="w-2.5 h-2.5 rounded-full" style={{ backgroundColor: entry.color || entry.fill }} />
                            <span className="text-gray-600 dark:text-gray-400">{entry.name}</span>
                        </div>
                        <span className="font-medium" style={{ color: entry.color || entry.fill }}>
                            {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(entry.value)}
                        </span>
                    </div>
                ))}
            </div>
        );
    }
    return null;
};

const Analytics: React.FC = () => {
    const { transactions: allTransactions, accounts: allAccounts, categories, filterAccount } = useBank();
    const { settings, updateSettings } = useSettings();
    const timeRange = settings.analyticsTimeRange || 'year';
    const customStartDate = settings.analyticsCustomStartDate || getDefaultCustomStartDate();
    const customEndDate = settings.analyticsCustomEndDate || getDefaultCustomEndDate();
    const monthStartsOnFirst = settings.analyticsMonthStartsOnFirst ?? true;
    const hiddenExpenseCategories = settings.analyticsHiddenExpenseCategories || [];
    const hiddenIncomeCategories = settings.analyticsHiddenIncomeCategories || [];

    const transactions = useMemo(() =>
        filterAccount.length === 0 ? allTransactions : allTransactions.filter(t => filterAccount.includes(t.accountId)),
        [allTransactions, filterAccount]);

    const accounts = useMemo(() =>
        filterAccount.length === 0 ? allAccounts : allAccounts.filter(a => filterAccount.includes(a.id)),
        [allAccounts, filterAccount]);

    const [isTimeRangeDropdownOpen, setIsTimeRangeDropdownOpen] = useState(false);

    const setTimeRange = (analyticsTimeRange: AnalyticsTimeRange) => {
        void updateSettings({ analyticsTimeRange });
    };

    const setCustomStartDate = (analyticsCustomStartDate: string) => {
        void updateSettings({ analyticsCustomStartDate });
    };

    const setCustomEndDate = (analyticsCustomEndDate: string) => {
        void updateSettings({ analyticsCustomEndDate });
    };

    const setMonthStartsOnFirst = (analyticsMonthStartsOnFirst: boolean) => {
        void updateSettings({ analyticsMonthStartsOnFirst });
    };

    const setHiddenExpenseCategories = (updater: string[] | ((current: string[]) => string[])) => {
        const analyticsHiddenExpenseCategories = typeof updater === 'function'
            ? updater(hiddenExpenseCategories)
            : updater;
        void updateSettings({ analyticsHiddenExpenseCategories });
    };

    const setHiddenIncomeCategories = (updater: string[] | ((current: string[]) => string[])) => {
        const analyticsHiddenIncomeCategories = typeof updater === 'function'
            ? updater(hiddenIncomeCategories)
            : updater;
        void updateSettings({ analyticsHiddenIncomeCategories });
    };

    const dateRange = useMemo(() => {
        const today = new Date();
        let start = subMonths(today, 6);
        let end = today;

        switch (timeRange) {
            case 'week':
                start = monthStartsOnFirst ? startOfMonth(subWeeks(today, 1)) : subWeeks(today, 1);
                break;
            case 'month':
                start = monthStartsOnFirst ? startOfMonth(today) : subMonths(today, 1);
                break;
            case '2months':
                start = monthStartsOnFirst ? startOfMonth(subMonths(today, 2)) : subMonths(today, 2);
                break;
            case '3months':
                start = monthStartsOnFirst ? startOfMonth(subMonths(today, 3)) : subMonths(today, 3);
                break;
            case '6months':
                start = monthStartsOnFirst ? startOfMonth(subMonths(today, 6)) : subMonths(today, 6);
                break;
            case '9months':
                start = monthStartsOnFirst ? startOfMonth(subMonths(today, 9)) : subMonths(today, 9);
                break;
            case 'year':
                start = monthStartsOnFirst ? startOfMonth(subYears(today, 1)) : subYears(today, 1);
                break;
            case 'custom':
                start = new Date(customStartDate);
                end = new Date(customEndDate);
                end.setHours(23, 59, 59, 999);
                break;
        }
        return { start, end };
    }, [timeRange, customStartDate, customEndDate, monthStartsOnFirst]);

    const filteredTransactions = useMemo(() => {
        return transactions.filter(t => {
            return isWithinInterval(new Date(t.date), { start: dateRange.start, end: dateRange.end });
        });
    }, [transactions, dateRange]);

    const expensesByCategory = useMemo(() => {
        const expenses = filteredTransactions.filter(t => t.type === 'expense');
        const byCategory = expenses.reduce((acc, t) => {
            acc[t.category] = (acc[t.category] || 0) + t.amount;
            return acc;
        }, {} as Record<string, number>);

        return Object.entries(byCategory)
            .map(([id, value]) => {
                const category = categories.find(c => c.id === id);
                return {
                    id,
                    name: category ? category.name : 'Inconnu',
                    value,
                    color: category ? category.color : '#9ca3af'
                };
            })
            .sort((a, b) => b.value - a.value);
    }, [filteredTransactions, categories]);

    const incomeByCategory = useMemo(() => {
        const income = filteredTransactions.filter(t => t.type === 'income');
        const byCategory = income.reduce((acc, t) => {
            acc[t.category] = (acc[t.category] || 0) + t.amount;
            return acc;
        }, {} as Record<string, number>);

        return Object.entries(byCategory)
            .map(([id, value]) => {
                const category = categories.find(c => c.id === id);
                return {
                    id,
                    name: category ? category.name : 'Inconnu',
                    value,
                    color: category ? category.color : '#9ca3af'
                };
            })
            .sort((a, b) => b.value - a.value);
    }, [filteredTransactions, categories]);

    const monthlyData = useMemo(() => {
        const diffTime = Math.abs(dateRange.end.getTime() - dateRange.start.getTime());
        const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
        const isShortRange = diffDays <= 31;

        let intervals;
        if (isShortRange) {
            const days = [];
            let current = new Date(dateRange.start);
            while (current <= dateRange.end) {
                days.push(new Date(current));
                current.setDate(current.getDate() + 1);
            }
            intervals = days;
        } else {
            intervals = eachMonthOfInterval({ start: dateRange.start, end: dateRange.end });
        }

        // Pre-group transactions by period key to avoid O(N*M) complexity
        const grouped = filteredTransactions.reduce((acc, t) => {
            const date = new Date(t.date);
            const key = isShortRange ? format(date, 'yyyy-MM-dd') : format(date, 'yyyy-MM');
            if (!acc[key]) acc[key] = { income: 0, expense: 0 };
            if (t.type === 'income') acc[key].income += t.amount;
            else acc[key].expense += t.amount;
            return acc;
        }, {} as Record<string, { income: number, expense: number }>);

        return intervals.map(date => {
            const key = isShortRange ? format(date, 'yyyy-MM-dd') : format(date, 'yyyy-MM');
            const data = grouped[key] || { income: 0, expense: 0 };
            const label = isShortRange 
                ? format(date, 'dd MMM', { locale: fr })
                : format(date, 'MMM yyyy', { locale: fr });

            return {
                name: label,
                Revenus: data.income,
                Dépenses: data.expense
            };
        });
    }, [filteredTransactions, dateRange]);

    const balanceHistory = useMemo(() => {
        const days = [];
        let current = new Date(dateRange.start);
        const end = new Date(dateRange.end);

        while (current <= end) {
            days.push(new Date(current));
            current.setDate(current.getDate() + 1);
        }

        const initialBalances: Record<string, number> = {};
        accounts.forEach(acc => {
            const transactionsBeforeStart = transactions.filter(t =>
                t.accountId === acc.id && new Date(t.date) < dateRange.start
            );
            initialBalances[acc.id] = acc.initialBalance + transactionsBeforeStart.reduce((sum, t) =>
                sum + (t.type === 'income' ? t.amount : -t.amount), 0
            );
        });

        // Group ALL transactions by day once
        const txByDay = transactions.reduce((acc, t) => {
            const key = t.date; // already yyyy-MM-dd
            if (!acc[key]) acc[key] = [];
            acc[key].push(t);
            return acc;
        }, {} as Record<string, typeof transactions>);

        const points = [];
        const currentBalances = { ...initialBalances };

        for (const day of days) {
            const dayKey = format(day, 'yyyy-MM-dd');
            const dayTransactions = txByDay[dayKey] || [];

            dayTransactions.forEach(t => {
                if (currentBalances[t.accountId] !== undefined) {
                    currentBalances[t.accountId] += (t.type === 'income' ? t.amount : -t.amount);
                }
            });

            const dataPoint: any = {
                date: format(day, 'dd MMM', { locale: fr }),
                fullDate: format(day, 'd MMMM yyyy', { locale: fr })
            };

            accounts.forEach(acc => {
                dataPoint[acc.name] = currentBalances[acc.id];
            });

            points.push(dataPoint);
        }

        return points;
    }, [transactions, accounts, dateRange]);

    const toggleExpenseCategory = (id: string) => {
        setHiddenExpenseCategories(prev =>
            prev.includes(id) ? prev.filter(c => c !== id) : [...prev, id]
        );
    };

    const toggleIncomeCategory = (id: string) => {
        setHiddenIncomeCategories(prev =>
            prev.includes(id) ? prev.filter(c => c !== id) : [...prev, id]
        );
    };


    return (
        <div className="space-y-6">
            <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 w-full">
                <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-200">Analyses Financières</h2>

                <div className="hidden md:flex flex-wrap gap-2 period-selector justify-end">
                    {ANALYTICS_TIME_RANGES.map((range) => (
                        <Button
                            key={range}
                            onClick={() => setTimeRange(range)}
                            variant="ghost"
                            size="sm"
                            className={`transition-colors ${timeRange === range
                                ? 'bg-primary-100 text-primary-700 hover:bg-primary-200 dark:bg-primary-900/20 dark:text-primary-400 dark:hover:bg-primary-900/30'
                                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-700'
                                }`}
                        >
                            {ANALYTICS_TIME_RANGE_LABELS[range]}
                        </Button>
                    ))}
                </div>

                <div className="relative md:hidden w-full z-30">
                    <button
                        onClick={() => setIsTimeRangeDropdownOpen(!isTimeRangeDropdownOpen)}
                        className="w-full flex items-center justify-between px-4 py-3 rounded-2xl border border-black/[0.06] dark:border-white/[0.08] bg-white/80 dark:bg-neutral-900/60 text-sm font-semibold text-gray-800 dark:text-gray-200 shadow-sm active:scale-[0.98] transition-all"
                    >
                        <span>{ANALYTICS_TIME_RANGE_LABELS[timeRange]}</span>
                        <ChevronDown className={`w-4 h-4 opacity-60 transition-transform duration-200 ${isTimeRangeDropdownOpen ? 'rotate-180' : ''}`} />
                    </button>
                    
                    {isTimeRangeDropdownOpen && (
                        <>
                            <div 
                                className="fixed inset-0 z-40 bg-transparent" 
                                onClick={() => setIsTimeRangeDropdownOpen(false)}
                            />
                            <div className="absolute left-0 right-0 w-full mt-1.5 z-50 rounded-2xl border border-black/[0.08] dark:border-white/[0.12] bg-white dark:bg-neutral-900 shadow-xl p-1.5 flex flex-col gap-0.5 animate-in fade-in-50 slide-in-from-top-2 duration-150">
                                {ANALYTICS_TIME_RANGES.map(range => (
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
                                        <span>{ANALYTICS_TIME_RANGE_LABELS[range]}</span>
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

            <div className={`transition-all duration-300 ease-in-out overflow-hidden ${timeRange === 'custom' ? 'max-h-48 opacity-100 mt-4 pointer-events-auto' : 'max-h-0 opacity-0 mt-0 pointer-events-none'}`}>
                <div className="flex gap-4 app-card p-4 w-fit ml-auto shadow-none">
                    <div>
                        <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Du</label>
                        <input
                            type="date"
                            value={customStartDate}
                            onChange={(e) => setCustomStartDate(e.target.value)}
                            className="px-3 py-1.5 app-input text-sm"
                        />
                    </div>
                    <div>
                        <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Au</label>
                        <input
                            type="date"
                            value={customEndDate}
                            onChange={(e) => setCustomEndDate(e.target.value)}
                            className="px-3 py-1.5 app-input text-sm"
                        />
                    </div>
                </div>
            </div>

            {/* Balance History Area Chart */}
            <div className="app-card p-6">
                <div className="mb-4 flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200">Évolution du Solde</h3>
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
                </div>
                <div className="h-80 min-w-0" style={{ minHeight: '320px' }}>
                    <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                        <AreaChart data={balanceHistory}>
                            <defs>
                                {accounts.map((acc, index) => (
                                    <linearGradient key={acc.id} id={`colorBalance-${acc.id}`} x1="0" y1="0" x2="0" y2="1">
                                        <stop offset="5%" stopColor={acc.color || COLORS[index % COLORS.length]} stopOpacity={0.8} />
                                        <stop offset="95%" stopColor={acc.color || COLORS[index % COLORS.length]} stopOpacity={0} />
                                    </linearGradient>
                                ))}
                            </defs>
                            <CartesianGrid strokeDasharray="3 3" />
                            <XAxis dataKey="date" minTickGap={30} />
                            <YAxis />
                            <Tooltip content={<CustomTooltip />} />
                            <Legend />
                            {accounts.map((acc, index) => (
                                <Area
                                    key={acc.id}
                                    type="monotone"
                                    dataKey={acc.name}
                                    stroke={acc.color || COLORS[index % COLORS.length]}
                                    strokeWidth={3}
                                    fillOpacity={1}
                                    fill={`url(#colorBalance-${acc.id})`}
                                    dot={false}
                                    activeDot={{ r: 6, strokeWidth: 0 }}
                                />
                            ))}
                        </AreaChart>
                    </ResponsiveContainer>
                </div>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <CategoryPieChart
                    title="Dépenses par Catégorie"
                    data={expensesByCategory}
                    hiddenCategories={hiddenExpenseCategories}
                    onToggle={toggleExpenseCategory}
                />

                <CategoryPieChart
                    title="Revenus par Catégorie"
                    data={incomeByCategory}
                    hiddenCategories={hiddenIncomeCategories}
                    onToggle={toggleIncomeCategory}
                />
            </div>

            {/* Monthly Bar Chart */}
            <div className="app-card p-6">
                <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200 mb-4">Revenus vs Dépenses</h3>
                <div className="h-80 min-w-0" style={{ minHeight: '320px' }}>
                    <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                        <BarChart data={monthlyData}>
                            <CartesianGrid strokeDasharray="3 3" />
                            <XAxis dataKey="name" />
                            <YAxis />
                            <Tooltip
                                content={<CustomTooltip />}
                                cursor={{ fill: 'var(--color-primary)', fillOpacity: 0.1 }}
                            />
                            <Legend />
                            <Bar dataKey="Revenus" name="Revenus" fill="#10B981" radius={[4, 4, 0, 0]} />
                            <Bar dataKey="Dépenses" name="Dépenses" fill="#EF4444" radius={[4, 4, 0, 0]} />
                        </BarChart>
                    </ResponsiveContainer>
                </div>
            </div>
        </div>
    );
};

export default Analytics;
