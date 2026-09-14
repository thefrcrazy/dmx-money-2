import { describe, expect, test } from 'bun:test';
import {
    addDayFlow,
    applyDayFlow,
    detectClosingCrossing,
    detectIntradayRisk,
    emptyDayFlow,
    highestSeverity,
} from './predictions';

describe('addDayFlow', () => {
    test('keeps withdrawals and income apart', () => {
        const flow = emptyDayFlow();
        addDayFlow(flow, -50_000);
        addDayFlow(flow, 120_000);
        addDayFlow(flow, -1_000);

        expect(flow).toEqual({ debits: -51_000, credits: 120_000 });
    });

    test('counts a zero amount as income and leaves both sides untouched', () => {
        expect(addDayFlow(emptyDayFlow(), 0)).toEqual({ debits: 0, credits: 0 });
    });
});

describe('applyDayFlow', () => {
    test('settles withdrawals before income', () => {
        // 200 € on the account, 500 € rent leaves, 1 000 € salary arrives.
        const { low, close } = applyDayFlow(20_000, { debits: -50_000, credits: 100_000 });

        expect(low).toBe(-30_000);
        expect(close).toBe(70_000);
    });

    test('closing balance does not depend on the ordering', () => {
        const flow = { debits: -33_333, credits: 77_777 };
        const { close } = applyDayFlow(12_345, flow);

        expect(close).toBe(12_345 + flow.debits + flow.credits);
    });

    test('a day without movement carries the balance over', () => {
        expect(applyDayFlow(45_000)).toEqual({ low: 45_000, close: 45_000 });
    });
});

describe('detectClosingCrossing', () => {
    test('flags the day the balance turns negative', () => {
        expect(detectClosingCrossing(-500, 1_000, 0)).toBe('danger');
    });

    test('does not repeat the alert while the balance stays negative', () => {
        expect(detectClosingCrossing(-800, -500, 0)).toBeNull();
    });

    test('flags the day the balance drops under the safety threshold', () => {
        expect(detectClosingCrossing(8_000, 12_000, 10_000)).toBe('warning');
    });

    test('ignores the threshold when it is disabled', () => {
        expect(detectClosingCrossing(8_000, 12_000, 0)).toBeNull();
    });
});

describe('detectIntradayRisk', () => {
    test('flags a dip below zero rescued by the same-day income', () => {
        expect(detectIntradayRisk(-30_000, 70_000, 0)).toBe('danger');
    });

    test('flags a dip under the threshold rescued by the same-day income', () => {
        expect(detectIntradayRisk(5_000, 70_000, 10_000)).toBe('warning');
    });

    test('stays silent when the day never dips', () => {
        expect(detectIntradayRisk(70_000, 70_000, 10_000)).toBeNull();
    });

    test('stays silent when the day ends negative, since the closing curve shows it', () => {
        expect(detectIntradayRisk(-30_000, -10_000, 0)).toBeNull();
    });

    test('reports the dip below zero rather than the threshold warning', () => {
        expect(detectIntradayRisk(-1, 70_000, 10_000)).toBe('danger');
    });
});

describe('closing and intraday signals together', () => {
    // 120 € left, 200 € debit then 150 € credit, safety threshold at 100 €.
    // The day closes at 70 € (under the threshold) after dipping to -80 €.
    const alertThreshold = 10_000;
    const { low, close } = applyDayFlow(12_000, { debits: -20_000, credits: 15_000 });

    test('the day ends under the threshold', () => {
        expect(close).toBe(7_000);
        expect(detectClosingCrossing(close, 12_000, alertThreshold)).toBe('warning');
    });

    test('and the dip below zero is still reported', () => {
        expect(low).toBe(-8_000);
        expect(detectIntradayRisk(low, close, alertThreshold)).toBe('danger');
    });
});

describe('highestSeverity', () => {
    test('danger wins over warning', () => {
        expect(highestSeverity([{ severity: 'warning' }, { severity: 'danger' }])).toBe('danger');
    });

    test('no crossing means no severity', () => {
        expect(highestSeverity([])).toBeNull();
    });
});
