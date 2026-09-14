import { format, parseISO, isValid } from 'date-fns';
import { fr } from 'date-fns/locale';

export const formatCurrency = (amount: number, minimumFractionDigits: number = 2): string => {
    try {
        return new Intl.NumberFormat(navigator.language || 'fr-FR', {
            style: 'currency',
            currency: 'EUR',
            minimumFractionDigits,
            maximumFractionDigits: 2
        }).format(amount);
    } catch (e) {
        // Fallback for very old systems
        return amount.toFixed(minimumFractionDigits) + ' €';
    }
};

export const formatDate = (date: string | Date, formatStr: string = 'dd/MM/yyyy'): string => {
    if (!date) return '';
    
    try {
        const d = typeof date === 'string' ? parseISO(date) : date;
        if (!isValid(d)) return String(date);
        
        return format(d, formatStr, { locale: fr });
    } catch (e) {
        // Manual fallback for old WebKit if date-fns fails
        try {
            const d = new Date(date);
            if (isNaN(d.getTime())) return String(date);
            const day = String(d.getDate()).padStart(2, '0');
            const month = String(d.getMonth() + 1).padStart(2, '0');
            const year = d.getFullYear();
            return `${day}/${month}/${year}`;
        } catch (e2) {
            return String(date);
        }
    }
};
