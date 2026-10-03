import type { Transaction } from '../types';

/** Mirror the server's atomic transfer update in the UI and offline cache. */
export function applyTransactionUpdate(items: Transaction[], updated: Transaction): Transaction[] {
  return items.map(item => {
    if (item.id === updated.id) return updated;
    if (updated.isTransfer && item.isTransfer && item.id === updated.linkedTransactionId && item.linkedTransactionId === updated.id) {
      return { ...item, amount: updated.amount, date: updated.date, description: updated.description, checked: updated.checked };
    }
    return item;
  });
}

/** A stale form only changes the fields the user edited; the server arbitrates conflicts. */
export function mergeTransactionDisplay(current: Transaction, submitted: Transaction, base: Transaction): Transaction {
  const merged = { ...current };
  for (const key of Object.keys(submitted) as (keyof Transaction)[]) {
    if (JSON.stringify(submitted[key]) !== JSON.stringify(base[key])) Object.assign(merged, { [key]: submitted[key] });
  }
  return merged;
}
