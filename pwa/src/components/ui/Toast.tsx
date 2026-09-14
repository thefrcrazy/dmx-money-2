import React, { useEffect } from 'react';
import { CheckCircle2, AlertCircle, X, Info } from 'lucide-react';

export type ToastType = 'success' | 'error' | 'info';

export interface ToastProps {
    id: string;
    message: string;
    type: ToastType;
    onClose: (id: string) => void;
    duration?: number;
}

const Toast: React.FC<ToastProps> = ({ id, message, type, onClose, duration = 3000 }) => {
    useEffect(() => {
        const timer = setTimeout(() => {
            onClose(id);
        }, duration);

        return () => clearTimeout(timer);
    }, [id, duration, onClose]);

    const icons = {
        success: <CheckCircle2 className="w-5 h-5 text-emerald-500" />,
        error: <AlertCircle className="w-5 h-5 text-red-500" />,
        info: <Info className="w-5 h-5 text-blue-500" />
    };

    const styles = {
        success: "border-emerald-500/20 bg-emerald-50/90 dark:bg-emerald-900/20 text-emerald-900 dark:text-emerald-100",
        error: "border-red-500/20 bg-red-50/90 dark:bg-red-900/20 text-red-900 dark:text-red-100",
        info: "border-blue-500/20 bg-blue-50/90 dark:bg-blue-900/20 text-blue-900 dark:text-blue-100"
    };

    return (
        <div className={`flex items-center gap-3 px-4 py-3 rounded-xl border shadow-lg backdrop-blur-md animate-in slide-in-from-bottom-5 fade-in duration-150 ${styles[type]} min-w-[300px] max-w-md`}>
            <div className="shrink-0">
                {icons[type]}
            </div>
            <p className="text-sm font-medium flex-1">{message}</p>
            <button 
                onClick={() => onClose(id)}
                className="shrink-0 p-1 hover:bg-black/5 dark:hover:bg-white/10 rounded-full transition-colors"
            >
                <X className="w-4 h-4 opacity-50 hover:opacity-100" />
            </button>
        </div>
    );
};

export default Toast;