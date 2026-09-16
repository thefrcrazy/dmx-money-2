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
