import React from 'react';
import { X, AlertTriangle, FileDown, GitMerge } from 'lucide-react';
import Button from '../../components/ui/Button';

interface ImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    onImport: (mode: 'replace' | 'merge') => void;
    fileName: string;
}

const ImportModal: React.FC<ImportModalProps> = ({ isOpen, onClose, onImport, fileName }) => {
    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 app-modal-overlay">
            <div className="app-card w-full max-w-md overflow-hidden animate-in fade-in zoom-in duration-200 app-modal-content app-import-modal">
                <div className="p-6 app-modal-body">
                    <div className="flex items-center justify-between mb-6 app-modal-header">
                        <h3 className="text-xl font-bold text-gray-900 dark:text-gray-100 flex items-center gap-2 app-modal-title">
                            <FileDown className="w-6 h-6 text-primary-600 dark:text-primary-400" />
                            Importer des données
                        </h3>
                        <button
                            onClick={onClose}
                            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors app-modal-close-btn"
                        >
                            <X className="w-5 h-5" />
                        </button>
                    </div>

                    <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-100 dark:border-blue-800 rounded-lg p-4 mb-6 app-import-file-info">
                        <p className="text-sm text-blue-800 dark:text-blue-200">
                            Fichier sélectionné : <span className="font-semibold">{fileName}</span>
                        </p>
                    </div>

                    <p className="text-gray-600 dark:text-gray-300 mb-6 app-import-instruction">
                        Comment souhaitez-vous importer ces données ?
                    </p>

                    <div className="space-y-4 app-import-options-container">
                        <button
                            onClick={() => onImport('merge')}
                            className="w-full flex items-start gap-4 p-4 rounded-lg border border-gray-200 dark:border-neutral-700 hover:border-primary-500 dark:hover:border-primary-500 hover:bg-primary-50 dark:hover:bg-primary-900/20 transition-all group text-left app-import-option app-import-option-merge"
                        >
                            <div className="p-2 bg-green-100 dark:bg-green-900/30 rounded-lg text-green-600 dark:text-green-400 group-hover:bg-green-200 dark:group-hover:bg-green-900/50 transition-colors app-import-option-icon">
                                <GitMerge className="w-5 h-5" />
                            </div>
                            <div>
                                <h4 className="font-semibold text-gray-900 dark:text-gray-100 mb-1 app-option-title">Fusionner</h4>
                                <p className="text-sm text-gray-500 dark:text-gray-400 app-option-description">
                                    Ajouter les nouvelles transactions sans toucher aux existantes.
                                </p>
                            </div>
                        </button>

                        <button
                            onClick={() => onImport('replace')}
                            className="w-full flex items-start gap-4 p-4 rounded-lg border border-gray-200 dark:border-neutral-700 hover:border-red-500 dark:hover:border-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-all group text-left app-import-option app-import-option-replace"
                        >
                            <div className="p-2 bg-red-100 dark:bg-red-900/30 rounded-lg text-red-600 dark:text-red-400 group-hover:bg-red-200 dark:group-hover:bg-red-900/50 transition-colors app-import-option-icon">
                                <AlertTriangle className="w-5 h-5" />
                            </div>
                            <div>
                                <h4 className="font-semibold text-gray-900 dark:text-gray-100 mb-1 app-option-title">Remplacer</h4>
                                <p className="text-sm text-gray-500 dark:text-gray-400 app-option-description">
                                    Supprimer toutes les transactions de ce compte et les remplacer.
                                </p>
                            </div>
                        </button>
                    </div>
                </div>

                <div className="bg-gray-50 dark:bg-neutral-700/50 px-6 py-4 flex justify-end app-modal-footer">
                    <Button variant="ghost" onClick={onClose}>
                        Annuler
                    </Button>
                </div>
            </div>
        </div>
    );
};

export default ImportModal;
