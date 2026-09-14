import React, { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import Button from './Button';

interface FormPopupProps {
    isOpen: boolean;
    onClose: () => void;
    children: React.ReactNode;
    title?: string;
    onSubmit?: (e: React.FormEvent) => void;
    submitLabel?: string;
    isSubmitting?: boolean;
    maxWidth?: 'sm' | 'md' | 'lg' | 'xl' | '2xl';
}

const EXIT_ANIMATION_MS = 280;

const FormPopup: React.FC<FormPopupProps> = ({
    isOpen,
    onClose,
    children,
    title,
    onSubmit,
    submitLabel = 'Enregistrer',
    isSubmitting = false,
    maxWidth = 'lg'
}) => {
    const [shouldRender, setShouldRender] = useState(isOpen);
    const [isClosing, setIsClosing] = useState(false);

    useEffect(() => {
        if (isOpen) {
            setShouldRender(true);
            setIsClosing(false);
            return;
        }

        if (!shouldRender) return;

        setIsClosing(true);
        const timeout = window.setTimeout(() => {
            setShouldRender(false);
            setIsClosing(false);
        }, EXIT_ANIMATION_MS);

        return () => window.clearTimeout(timeout);
    }, [isOpen, shouldRender]);

    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === 'Escape') onClose();
        };
        if (shouldRender && !isClosing) window.addEventListener('keydown', handleEscape);
        return () => window.removeEventListener('keydown', handleEscape);
    }, [isClosing, onClose, shouldRender]);

    useEffect(() => {
        if (!shouldRender) return;

        const previousBodyOverflow = document.body.style.overflow;
        const previousDocumentOverflow = document.documentElement.style.overflow;
        document.body.style.overflow = 'hidden';
        document.documentElement.style.overflow = 'hidden';

        return () => {
            document.body.style.overflow = previousBodyOverflow;
            document.documentElement.style.overflow = previousDocumentOverflow;
        };
    }, [shouldRender]);

    if (!shouldRender) return null;

    const maxWidthClasses = {
        sm: 'max-w-sm',
        md: 'max-w-md',
        lg: 'max-w-lg',
        xl: 'max-w-xl',
        '2xl': 'max-w-2xl'
    };

    const overlayAnimationClass = isClosing
        ? 'app-form-popup-overlay--exit'
        : 'app-form-popup-overlay--enter';
    const contentAnimationClass = isClosing
        ? 'app-form-popup-content--exit'
        : 'app-form-popup-content--enter';

    const Content = (
        <div className="space-y-4 app-form-content">
            {children}
            {onSubmit && (
                <div className="flex justify-end gap-3 pt-4 border-t border-black/[0.05] dark:border-white/10 app-modal-footer">
                    <Button
                        type="button"
                        variant="secondary"
                        onClick={onClose}
                        disabled={isSubmitting}
                    >
                        Annuler
                    </Button>
                    <Button
                        type="submit"
                        isLoading={isSubmitting}
                    >
                        {submitLabel}
                    </Button>
                </div>
            )}
        </div>
    );

    return createPortal(
        <div
            className={`fixed inset-0 z-[90] flex h-[100dvh] w-screen items-center justify-center p-4 bg-black/50 backdrop-blur-sm app-modal-overlay app-form-popup-overlay ${overlayAnimationClass}`}
        >
            <div
                className={`app-card w-full ${maxWidthClasses[maxWidth]} max-h-[calc(100dvh-2rem)] flex flex-col overflow-hidden app-modal-content app-form-popup-content ${contentAnimationClass}`}
                onClick={(e) => e.stopPropagation()}
            >
                {/* Drag handle visuel mobile */}
                <div className="flex justify-center py-2.5 md:hidden cursor-pointer" onClick={onClose}>
                    <div className="w-12 h-1.5 bg-gray-300 dark:bg-neutral-800 rounded-full" />
                </div>

                {title && (
                    <div className="flex items-center justify-between p-4 border-b border-black/[0.05] dark:border-white/10 app-modal-header">
                        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 app-modal-title">
                            {title}
                        </h3>
                        <button
                            onClick={onClose}
                            aria-label="Fermer"
                            className="text-gray-400 hover:text-gray-500 dark:hover:text-gray-300 transition-colors app-modal-close-btn"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>
                )}

                <div className={onSubmit ? "p-4 app-modal-body" : "app-modal-body"}>
                    {onSubmit ? (
                        <form onSubmit={onSubmit}>
                            {Content}
                        </form>
                    ) : (
                        Content
                    )}
                </div>
            </div>
        </div>,
        document.body
    );
};

export default FormPopup;
