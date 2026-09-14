import React, { useEffect } from 'react';
import { X } from 'lucide-react';

interface ModalProps {
    isOpen: boolean;
    onClose: () => void;
    title: string;
    children: React.ReactNode;
}

const Modal: React.FC<ModalProps> = ({ isOpen, onClose, title, children }) => {
    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        if (isOpen) window.addEventListener('keydown', handleEscape);
        return () => window.removeEventListener('keydown', handleEscape);
    }, [isOpen, onClose]);

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm app-modal-overlay">
            <div className="app-card w-full max-w-2xl animate-in fade-in zoom-in duration-200 app-modal-content">
                <div className="flex items-center justify-between p-4 border-b border-black/[0.05] dark:border-white/10 app-modal-header">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200 app-modal-title">{title}</h3>
                    <button
                        onClick={onClose}
                        className="p-1 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors app-modal-close-btn"
                    >
                        <X className="w-5 h-5" />
                    </button>
                </div>
                <div className="p-4 app-modal-body">
                    {children}
                </div>
            </div>
        </div>
    );
};

export default Modal;
