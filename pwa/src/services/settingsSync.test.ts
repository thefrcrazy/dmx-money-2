import { describe, expect, test } from 'bun:test';
import type { Settings } from '../types';
import {
    applySettingsMutation,
    createSettingsMutation,
    mergeSettingsMutations,
} from './settingsSync';

const baseSettings = (): Settings => ({
    theme: 'light',
    primaryColor: 'default',
    windowPosition: null,
    windowSize: null,
    componentSpacing: 6,
    componentPadding: 6,
    predictionAlertThreshold: 50,
    predictionFakeTransactions: [{
        id: 'fake-a',
        date: '2026-06-19',
        accountId: 'account-a',
        type: 'expense',
        amount: 25,
        category: 'fuel',
        description: 'A',
        enabled: true,
    }],
    dismissedBudgetSuggestions: ['fuel'],
    dismissedScheduledSuggestions: [],
    analyticsHiddenExpenseCategories: ['tax'],
    analyticsHiddenIncomeCategories: [],
});

describe('settings synchronization', () => {
    test('the transport revision is never written back as a user setting', () => {
        const previous = { ...baseSettings(), settingsRevision: 4 };
        const next = { ...previous, settingsRevision: 9 };

        expect(createSettingsMutation(previous, next, ['settingsRevision'])).toEqual({
            schemaVersion: 2,
            baseRevision: 4,
        });
    });

    test('a scalar patch does not overwrite unrelated settings', () => {
        const server = {
            ...baseSettings(),
            predictionAlertThreshold: 250,
            predictionFakeTransactions: [
                ...(baseSettings().predictionFakeTransactions || []),
                {
                    id: 'fake-b',
                    date: '2026-06-20',
                    accountId: 'account-a',
                    type: 'income' as const,
                    amount: 100,
                    category: 'salary',
                    description: 'B',
                    enabled: true,
                },
            ],
        };
        const staleMobile = baseSettings();
        const mobileNext = { ...staleMobile, theme: 'dark' as const };
        const mutation = createSettingsMutation(staleMobile, mobileNext, ['theme']);

        expect(mutation.expectedValues).toEqual({ theme: 'light' });
        expect(applySettingsMutation(server, mutation)).toEqual({
            ...server,
            theme: 'dark',
        });
    });

    test('dismissed suggestions are additive across devices', () => {
        const server = { ...baseSettings(), dismissedBudgetSuggestions: ['fuel', 'tax'] };
        const mobile = baseSettings();
        const mobileNext = { ...mobile, dismissedBudgetSuggestions: ['fuel', 'car'] };
        const mutation = createSettingsMutation(mobile, mobileNext, ['dismissedBudgetSuggestions']);

        expect(applySettingsMutation(server, mutation).dismissedBudgetSuggestions)
            .toEqual(['fuel', 'tax', 'car']);
    });

    test('fake transaction operations preserve remote additions and explicit deletions', () => {
        const server = {
            ...baseSettings(),
            predictionFakeTransactions: [
                ...(baseSettings().predictionFakeTransactions || []),
                {
                    id: 'remote',
                    date: '2026-06-21',
                    accountId: 'account-a',
                    type: 'income' as const,
                    amount: 80,
                    category: 'salary',
                    description: 'Remote',
                    enabled: true,
                },
            ],
        };
        const mobile = baseSettings();
        const mobileNext = { ...mobile, predictionFakeTransactions: [] };
        const mutation = createSettingsMutation(mobile, mobileNext, ['predictionFakeTransactions']);

        expect(applySettingsMutation(server, mutation).predictionFakeTransactions?.map(item => item.id))
            .toEqual(['remote']);
    });

    test('queued mutations keep the latest scalar value and merge collection operations', () => {
        const first = createSettingsMutation(
            baseSettings(),
            { ...baseSettings(), predictionAlertThreshold: 100, dismissedBudgetSuggestions: ['fuel', 'tax'] },
            ['predictionAlertThreshold', 'dismissedBudgetSuggestions'],
        );
        const secondBase = applySettingsMutation(baseSettings(), first);
        const second = createSettingsMutation(
            secondBase,
            { ...secondBase, predictionAlertThreshold: 200, dismissedBudgetSuggestions: ['fuel', 'tax', 'car'] },
            ['predictionAlertThreshold', 'dismissedBudgetSuggestions'],
        );

        const merged = mergeSettingsMutations(first, second);
        const result = applySettingsMutation(baseSettings(), merged);
        expect(merged.expectedValues?.predictionAlertThreshold).toBe(50);
        expect(result.predictionAlertThreshold).toBe(200);
        expect(result.dismissedBudgetSuggestions).toEqual(['fuel', 'tax', 'car']);
    });

    test('fake transaction mutations keep the original item as conflict precondition', () => {
        const previous = baseSettings();
        const updatedTransaction = {
            ...(previous.predictionFakeTransactions || [])[0],
            amount: 40,
        };
        const next = {
            ...previous,
            predictionFakeTransactions: [updatedTransaction],
        };

        const mutation = createSettingsMutation(previous, next, ['predictionFakeTransactions']);
        expect(mutation.predictionFakeTransactionsExpected).toEqual({
            'fake-a': (previous.predictionFakeTransactions || [])[0],
        });
    });
});
