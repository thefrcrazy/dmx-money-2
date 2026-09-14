import React, { useEffect } from 'react';
import { TriangleAlert } from 'lucide-react';
import Button from './Button';
import FormPopup from './FormPopup';
import { isMobileCompanion } from '../../utils/runtime';

interface ConfirmModalProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm: () => void;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    isDangerous?: boolean;
}

const ConfirmModal: React.FC<ConfirmModalProps> = ({
    isOpen,
    onClose,
    onConfirm,
    title,
    message,
    confirmLabel = 'Confirmer',
    cancelLabel = 'Annuler',
    isDangerous = false
}) => {
    const mobileMode = isMobileCompanion();

    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        if (isOpen && !mobileMode) window.addEventListener('keydown', handleEscape);
        return () => window.removeEventListener('keydown', handleEscape);
    }, [isOpen, mobileMode, onClose]);

    const handleConfirm = () => {
        onConfirm();
        onClose();
    };

    if (mobileMode) {
        return (
            <FormPopup
                isOpen={isOpen}
                onClose={onClose}
                title={title}
                maxWidth="md"
            >
                <div className="space-y-6 px-2 pb-1">
                    <div className="flex items-start gap-4">
                        {isDangerous && (
                            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-red-50 dark:bg-red-500/10">
                                <TriangleAlert className="h-6 w-6 text-red-600 dark:text-red-400" />
                            </div>
                        )}
                        <p className={`text-[15px] leading-6 text-gray-600 dark:text-gray-300 ${isDangerous ? 'pt-1' : ''}`}>
                            {message}
                        </p>
                    </div>

                    <div className="grid grid-cols-2 gap-3">
                        <Button
                            variant="secondary"
                            fullWidth
                            onClick={onClose}
                        >
                            {cancelLabel}
                        </Button>
                        <Button
                            variant={isDangerous ? 'danger' : 'primary'}
                            fullWidth
                            onClick={handleConfirm}
                        >
                            {confirmLabel}
                        </Button>
                    </div>
                </div>
            </FormPopup>
        );
    }

    if (!isOpen) return null;

    return (
        <div
            className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/20 app-modal-overlay"
        >
            <div
                className="app-card w-full max-w-md animate-in fade-in zoom-in duration-100 app-modal-content app-confirm-modal"
                onClick={(e) => e.stopPropagation()}
            >
                <div className="p-6 space-y-6 app-modal-body">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-200 app-confirm-title">{title}</h3>

                    <div className="flex items-start gap-4 app-confirm-message-container">
                        {isDangerous && (
                            <div className="p-2 bg-red-100 rounded-full flex-shrink-0 app-confirm-icon-wrapper">
                                <TriangleAlert className="w-6 h-6 text-red-600" />
                            </div>
                        )}
                        <div className={`text-gray-600 dark:text-gray-300 ${isDangerous ? 'pt-1' : ''} app-confirm-message`}>
                            {message}
                        </div>
                    </div>

                    <div className="flex justify-end gap-3 app-modal-footer">
                        <Button
                            variant="secondary"
                            onClick={onClose}
                        >
                            {cancelLabel}
                        </Button>
                        <Button
                            variant={isDangerous ? 'danger' : 'primary'}
                            onClick={handleConfirm}
                        >
                            {confirmLabel}
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    );
};

export default ConfirmModal;
