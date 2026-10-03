import React, { createContext, useContext, useState } from 'react';
const count = Number(new URLSearchParams(location.search).get('count') || 30000);
if (!Number.isInteger(count) || count < 1 || count > 100000) throw new Error('Le banc accepte de 1 à 100 000 opérations fictives.');
const transactions = Array.from({length: count}, (_, index) => ({
 id: `fixture-${index}`, accountId: 'fixture-account', type: index % 7 === 0 ? 'income' : 'expense', amount: (index % 15000) / 100,
 date: new Date(Date.UTC(2026, 9, 3) - Math.floor(index / 10) * 86400000).toISOString().slice(0,10),
 category: 'fixture-category', description: index === count - 1 ? "Zèbre terminal unique" : `Opération synthétique ${index}`, checked: index % 3 === 0,
}));
const noop = async () => {}; // Fixture only: never calls the financial API.
const bank = { accounts: [{id: 'fixture-account', name: 'Compte fictif', type: 'bank', initialBalance: 5000, color:'#6366f1'}], transactions,
 categories: [{id:'fixture-category', name:'Catégorie fictive', icon:'Tag', color:'#6366f1'}], budgets: [], scheduled: [], filterAccount:[],
 addTransaction: noop, addTransfer: noop, updateTransaction: noop, deleteTransaction: noop, processDueScheduledTransactions: noop,
 toggleTransactionCheck: async (id:string) => { (window as any).__lastChecked = id; },
};
const context = createContext(bank);
export function FixtureProvider({ children }: {children: React.ReactNode}) {
    const [items, setItems] = useState(transactions);
    (window as any).__fixturePrepend = () => setItems(items => [{ ...items[0], id:'fixture-new', date:'2027-01-01' }, ...items]);
    (window as any).__fixtureDelete = (id: string) => setItems(items => items.filter(item => item.id !== id));
    return React.createElement(context.Provider, {value: {...bank, transactions:items}}, children);
}
export const useBank = () => useContext(context);
