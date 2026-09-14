import React, { useState } from 'react';
import { CheckCircle, AlertCircle, ChevronDown, ChevronUp, Info } from 'lucide-react';
import Button from './Button';

interface AlertModalProps {
    isOpen: boolean;
    onClose: () => void;
    title: string;
    message: string;
    type?: 'success' | 'error';
    technicalDetails?: string;
}

const AlertModal: React.FC<AlertModalProps> = ({
    isOpen,
    onClose,
    title,
    message,
    type = 'success',
    technicalDetails
}) => {
    const [showDetails, setShowDetails] = useState(false);

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 animate-in fade-in duration-200 app-modal-overlay">
            <div className="app-card w-full max-w-md overflow-hidden animate-in zoom-in-95 duration-200 app-modal-content app-alert-modal">
                <div className="p-6 app-modal-body">
                    <div className="text-center">
                        <div className={`mx-auto w-14 h-14 rounded-full flex items-center justify-center mb-4 app-alert-icon-container ${type === 'success'
                            ? 'bg-green-100 text-green-600 dark:bg-green-900/30 dark:text-green-400'
                            : 'bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400'
                            }`}>
                            {type === 'success' ? (
                                <CheckCircle className="w-8 h-8 app-alert-icon-success" />
                            ) : (
                                <AlertCircle className="w-8 h-8 app-alert-icon-error" />
                            )}
                        </div>

                        <h3 className="text-xl font-bold text-gray-900 dark:text-gray-100 mb-2 app-alert-title">
                            {title}
                        </h3>

                        <p className="text-gray-600 dark:text-gray-300 mb-4 app-alert-message">
                            {message}
                        </p>
                    </div>

                    {type === 'error' && (
                        <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-100 dark:border-blue-800/50 rounded-lg p-4 mb-6">
                            <div className="flex gap-3">
                                <Info className="w-5 h-5 text-blue-500 shrink-0 mt-0.5" />
                                <div className="text-left">
                                    <h4 className="text-sm font-semibold text-blue-900 dark:text-blue-300 mb-1">
                                        Que s'est-il passé ?
                                    </h4>
                                    <p className="text-xs text-blue-800 dark:text-blue-400 leading-relaxed">
                                        Une erreur est survenue lors de l'exécution de l'action demandée. Cela peut être dû à un problème de connexion avec la base de données, à un format de donnée invalide ou à une erreur système imprévue.
                                    </p>
                                </div>
                            </div>
                        </div>
                    )}

                    {technicalDetails && (
                        <div className="mb-6 border border-black/[0.05] dark:border-white/10 rounded-lg overflow-hidden">
                            <button
                                onClick={() => setShowDetails(!showDetails)}
                                className="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-800/50 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                            >
                                <span>Détails techniques</span>
                                {showDetails ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                            </button>
                            {showDetails && (
                                <div className="p-3 bg-gray-900 text-gray-300 font-mono text-xs overflow-x-auto max-h-40">
                                    <pre className="whitespace-pre-wrap break-all">{technicalDetails}</pre>
                                </div>
                            )}
                        </div>
                    )}

                    <Button
                        onClick={onClose}
                        className="w-full justify-center app-alert-button"
                        variant={type === 'success' ? 'primary' : 'danger'}
                    >
                        Compris
                    </Button>
                </div>
            </div>
        </div>
    );
};

export default AlertModal;
