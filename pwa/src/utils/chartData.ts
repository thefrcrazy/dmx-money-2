import { format, isValid } from 'date-fns';
import { fr } from 'date-fns/locale';
import type { Account, Transaction } from '../types';

export const MAX_CHART_DAYS = 1826;
export const MAX_CHART_POINTS = 500_000;

export function parseBankDate(value: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return new Date(NaN);
  const [, year, month, day] = match.map(Number);
  const result = new Date(year, month - 1, day);
  return result.getFullYear() === year && result.getMonth() === month - 1 && result.getDate() === day ? result : new Date(NaN);
}

export function chartRangeError(start: Date, end: Date, accounts: number): string | null {
  if (!isValid(start) || !isValid(end) || end < start) return 'Choisissez une période valide, avec une date de fin après le début.';
  const calendarDay = (date: Date) => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round((calendarDay(end) - calendarDay(start)) / 86400000) + 1;
  if (days > MAX_CHART_DAYS) return 'La période du graphique est limitée à cinq ans. Réduisez la période.';
  if (days * Math.max(1, accounts) > MAX_CHART_POINTS) return 'Trop de points à afficher. Réduisez la période ou le nombre de comptes.';
  return null;
}

/** Account IDs remain technical keys; date-only bank records never pass through UTC parsing. */
export function buildBalanceHistory(accounts: Account[], transactions: Transaction[], start: Date, end: Date) {
  if (chartRangeError(start, end, accounts.length)) return [];
  const startKey = format(start, 'yyyy-MM-dd');
  const endKey = format(end, 'yyyy-MM-dd');
  const balances: Record<string, number> = Object.fromEntries(accounts.map(account => [account.id, account.initialBalance]));
  const byDay = new Map<string, Transaction[]>();
  for (const transaction of transactions) {
    if (!Object.prototype.hasOwnProperty.call(balances, transaction.accountId)) continue;
    if (transaction.date < startKey) balances[transaction.accountId] += transaction.type === 'income' ? transaction.amount : -transaction.amount;
    else if (transaction.date <= endKey) {
      const day = byDay.get(transaction.date) ?? [];
      day.push(transaction);
      byDay.set(transaction.date, day);
    }
  }
  const points: { date: string; fullDate: string; balances: Record<string, number> }[] = [];
  for (const day = new Date(start); day <= end; day.setDate(day.getDate() + 1)) {
    for (const transaction of byDay.get(format(day, 'yyyy-MM-dd')) ?? []) {
      balances[transaction.accountId] += transaction.type === 'income' ? transaction.amount : -transaction.amount;
    }
    points.push({ date: format(day, 'dd MMM', { locale: fr }), fullDate: format(day, 'd MMMM yyyy', { locale: fr }), balances: { ...balances } });
  }
  return points;
}
