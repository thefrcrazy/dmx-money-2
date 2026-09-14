import type { PredictionFakeTransaction, Settings } from '../types';

export const SETTINGS_SYNC_SCHEMA_VERSION = 2;

type SettingsValuePatch = Partial<Omit<
    Settings,
    | 'settingsRevision'
    | 'dismissedBudgetSuggestions'
    | 'dismissedScheduledSuggestions'
    | 'predictionFakeTransactions'
    | 'analyticsHiddenExpenseCategories'
    | 'analyticsHiddenIncomeCategories'
>>;

type SettingsExpectedValues = Partial<Record<keyof SettingsValuePatch, unknown>>;

export interface SettingsMutation {
    schemaVersion: number;
    baseRevision: number;
    values?: SettingsValuePatch;
    expectedValues?: SettingsExpectedValues;
    dismissedBudgetSuggestionsAdd?: string[];
    dismissedScheduledSuggestionsAdd?: string[];
    predictionFakeTransactionsUpsert?: PredictionFakeTransaction[];
    predictionFakeTransactionDeleteIds?: string[];
    predictionFakeTransactionsExpected?: Record<string, PredictionFakeTransaction | null>;
    analyticsHiddenExpenseCategoriesAdd?: string[];
    analyticsHiddenExpenseCategoriesRemove?: string[];
    analyticsHiddenExpenseCategoriesExpected?: Record<string, boolean>;
    analyticsHiddenIncomeCategoriesAdd?: string[];
    analyticsHiddenIncomeCategoriesRemove?: string[];
    analyticsHiddenIncomeCategoriesExpected?: Record<string, boolean>;
}

const unique = (values: string[] = []) => Array.from(new Set(values));

const difference = (left: string[] = [], right: string[] = []) => {
    const rightSet = new Set(right);
    return unique(left).filter(value => !rightSet.has(value));
};

const sameValue = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);

const mergeSetOperations = (
    currentAdd: string[] = [],
    currentRemove: string[] = [],
    nextAdd: string[] = [],
    nextRemove: string[] = [],
) => {
    const add = new Set(currentAdd);
    const remove = new Set(currentRemove);

    nextRemove.forEach(value => {
        add.delete(value);
        remove.add(value);
    });
    nextAdd.forEach(value => {
        remove.delete(value);
        add.add(value);
    });

    return {
        add: Array.from(add),
        remove: Array.from(remove),
    };
};

export const createSettingsMutation = (
    previous: Settings,
    next: Settings,
    changedKeys: Array<keyof Settings>,
): SettingsMutation => {
    const mutation: SettingsMutation = {
        schemaVersion: SETTINGS_SYNC_SCHEMA_VERSION,
        baseRevision: previous.settingsRevision || 0,
    };
    const values: SettingsValuePatch = {};
    const expectedValues: SettingsExpectedValues = {};

    changedKeys.forEach(key => {
        if (sameValue(previous[key], next[key])) return;

        switch (key) {
            case 'settingsRevision':
                break;
            case 'dismissedBudgetSuggestions':
                mutation.dismissedBudgetSuggestionsAdd = difference(
                    next.dismissedBudgetSuggestions,
                    previous.dismissedBudgetSuggestions,
                );
                break;
            case 'dismissedScheduledSuggestions':
                mutation.dismissedScheduledSuggestionsAdd = difference(
                    next.dismissedScheduledSuggestions,
                    previous.dismissedScheduledSuggestions,
                );
                break;
            case 'predictionFakeTransactions': {
                const previousById = new Map(
                    (previous.predictionFakeTransactions || []).map(transaction => [transaction.id, transaction]),
                );
                const nextById = new Map(
                    (next.predictionFakeTransactions || []).map(transaction => [transaction.id, transaction]),
                );
                mutation.predictionFakeTransactionsUpsert = Array.from(nextById.values())
                    .filter(transaction => !sameValue(previousById.get(transaction.id), transaction));
                mutation.predictionFakeTransactionDeleteIds = Array.from(previousById.keys())
                    .filter(id => !nextById.has(id));
                mutation.predictionFakeTransactionsExpected = {};
                mutation.predictionFakeTransactionsUpsert.forEach(transaction => {
                    mutation.predictionFakeTransactionsExpected![transaction.id] =
                        previousById.get(transaction.id) || null;
                });
                mutation.predictionFakeTransactionDeleteIds.forEach(id => {
                    mutation.predictionFakeTransactionsExpected![id] = previousById.get(id) || null;
                });
                break;
            }
            case 'analyticsHiddenExpenseCategories': {
                mutation.analyticsHiddenExpenseCategoriesAdd = difference(
                    next.analyticsHiddenExpenseCategories,
                    previous.analyticsHiddenExpenseCategories,
                );
                mutation.analyticsHiddenExpenseCategoriesRemove = difference(
                    previous.analyticsHiddenExpenseCategories,
                    next.analyticsHiddenExpenseCategories,
                );
                mutation.analyticsHiddenExpenseCategoriesExpected = {};
                mutation.analyticsHiddenExpenseCategoriesAdd.forEach(value => {
                    mutation.analyticsHiddenExpenseCategoriesExpected![value] = false;
                });
                mutation.analyticsHiddenExpenseCategoriesRemove.forEach(value => {
                    mutation.analyticsHiddenExpenseCategoriesExpected![value] = true;
                });
                break;
            }
            case 'analyticsHiddenIncomeCategories': {
                mutation.analyticsHiddenIncomeCategoriesAdd = difference(
                    next.analyticsHiddenIncomeCategories,
                    previous.analyticsHiddenIncomeCategories,
                );
                mutation.analyticsHiddenIncomeCategoriesRemove = difference(
                    previous.analyticsHiddenIncomeCategories,
                    next.analyticsHiddenIncomeCategories,
                );
                mutation.analyticsHiddenIncomeCategoriesExpected = {};
                mutation.analyticsHiddenIncomeCategoriesAdd.forEach(value => {
                    mutation.analyticsHiddenIncomeCategoriesExpected![value] = false;
                });
                mutation.analyticsHiddenIncomeCategoriesRemove.forEach(value => {
                    mutation.analyticsHiddenIncomeCategoriesExpected![value] = true;
                });
                break;
            }
            default:
                Object.assign(values, { [key]: next[key] });
                Object.assign(expectedValues, { [key]: previous[key] });
        }
    });

    if (Object.keys(values).length > 0) mutation.values = values;
    if (Object.keys(expectedValues).length > 0) mutation.expectedValues = expectedValues;
    return compactSettingsMutation(mutation);
};

export const applySettingsMutation = (
    settings: Settings,
    mutation: SettingsMutation,
): Settings => {
    const next: Settings = {
        ...settings,
        ...(mutation.values || {}),
    };

    next.dismissedBudgetSuggestions = unique([
        ...(settings.dismissedBudgetSuggestions || []),
        ...(mutation.dismissedBudgetSuggestionsAdd || []),
    ]);
    next.dismissedScheduledSuggestions = unique([
        ...(settings.dismissedScheduledSuggestions || []),
        ...(mutation.dismissedScheduledSuggestionsAdd || []),
    ]);

    const fakeTransactions = new Map(
        (settings.predictionFakeTransactions || []).map(transaction => [transaction.id, transaction]),
    );
    (mutation.predictionFakeTransactionDeleteIds || []).forEach(id => fakeTransactions.delete(id));
    (mutation.predictionFakeTransactionsUpsert || []).forEach(transaction => {
        fakeTransactions.set(transaction.id, transaction);
    });
    next.predictionFakeTransactions = Array.from(fakeTransactions.values());

    const applySetMutation = (
        current: string[] = [],
        add: string[] = [],
        remove: string[] = [],
    ) => {
        const values = new Set(current);
        remove.forEach(value => values.delete(value));
        add.forEach(value => values.add(value));
        return Array.from(values);
    };

    next.analyticsHiddenExpenseCategories = applySetMutation(
        settings.analyticsHiddenExpenseCategories,
        mutation.analyticsHiddenExpenseCategoriesAdd,
        mutation.analyticsHiddenExpenseCategoriesRemove,
    );
    next.analyticsHiddenIncomeCategories = applySetMutation(
        settings.analyticsHiddenIncomeCategories,
        mutation.analyticsHiddenIncomeCategoriesAdd,
        mutation.analyticsHiddenIncomeCategoriesRemove,
    );

    return next;
};

export const mergeSettingsMutations = (
    current: SettingsMutation,
    incoming: SettingsMutation,
): SettingsMutation => {
    const fakeUpserts = new Map(
        (current.predictionFakeTransactionsUpsert || []).map(transaction => [transaction.id, transaction]),
    );
    const fakeDeletes = new Set(current.predictionFakeTransactionDeleteIds || []);

    (incoming.predictionFakeTransactionDeleteIds || []).forEach(id => {
        fakeUpserts.delete(id);
        fakeDeletes.add(id);
    });
    (incoming.predictionFakeTransactionsUpsert || []).forEach(transaction => {
        fakeDeletes.delete(transaction.id);
        fakeUpserts.set(transaction.id, transaction);
    });

    const expense = mergeSetOperations(
        current.analyticsHiddenExpenseCategoriesAdd,
        current.analyticsHiddenExpenseCategoriesRemove,
        incoming.analyticsHiddenExpenseCategoriesAdd,
        incoming.analyticsHiddenExpenseCategoriesRemove,
    );
    const income = mergeSetOperations(
        current.analyticsHiddenIncomeCategoriesAdd,
        current.analyticsHiddenIncomeCategoriesRemove,
        incoming.analyticsHiddenIncomeCategoriesAdd,
        incoming.analyticsHiddenIncomeCategoriesRemove,
    );

    return compactSettingsMutation({
        schemaVersion: Math.max(current.schemaVersion, incoming.schemaVersion),
        baseRevision: Math.min(current.baseRevision, incoming.baseRevision),
        values: {
            ...(current.values || {}),
            ...(incoming.values || {}),
        },
        expectedValues: {
            ...(incoming.expectedValues || {}),
            ...(current.expectedValues || {}),
        },
        dismissedBudgetSuggestionsAdd: unique([
            ...(current.dismissedBudgetSuggestionsAdd || []),
            ...(incoming.dismissedBudgetSuggestionsAdd || []),
        ]),
        dismissedScheduledSuggestionsAdd: unique([
            ...(current.dismissedScheduledSuggestionsAdd || []),
            ...(incoming.dismissedScheduledSuggestionsAdd || []),
        ]),
        predictionFakeTransactionsUpsert: Array.from(fakeUpserts.values()),
        predictionFakeTransactionDeleteIds: Array.from(fakeDeletes),
        predictionFakeTransactionsExpected: {
            ...(incoming.predictionFakeTransactionsExpected || {}),
            ...(current.predictionFakeTransactionsExpected || {}),
        },
        analyticsHiddenExpenseCategoriesAdd: expense.add,
        analyticsHiddenExpenseCategoriesRemove: expense.remove,
        analyticsHiddenExpenseCategoriesExpected: {
            ...(incoming.analyticsHiddenExpenseCategoriesExpected || {}),
            ...(current.analyticsHiddenExpenseCategoriesExpected || {}),
        },
        analyticsHiddenIncomeCategoriesAdd: income.add,
        analyticsHiddenIncomeCategoriesRemove: income.remove,
        analyticsHiddenIncomeCategoriesExpected: {
            ...(incoming.analyticsHiddenIncomeCategoriesExpected || {}),
            ...(current.analyticsHiddenIncomeCategoriesExpected || {}),
        },
    });
};

export const compactSettingsMutation = (mutation: SettingsMutation): SettingsMutation => {
    const compacted = { ...mutation };
    Object.entries(compacted).forEach(([key, value]) => {
        if (key === 'schemaVersion') return;
        if (key === 'baseRevision') return;
        if (Array.isArray(value) && value.length === 0) {
            delete compacted[key as keyof SettingsMutation];
        } else if (value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === 0) {
            delete compacted[key as keyof SettingsMutation];
        }
    });
    return compacted;
};

export const hasSettingsMutationChanges = (mutation: SettingsMutation) => (
    Object.keys(mutation).some(key => key !== 'schemaVersion' && key !== 'baseRevision')
);
