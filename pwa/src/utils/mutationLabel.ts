import type { OfflineMutation } from '../services/offlineStore';

export function mutationLabel(item: Pick<OfflineMutation, 'path' | 'body'>): string {
    if (item.path.split('/')[2] === 'settings') return 'Paramètres';
    try {
        const value: unknown = JSON.parse(item.body || '{}');
        if (value && typeof value === 'object' && 'description' in value && typeof value.description === 'string') {
            return value.description || 'Modification';
        }
    } catch { /* Recovery remains available even if the body cannot be decoded. */ }
    return 'Modification';
}
