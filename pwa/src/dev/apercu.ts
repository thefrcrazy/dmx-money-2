/**
 * Aperçu de la PWA sans Mac ni clé d'accès, pour travailler l'interface dans un navigateur :
 * `bun run dev`, puis ouvrir `/mobile/?apercu` (ajouter `&theme=dark` pour le thème sombre).
 *
 * Les appels au pont (`/api/*`, `/auth/*`) sont servis ici avec des données fictives gardées en
 * mémoire. Ce module n'est chargé qu'en développement (voir main.tsx) : le build n'en contient rien.
 */
import type { Account, Budget, Category, ScheduledTransaction, Transaction, TransactionType } from '../types';
import { LATEST_VERSION } from '../constants/changelog';
import { markMobilePasskeyReady, setMobileApiBaseUrl, setMobileCsrfToken } from '../utils/runtime';

const pad = (value: number) => String(value).padStart(2, '0');
const isoDate = (date: Date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;

const today = () => {
    const date = new Date();
    date.setHours(12, 0, 0, 0);
    return date;
};

const daysAgo = (days: number) => {
    const date = today();
    date.setDate(date.getDate() - days);
    return date;
};

const dayOfMonth = (monthOffset: number, day: number) => {
    const now = today();
    return new Date(now.getFullYear(), now.getMonth() + monthOffset, day, 12);
};

/** Prochaine occurrence mensuelle du jour donné, aujourd'hui exclu. */
const nextMonthly = (day: number) => {
    const date = dayOfMonth(0, day);
    return date > today() ? date : dayOfMonth(1, day);
};

const categories: Category[] = [
    { id: '1', name: 'Loyer / Prêt', icon: 'Home', color: '#1e3a8a' },
    { id: '2', name: 'Charges / Énergie', icon: 'Zap', color: '#f59e0b' },
    { id: '5', name: 'Alimentation', icon: 'ShoppingBag', color: '#ef4444' },
    { id: '6', name: 'Restaurants / Cafés', icon: 'Utensils', color: '#ea580c' },
    { id: '9', name: 'Carburant', icon: 'Fuel', color: '#b45309' },
    { id: '15', name: 'Loisirs / Cinéma', icon: 'Gamepad2', color: '#8b5cf6' },
    { id: '16', name: 'Abonnements (VOD/Musique)', icon: 'Tv', color: '#6366f1' },
    { id: '17', name: 'Sport / Bien-être', icon: 'Dumbbell', color: '#06b6d4' },
    { id: '19', name: 'Téléphonie / Internet', icon: 'Wifi', color: '#3b82f6' },
    { id: '21', name: 'Salaire', icon: 'Banknote', color: '#16a34a' },
    { id: '24', name: 'Remboursements', icon: 'TrendingUp', color: '#4ade80' },
    { id: 'transfer', name: 'Virement', icon: 'ArrowRightLeft', color: '#64748b' },
];

const accounts: Account[] = [
    { id: 'courant', name: 'Compte courant', type: 'Courant', initialBalance: 1240.5, color: '#007AFF', icon: 'Landmark' },
    { id: 'livret', name: 'Livret A', type: 'Épargne', initialBalance: 5800, color: '#34C759', icon: 'PiggyBank' },
    { id: 'pea', name: 'PEA', type: 'Investissement', initialBalance: 3200, color: '#AF52DE', icon: 'TrendingUp' },
    { id: 'especes', name: 'Espèces', type: 'Espèces', initialBalance: 150, color: '#FF9500', icon: 'Wallet' },
];

const transactions: Transaction[] = [];
let sequence = 0;

const add = (date: Date, accountId: string, type: TransactionType, amount: number, category: string, description: string) => {
    if (date > today()) return;
    transactions.push({
        id: `apercu-${++sequence}`,
        date: isoDate(date),
        accountId,
        type,
        amount,
        category,
        description,
        checked: date < daysAgo(4),
    });
};

for (const month of [-2, -1, 0]) {
    add(dayOfMonth(month, 1), 'courant', 'income', 2480, '21', 'Salaire');
    add(dayOfMonth(month, 5), 'courant', 'expense', 760, '1', 'Loyer');
    add(dayOfMonth(month, 8), 'courant', 'expense', 34.99, '19', 'Box internet');
    add(dayOfMonth(month, 12), 'courant', 'expense', 72.4, '2', 'Électricité');
    add(dayOfMonth(month, 15), 'courant', 'expense', 13.49, '16', 'Netflix');
    add(dayOfMonth(month, 18), 'courant', 'expense', 29.9, '17', 'Salle de sport');

    const transferDate = dayOfMonth(month, 3);
    if (transferDate <= today()) {
        const out = `apercu-${++sequence}`;
        const into = `apercu-${++sequence}`;
        const shared = { date: isoDate(transferDate), amount: 200, category: 'transfer', checked: true, isTransfer: true };
        transactions.push({ ...shared, id: out, accountId: 'courant', type: 'expense', description: 'Virement vers Livret A', linkedTransactionId: into });
        transactions.push({ ...shared, id: into, accountId: 'livret', type: 'income', description: 'Virement depuis Compte courant', linkedTransactionId: out });
    }
}

const groceries: Array<[string, number]> = [
    ['Carrefour', 64.2], ['Marché', 23.75], ['Lidl', 48.4], ['Boulangerie', 7.6], ['Monoprix', 31.1], ['Picard', 26.35],
];
for (let day = 1, index = 0; day <= 62; day += 3, index++) {
    const [description, amount] = groceries[index % groceries.length];
    add(daysAgo(day), 'courant', 'expense', amount, '5', description);
}

const outings: Array<[string, number, string]> = [
    ['Café', 4.2, '6'], ['Restaurant italien', 38.5, '6'], ['Cinéma', 11.8, '15'], ['Brunch', 24, '6'],
];
for (let day = 2, index = 0; day <= 62; day += 5, index++) {
    const [description, amount, category] = outings[index % outings.length];
    add(daysAgo(day), index % 3 === 0 ? 'especes' : 'courant', 'expense', amount, category, description);
}

for (let day = 6; day <= 62; day += 11) {
    add(daysAgo(day), 'courant', 'expense', 58.3, '9', 'Station Total');
}
add(daysAgo(9), 'courant', 'income', 34.2, '24', 'Remboursement mutuelle');
add(daysAgo(20), 'pea', 'income', 86.4, '24', 'Dividendes');
transactions.sort((a, b) => b.date.localeCompare(a.date));

const budgets: Budget[] = [
    { id: 'budget-courses', name: 'Courses', amount: 380, category: '5' },
    { id: 'budget-sorties', name: 'Sorties', amount: 150, category: '6' },
    { id: 'budget-carburant', name: 'Carburant', amount: 180, category: '9' },
    { id: 'budget-abonnements', name: 'Abonnements', amount: 50, category: '16' },
];

const scheduled: ScheduledTransaction[] = [
    { id: 'echeance-salaire', description: 'Salaire', amount: 2480, type: 'income', frequency: 'monthly', accountId: 'courant', nextDate: isoDate(nextMonthly(1)), category: '21' },
    { id: 'echeance-epargne', description: 'Épargne mensuelle', amount: 200, type: 'transfer', frequency: 'monthly', accountId: 'courant', toAccountId: 'livret', nextDate: isoDate(nextMonthly(3)), category: 'transfer' },
    { id: 'echeance-loyer', description: 'Loyer', amount: 760, type: 'expense', frequency: 'monthly', accountId: 'courant', nextDate: isoDate(nextMonthly(5)), category: '1' },
    { id: 'echeance-internet', description: 'Box internet', amount: 34.99, type: 'expense', frequency: 'monthly', accountId: 'courant', nextDate: isoDate(nextMonthly(8)), category: '19' },
    { id: 'echeance-netflix', description: 'Netflix', amount: 13.49, type: 'expense', frequency: 'monthly', accountId: 'courant', nextDate: isoDate(nextMonthly(15)), category: '16', budgetId: 'budget-abonnements' },
];

const settings: Record<string, unknown> = {
    settingsRevision: 1,
    theme: 'light',
    primaryColor: '#007AFF',
    windowPosition: null,
    windowSize: null,
    componentSpacing: 6,
    componentPadding: 6,
    lastSeenVersion: LATEST_VERSION,
};

const collections: Record<string, Array<{ id: string }>> = { accounts, transactions, categories, budgets, scheduled };
let dataVersion = 1;

const json = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

const parseBody = (init?: RequestInit): Record<string, unknown> | undefined => {
    if (typeof init?.body !== 'string' || !init.body) return undefined;
    try {
        return JSON.parse(init.body) as Record<string, unknown>;
    } catch {
        return undefined;
    }
};

const updateCollection = (items: Array<{ id: string }>, method: string, body?: Record<string, unknown>) => {
    const id = typeof body?.id === 'string' ? body.id : undefined;
    const index = id ? items.findIndex(item => item.id === id) : -1;
    if (method === 'POST' && id) items.unshift(body as { id: string });
    if (method === 'PUT' && index >= 0) items[index] = body as { id: string };
    if (method === 'DELETE' && index >= 0) items.splice(index, 1);
    dataVersion += 1;
};

const handle = (url: URL, init?: RequestInit): Response => {
    const method = (init?.method || 'GET').toUpperCase();
    const body = parseBody(init);
    const [, scope, name, extra] = url.pathname.split('/');

    if (scope === 'auth') return json({ ok: true, csrfToken: 'apercu' });
    if (name === 'status') return json({ ok: true, dataVersion });
    if (name === 'assistant') {
        return json({
            ok: true,
            summary: 'Aperçu : sur le vrai mobile, le Mac répond ici.',
            details: [],
            changed: false,
            understood: true,
            interpreted: String(body?.text ?? ''),
        });
    }
    if (name === 'scheduled' && extra === 'process-due') return json({ processed: 0 });
    if (name === 'settings') {
        if (method === 'GET') return json(settings);
        const values = body?.values && typeof body.values === 'object' ? body.values as Record<string, unknown> : body;
        Object.assign(settings, values, { settingsRevision: Number(settings.settingsRevision) + 1 });
        dataVersion += 1;
        return json({ ok: true, revision: settings.settingsRevision, conflicts: [] });
    }
    if (name === 'transfers') {
        dataVersion += 1;
        return json({ ok: true });
    }
    const items = collections[name];
    if (items) {
        if (method === 'GET') return json(items);
        updateCollection(items, method, body);
        return json({ ok: true });
    }
    return json({ error: 'Route introuvable' }, 404);
};

export const installPreview = () => {
    const params = new URLSearchParams(window.location.search);
    const theme = params.get('theme');
    if (theme === 'dark' || theme === 'light' || theme === 'system') settings.theme = theme;

    setMobileApiBaseUrl(window.location.origin);
    setMobileCsrfToken('apercu');
    markMobilePasskeyReady();
    // La PWA n'affiche l'application qu'une fois installée sur l'écran d'accueil ; montants à la
    // française, comme sur un iPhone réglé en français.
    Object.defineProperty(navigator, 'standalone', { configurable: true, value: true });
    Object.defineProperty(navigator, 'language', { configurable: true, value: 'fr-FR' });

    const nativeFetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
        const url = new URL(input instanceof Request ? input.url : String(input), window.location.origin);
        const isBridgeCall = url.origin === window.location.origin && /^\/(api|auth)\//.test(url.pathname);
        return isBridgeCall ? handle(url, init) : nativeFetch(input, init);
    };

    // Onglet et bouton à ouvrir d'emblée, pour les captures : `&page=Journal&ouvrir=Netflix`.
    const clickButton = (label: string | null, scope: string) => {
        if (!label) return;
        const buttons = [...document.querySelectorAll<HTMLElement>(`${scope} button`)];
        buttons.find(button => button.textContent?.trim().startsWith(label))?.click();
    };
    window.setTimeout(() => {
        clickButton(params.get('page'), 'nav');
        window.setTimeout(() => clickButton(params.get('ouvrir'), 'main'), 800);
    }, 1500);
};
