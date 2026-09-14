export type CrossingSeverity = 'warning' | 'danger';

export interface DayFlow {
    /** Sum of the day's outgoing cents, always negative or zero. */
    debits: number;
    /** Sum of the day's incoming cents, always positive or zero. */
    credits: number;
}

export interface DayBalances {
    /** Balance once the day's withdrawals are settled, before any income. */
    low: number;
    /** Balance at the end of the day. */
    close: number;
}

export const emptyDayFlow = (): DayFlow => ({ debits: 0, credits: 0 });

export const addDayFlow = (flow: DayFlow, amountCents: number): DayFlow => {
    if (amountCents < 0) flow.debits += amountCents;
    else flow.credits += amountCents;
    return flow;
};

/**
 * Banks settle withdrawals before they credit income, so the lowest point of the
 * day is what decides whether an overdraft fee is charged. The closing balance is
 * unchanged by the ordering; only `low` is new information.
 */
export const applyDayFlow = (openingCents: number, flow?: DayFlow): DayBalances => {
    const low = openingCents + (flow?.debits || 0);
    return { low, close: low + (flow?.credits || 0) };
};

/**
 * Crossing of the end-of-day curve: the balance actually finishes the day below
 * zero (danger) or below the user's safety threshold (warning).
 */
export const detectClosingCrossing = (
    value: number,
    previousValue: number,
    alertThreshold: number,
): CrossingSeverity | null => {
    if (value < 0 && previousValue >= 0) return 'danger';
    if (alertThreshold > 0 && value < alertThreshold && previousValue >= alertThreshold) return 'warning';
    return null;
};

/**
 * The day ends safely only because income lands after the withdrawals. Nothing on
 * the end-of-day curve shows it, which is exactly why it is worth flagging: the
 * bank still sees the dip.
 */
export const detectIntradayRisk = (
    low: number,
    value: number,
    alertThreshold: number,
): CrossingSeverity | null => {
    if (low < 0 && value >= 0) return 'danger';
    if (alertThreshold > 0 && low < alertThreshold && value >= alertThreshold) return 'warning';
    return null;
};

export const highestSeverity = (
    entries: Array<{ severity: CrossingSeverity }>,
): CrossingSeverity | null => {
    if (entries.length === 0) return null;
    return entries.some(entry => entry.severity === 'danger') ? 'danger' : 'warning';
};
