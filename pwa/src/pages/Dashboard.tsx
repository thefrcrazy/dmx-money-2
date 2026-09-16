import { useLocalToday } from '../hooks/useLocalToday';
import React, { useState, useMemo } from 'react';
import { TrendingUp, TrendingDown, DollarSign, Tag, ArrowRightLeft, Wallet, Receipt, Plus, CalendarClock, AlertCircle, CheckCircle2, Clock } from 'lucide-react';
import { useBank } from '../context/BankContext';
import { useNavigation } from '../context/NavigationContext';
import { format, isSameMonth } from 'date-fns';
import { fr } from 'date-fns/locale';
import { PieChart, Pie, Cell, ResponsiveContainer } from 'recharts';
import FormPopup from '../components/ui/FormPopup';
import SearchableSelect from '../components/ui/SearchableSelect';
import Button from '../components/ui/Button';
import Input from '../components/ui/Input';
import Card from '../components/ui/Card';
import { ICONS } from '../constants/icons';
import { useFinancialMetrics } from '../hooks/useFinancialMetrics';
import { formatCurrency } from '../utils/format';

const Dashboard: React.FC = () => {
    const localToday = useLocalToday();
    const { accounts, scheduled, categories, budgets, filterAccount, addAccount } = useBank();
    const { setActivePage } = useNavigation();
    const { monthlyIncome, monthlyExpenses, monthlySaved, relevantTransactions, currentBalance } = useFinancialMetrics();
    
    const [isAccountModalOpen, setIsAccountModalOpen] = useState(false);
    const [newAccountName, setNewAccountName] = useState('');
    const [newAccountType, setNewAccountType] = useState('Courant');
    const [newAccountBalance, setNewAccountBalance] = useState('');
    const [newAccountIcon] = useState('Wallet');
    const [newAccountColor] = useState('#3b82f6');

    // --- Specific Dashboard Logic (Not in global hook) ---

    // Accounts with individual balance (for list)
    const accountsWithBalance = useMemo(() => accounts.map(account => {
        const accountTransactions = relevantTransactions.filter(t => t.accountId === account.id);
        const balance = account.initialBalance + accountTransactions.reduce((sum, t) => {
            return sum + (t.type === 'income' ? t.amount : -t.amount);
        }, 0);
        return { ...account, balance };
    }), [accounts, relevantTransactions]);

    // Categories Breakdown
    const topCategories = useMemo(() => {
        const expensesByCategory = relevantTransactions
            .filter(t => t.type === 'expense' && t.category !== 'transfer' && isSameMonth(new Date(t.date + 'T00:00:00'), localToday))
            .reduce((acc, t) => {
                acc[t.category] = (acc[t.category] || 0) + t.amount;
                return acc;
            }, {} as Record<string, number>);

        return Object.entries(expensesByCategory)
            .map(([id, amount]) => {
                const cat = categories.find(c => c.id === id);
                return {
                    id,
                    name: cat?.name || 'Inconnu',
                    color: cat?.color || '#9ca3af',
                    icon: cat?.icon || 'Tag',
                    amount,
                    percentage: monthlyExpenses > 0 ? (amount / monthlyExpenses) * 100 : 0
                };
            })
            .sort((a, b) => b.amount - a.amount);
    }, [relevantTransactions, categories, monthlyExpenses, localToday]);

    // Budget Logic
    const { totalBudgeted, budgetRemaining, budgetProgress, budgetExpenses } = useMemo(() => {
        const visibleBudgets = filterAccount.length === 0
            ? budgets
            : budgets.filter(budget => !budget.accountId || filterAccount.includes(budget.accountId));
        const total = visibleBudgets.reduce((sum, budget) => sum + budget.amount, 0);
        const expenses = relevantTransactions
            .filter(transaction => {
                if (transaction.type !== 'expense' || transaction.category === 'transfer') return false;
                try {
                    return isSameMonth(new Date(transaction.date), new Date());
                } catch {
                    return false;
                }
            })
            .reduce((sum, transaction) => sum + transaction.amount, 0);

        return {
            totalBudgeted: total,
            budgetRemaining: total - expenses,
            budgetProgress: total > 0 ? Math.min((expenses / total) * 100, 100) : 0,
            budgetExpenses: expenses
        };
    }, [budgets, filterAccount, relevantTransactions]);

    // Upcoming Scheduled
    const upcomingScheduled = useMemo(() => scheduled
        .map(s => {
            const nextDate = new Date(s.nextDate);
            const today = new Date();
            const diffTime = nextDate.getTime() - today.getTime();
            const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
            return { ...s, diffDays, dateObj: nextDate };
        })
        .sort((a, b) => a.dateObj.getTime() - b.dateObj.getTime())
        .slice(0, 3), [scheduled]);

    const renderCategoryIcon = (iconName: string, className: string = "w-4 h-4") => {
        if (iconName === 'ArrowRightLeft') return <ArrowRightLeft className={className} />;
        const Icon = ICONS[iconName] || Tag;
        return <Icon className={className} />;
    };

    const handleAddAccount = (e: React.FormEvent) => {
        e.preventDefault();
        addAccount({ name: newAccountName, type: newAccountType, initialBalance: parseFloat(newAccountBalance) || 0, icon: newAccountIcon, color: newAccountColor });
        setIsAccountModalOpen(false);
        setNewAccountName(''); setNewAccountType('Courant'); setNewAccountBalance('');
    };

    return (
        <div className="space-y-4 md:space-y-8 animate-fade-in-up">
            {/* Assistant : seulement quand le mobile est relié au Mac, qui fait l'analyse. */}
            <h2 className="hidden md:block text-2xl font-bold text-gray-900 dark:text-gray-100">Tableau de Bord</h2>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 md:gap-6">
                {/* 1. Échéances */}
                <Card 
                    title="Échéances" 
                    icon={CalendarClock} 
                    action={<button onClick={() => setActivePage('scheduled')} className="text-xs text-primary-600 hover:underline">Tout voir</button>}
                >
                    <div className="space-y-3 md:space-y-4 min-h-[128px] md:min-h-[140px] flex flex-col justify-center">
                        {upcomingScheduled.length > 0 ? (
                            upcomingScheduled.map(s => (
                                <div key={s.id} className="flex items-center justify-between">
                                    <div className="flex items-center gap-3 overflow-hidden">
                                        <div className={`p-2 rounded-full ${s.diffDays < 0 ? 'bg-red-50 text-red-600 dark:bg-red-500/15 dark:text-red-400' : 'bg-emerald-50 text-emerald-600 dark:bg-emerald-500/15 dark:text-emerald-400'}`}>
                                            {s.diffDays < 0 ? <AlertCircle className="w-4 h-4" /> : <Clock className="w-4 h-4" />}
                                        </div>
                                        <div className="flex flex-col truncate">
                                            <span className="text-sm font-medium truncate">{s.description}</span>
                                            <span className="text-[10px] text-gray-500">
                                                {s.diffDays === 0 ? "Aujourd'hui" : s.diffDays === 1 ? "Demain" : `Dans ${s.diffDays} jours`}
                                            </span>
                                        </div>
                                    </div>
                                    <span className="font-bold text-sm">{formatCurrency(s.amount)}</span>
                                </div>
                            ))
                        ) : (
                            <div className="flex flex-col items-center text-neutral-400 dark:text-neutral-600">
                                <div className="w-12 h-12 rounded-full bg-neutral-50 dark:bg-neutral-900 flex items-center justify-center mb-2">
                                    <CheckCircle2 className="w-6 h-6 opacity-20 dark:opacity-40" />
                                </div>
                                <span className="text-xs">Aucune échéance à venir</span>
                            </div>
                        )}
                    </div>
                </Card>

                {/* 2. Opérations */}
                <Card 
                    title="Opérations" 
                    icon={ArrowRightLeft}
                    action={<span className="text-[10px] bg-gray-100 dark:bg-neutral-800 px-2 py-0.5 rounded-full text-gray-500 dark:text-neutral-300">Ce mois-ci</span>}
                >
                    <div className="flex flex-col items-center justify-center min-h-[128px] md:min-h-[140px]">
                        <div className="text-[10px] text-gray-400 uppercase font-bold tracking-widest mb-1">Revenus - Dépenses</div>
                        <div className={`text-4xl font-bold mb-5 md:mb-6 ${monthlySaved >= 0 ? 'text-emerald-600' : 'text-red-600'}`}>
                            {monthlySaved > 0 ? '+' : ''}{Math.round(monthlySaved)} €
                        </div>
                        <div className="grid grid-cols-3 w-full gap-2 text-center border-t border-black/[0.05] dark:border-white/10 pt-4">
                            <div>
                                <div className="flex items-center justify-center gap-1 text-[10px] text-gray-500 mb-0.5">
                                    <TrendingUp className="w-3 h-3 text-emerald-500" /> Revenus
                                </div>
                                <div className="text-xs font-bold">{Math.round(monthlyIncome)} €</div>
                            </div>
                            <div>
                                <div className="flex items-center justify-center gap-1 text-[10px] text-gray-500 mb-0.5">
                                    <TrendingDown className="w-3 h-3 text-red-500" /> Dépenses
                                </div>
                                <div className="text-xs font-bold">{Math.round(monthlyExpenses)} €</div>
                            </div>
                            <div>
                                <div className="flex items-center justify-center gap-1 text-[10px] text-gray-500 mb-0.5">
                                    <Wallet className="w-3 h-3 text-primary-500" /> Epargné
                                </div>
                                <div className={`text-xs font-bold ${monthlySaved >= 0 ? 'text-emerald-500' : 'text-red-500'}`}>{Math.round(monthlySaved)} €</div>
                            </div>
                        </div>
                    </div>
                </Card>

                {/* 3. Catégories */}
                <Card 
                    title="Catégories" 
                    icon={Tag}
                    action={<span className="text-[10px] bg-gray-100 dark:bg-neutral-800 px-2 py-0.5 rounded-full text-gray-500 dark:text-neutral-300">Ce mois-ci</span>}
                >
                    <div className="min-h-[128px] md:min-h-[140px] flex items-center justify-center text-gray-400 text-xs text-center">
                        {topCategories.length > 0 ? (
                            <div className="w-full space-y-3">
                                {topCategories.slice(0, 3).map(cat => (
                                    <div key={cat.id}>
                                        <div className="flex items-center justify-between gap-3">
                                            <div className="flex min-w-0 items-center gap-2">
                                                <div className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: cat.color }} />
                                                <span className="truncate text-[15px] text-gray-700 dark:text-gray-300 md:text-xs">{cat.name}</span>
                                            </div>
                                            <span className="text-[15px] font-semibold tabular-nums text-gray-900 dark:text-gray-100 md:text-xs">{Math.round(cat.percentage)}%</span>
                                        </div>
                                        <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-black/[0.06] dark:bg-white/10">
                                            <div className="h-full rounded-full" style={{ width: `${Math.min(100, cat.percentage)}%`, backgroundColor: cat.color }} />
                                        </div>
                                    </div>
                                ))}
                            </div>
                        ) : "Aucune dépense ce mois-ci"}
                    </div>
                </Card>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 md:gap-6">
                {/* 4. Mes Comptes */}
                <Card 
                    title="Mes Comptes" 
                    icon={Wallet}
                    action={<button onClick={() => setIsAccountModalOpen(true)} className="text-primary-600 hover:bg-primary-50 p-1 rounded-full"><Plus className="w-4 h-4" /></button>}
                    noPadding
                >
                    <div className="min-h-[200px] flex flex-col">
                        <div className="flex-1 divide-y divide-neutral-100 dark:divide-neutral-800">
                            {accountsWithBalance.map(acc => (
                                <div key={acc.id} className="px-4 md:px-6 py-3 flex items-center justify-between hover:bg-neutral-50/50 dark:hover:bg-neutral-800/50 transition-colors cursor-pointer">
                                    <div className="flex items-center gap-3">
                                        <div className="w-8 h-8 rounded-lg flex items-center justify-center" style={{ backgroundColor: `${acc.color}15`, color: acc.color }}>
                                            {renderCategoryIcon(acc.icon || 'Wallet')}
                                        </div>
                                        <span className="text-sm font-medium">{acc.name}</span>
                                    </div>
                                    <span className={`text-sm font-bold ${acc.balance < 0 ? 'text-red-600' : ''}`}>{formatCurrency(acc.balance)}</span>
                                </div>
                            ))}
                        </div>
                        <div className="px-4 md:px-6 py-4 border-t border-black/[0.05] dark:border-white/10 flex justify-between items-center bg-neutral-50/30 dark:bg-neutral-900/50">
                            <span className="text-xs font-bold text-neutral-400 dark:text-neutral-500 uppercase tracking-widest">Total</span>
                            <span className="text-lg font-bold text-primary-600">{formatCurrency(currentBalance)}</span>
                        </div>
                    </div>
                </Card>

                {/* 5. Dernières */}
                <Card 
                    title="Dernières" 
                    icon={Receipt}
                    action={<button onClick={() => setActivePage('transactions')} className="text-xs text-primary-600 hover:underline">Tout voir</button>}
                    noPadding
                >
                    <div className="min-h-[200px] divide-y divide-neutral-100 dark:divide-neutral-800">
                        {relevantTransactions.slice(0, 5).map(t => {
                            const cat = categories.find(c => c.id === t.category);
                            return (
                                <div key={t.id} className="px-4 md:px-6 py-3 flex items-center justify-between hover:bg-neutral-50/50 dark:hover:bg-neutral-800/50 transition-colors cursor-pointer">
                                    <div className="flex items-center gap-3 overflow-hidden">
                                        <div className="w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: `${cat?.color || '#9ca3af'}15`, color: cat?.color || '#9ca3af' }}>
                                            {renderCategoryIcon(cat?.icon || 'Tag')}
                                        </div>
                                        <div className="flex flex-col truncate">
                                            <span className="text-sm font-medium truncate">{t.description}</span>
                                            <span className="text-[10px] text-neutral-400 dark:text-neutral-500">{format(new Date(t.date), 'dd MMM', { locale: fr })}</span>
                                        </div>
                                    </div>
                                    <span className={`text-sm font-bold ${t.type === 'income' ? 'text-emerald-600' : ''}`}>
                                        {t.type === 'income' ? '+' : ''}{formatCurrency(t.amount)}
                                    </span>
                                </div>
                            );
                        })}
                    </div>
                </Card>

                {/* 6. Budget */}
                <Card title="Budget" icon={DollarSign} action={<span className="text-[10px] bg-gray-100 dark:bg-neutral-800 px-2 py-0.5 rounded-full text-gray-500 dark:text-neutral-300">Ce mois-ci</span>}>
                    <div className="min-h-[200px] flex flex-col items-center justify-center">
                        <div className="relative w-28 h-28 flex items-center justify-center mb-4">
                            <ResponsiveContainer width="100%" height="100%" minWidth={0}>
                                <PieChart>
                                    <Pie data={[{ value: budgetExpenses }, { value: Math.max(0, totalBudgeted - budgetExpenses) }]} innerRadius={35} outerRadius={45} startAngle={90} endAngle={-270} dataKey="value" stroke="none">
                                        <Cell fill={budgetRemaining >= 0 ? "#10b981" : "#ef4444"} />
                                        <Cell fill="var(--color-border)" opacity={0.2} />
                                    </Pie>
                                </PieChart>
                            </ResponsiveContainer>
                            <div className="absolute inset-0 flex flex-col items-center justify-center">
                                <span className="text-[9px] text-gray-400 uppercase font-bold">Dépenses</span>
                                <span className="text-lg font-bold">{Math.round(budgetProgress)}%</span>
                            </div>
                        </div>
                        <div className="grid grid-cols-3 w-full text-center gap-1">
                            <div>
                                <div className="text-[9px] text-gray-400 uppercase font-bold">Restant</div>
                                <div className={`text-xs font-bold ${budgetRemaining < 0 ? 'text-red-600' : ''}`}>{Math.round(budgetRemaining)} €</div>
                            </div>
                            <div>
                                <div className="text-[9px] text-gray-400 uppercase font-bold">Dépenses</div>
                                <div className="text-xs font-bold text-emerald-600">{Math.round(budgetExpenses)} €</div>
                            </div>
                            <div>
                                <div className="text-[9px] text-gray-400 uppercase font-bold">Prévu</div>
                                <div className="text-xs font-bold">{Math.round(totalBudgeted)} €</div>
                            </div>
                        </div>
                    </div>
                </Card>
            </div>

            <FormPopup isOpen={isAccountModalOpen} onClose={() => setIsAccountModalOpen(false)}>
                <form onSubmit={handleAddAccount} className="p-6 space-y-6">
                    <h3 className="text-lg font-semibold">Ajouter un compte</h3>
                    <div className="space-y-4">
                        <Input label="Nom" required value={newAccountName} onChange={e => setNewAccountName(e.target.value)} />
                        <SearchableSelect label="Type" value={newAccountType} onChange={setNewAccountType} options={[{id:'Courant',label:'Courant'},{id:'Épargne',label:'Épargne'}]} />
                        <Input label="Solde Initial" type="number" required value={newAccountBalance} onChange={e => setNewAccountBalance(e.target.value)} rightElement="€" />
                        <div className="flex gap-3 pt-4">
                            <Button type="button" variant="secondary" fullWidth onClick={() => setIsAccountModalOpen(false)}>Annuler</Button>
                            <Button type="submit" fullWidth>Créer</Button>
                        </div>
                    </div>
                </form>
            </FormPopup>
        </div>
    );
};

export default Dashboard;
