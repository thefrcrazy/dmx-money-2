import React, { useState, useEffect, useMemo } from 'react';
import { X, Upload, ArrowRight, Check, AlertTriangle, FileText, Settings, Database, Tag } from 'lucide-react';
import Button from '../../components/ui/Button';
import SearchableSelect from '../../components/ui/SearchableSelect';
import { useBank } from '../../context/BankContext';
import { useSettings } from '../../context/SettingsContext';
import { useToast } from '../../context/ToastContext';
import { parseBankAmount, parseBankDate, parseDelimitedRows } from '../../utils/importParsers';

interface CsvImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: { name: string; content: string } | null;
    onImport: (transactions: any[], accountId: string) => Promise<void>;
}

type Step = 'preview' | 'account' | 'categories' | 'confirm';

interface ColumnMapping {
    date: number;
    amount: number;
    description: number;
    category: number; // Optional in CSV
}

const CsvImportModal: React.FC<CsvImportModalProps> = ({ isOpen, onClose, file, onImport }) => {
    const { accounts, categories, addAccount, addCategory } = useBank();
    const { settings } = useSettings();
    const { showToast } = useToast();

    const [currentStep, setCurrentStep] = useState<Step>('preview');
    const [separator, setSeparator] = useState<string>(';');
    const [hasHeader, setHasHeader] = useState(false);
    const [mapping, setMapping] = useState<ColumnMapping>({ date: 0, amount: 1, description: 3, category: -1 });
    const [selectedAccountId, setSelectedAccountId] = useState<string>('');
    const [newAccountName, setNewAccountName] = useState('');
    const [newAccountType, setNewAccountType] = useState('Courant');
    const [finalBalance, setFinalBalance] = useState('');
    const [categoryMapping, setCategoryMapping] = useState<Record<string, string>>({});
    const [isImporting, setIsImporting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // Reset state when file changes or modal opens
    useEffect(() => {
        if (isOpen && file) {
            setCurrentStep('preview');
            // Auto-detect separator
            const firstLine = file.content.split('\n')[0];
            if (firstLine.includes(';')) setSeparator(';');
            else if (firstLine.includes(',')) setSeparator(',');

            // Reset other states
            setSelectedAccountId('');
            setNewAccountName('');
            setNewAccountType('Courant');
            setFinalBalance('');
            setCategoryMapping({});
            setError(null);
        }
    }, [isOpen, file]);

    const parsedData = useMemo(() => {
        if (!file) return [];
        try {
            const rows = parseDelimitedRows(file.content, separator, hasHeader);
            if (rows.length === 0) throw new Error(hasHeader ? "Le fichier ne contient que l'en-tête." : "Le fichier est vide.");

            return rows;
        } catch (e: any) {
            console.error("CSV Parsing error:", e);
            setError(e.message || "Erreur lors de la lecture du fichier CSV");
            return [];
        }
    }, [file, separator, hasHeader]);

    const previewData = useMemo(() => parsedData.slice(0, 5), [parsedData]);

    const maxColumns = useMemo(() => {
        if (previewData.length === 0) return 0;
        return Math.max(...previewData.map(row => row.length));
    }, [previewData]);

    const uniqueCsvCategories = useMemo(() => {
        if (mapping.category === -1) return [];
        const cats = new Set<string>();
        parsedData.forEach(row => {
            if (row[mapping.category]) cats.add(row[mapping.category]);
        });
        return Array.from(cats).sort();
    }, [parsedData, mapping.category]);

    const handleNext = () => {
        if (currentStep === 'preview') {
            if (parsedData.length === 0) {
                showToast("Aucune donnée à importer", "error");
                return;
            }
            if (mapping.date === -1 || mapping.amount === -1) {
                showToast("Vous devez assigner au moins la Date et le Montant", "error");
                return;
            }
            setCurrentStep('account');
        }
        else if (currentStep === 'account') {
            if (mapping.category !== -1 && uniqueCsvCategories.length > 0) {
                // Pre-fill mapping with exact matches
                const newMapping = { ...categoryMapping };
                uniqueCsvCategories.forEach(csvCat => {
                    const match = categories.find(c => c.name.toLowerCase() === csvCat.toLowerCase());
                    if (match) newMapping[csvCat] = match.id;
                    else newMapping[csvCat] = 'new'; // Default to create new
                });
                setCategoryMapping(newMapping);
                setCurrentStep('categories');
            } else {
                setCurrentStep('confirm');
            }
        }
        else if (currentStep === 'categories') setCurrentStep('confirm');
    };

    const handleBack = () => {
        if (currentStep === 'account') setCurrentStep('preview');
        else if (currentStep === 'categories') setCurrentStep('account');
        else if (currentStep === 'confirm') {
            if (mapping.category !== -1 && uniqueCsvCategories.length > 0) setCurrentStep('categories');
            else setCurrentStep('account');
        }
    };

    const handleFinalImport = async () => {
        setIsImporting(true);
        setError(null);
        try {
            // 1. Parse transactions
            let processedTransactions = parsedData.map(row => {
                const amount = parseBankAmount(row[mapping.amount]);
                const date = parseBankDate(row[mapping.date]);
                const description = row[mapping.description] || 'Import CSV';
                const rawCategory = mapping.category !== -1 ? row[mapping.category] : undefined;

                return {
                    rawCategory: rawCategory,
                    date,
                    amount: Math.abs(amount),
                    type: amount >= 0 ? 'income' : 'expense',
                    description,
                    checked: true
                };
            });

            // 2. Handle Account
            let targetAccountId = selectedAccountId;
            if (selectedAccountId === 'new') {
                if (!newAccountName) throw new Error("Nom du compte requis");

                let initialBalance = 0;
                if (finalBalance) {
                    const netChange = processedTransactions.reduce((sum, tx) => {
                        return sum + (tx.type === 'income' ? tx.amount : -tx.amount);
                    }, 0);

                    const final = parseFloat(finalBalance.replace(',', '.'));
                    if (!isNaN(final)) {
                        initialBalance = final - netChange;
                    }
                }

                targetAccountId = await addAccount({
                    name: newAccountName,
                    type: newAccountType,
                    initialBalance: initialBalance,
                    icon: 'Wallet',
                    color: settings.primaryColor === 'default' ? '#3b82f6' : settings.primaryColor
                });
            }

            // 3. Categories
            const finalCategoryMapping = { ...categoryMapping };
            for (const csvCat of Object.keys(categoryMapping)) {
                if (categoryMapping[csvCat] === 'new') {
                    const newCatId = await addCategory({
                        name: csvCat,
                        icon: 'Tag',
                        color: '#9ca3af'
                    });
                    finalCategoryMapping[csvCat] = newCatId;
                }
            }

            // 4. Finalize
            const transactionsToImport = processedTransactions.map(tx => {
                let categoryId = categories[0]?.id || 'uncategorized';
                if (mapping.category !== -1 && tx.rawCategory) {
                    categoryId = finalCategoryMapping[tx.rawCategory] || categoryId;
                }

                return {
                    date: tx.date,
                    amount: tx.amount,
                    type: tx.type as 'income' | 'expense',
                    description: tx.description,
                    category: categoryId,
                    accountId: targetAccountId,
                    checked: tx.checked
                };
            });

            await onImport(transactionsToImport, targetAccountId);
            showToast(`${transactionsToImport.length} transactions importées avec succès`, "success");
            onClose();
        } catch (e: any) {
            console.error("Import error:", e);
            setError(e.message || "Erreur lors de l'importation");
            showToast(e.message || "Erreur lors de l'importation", "error");
        } finally {
            setIsImporting(false);
        }
    };

    if (!isOpen || !file) return null;

    const renderStepContent = () => {
        if (error && currentStep === 'preview') {
            return (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                    <div className="w-16 h-16 rounded-full bg-red-50 dark:bg-red-900/20 flex items-center justify-center mb-4">
                        <AlertTriangle className="w-8 h-8 text-red-600 dark:text-red-400" />
                    </div>
                    <h4 className="text-lg font-bold text-gray-900 dark:text-gray-100 mb-2">Erreur de lecture</h4>
                    <p className="text-gray-500 dark:text-gray-400 max-w-md mb-6">{error}</p>
                    <Button variant="secondary" onClick={onClose}>Fermer</Button>
                </div>
            );
        }

        switch (currentStep) {
            case 'preview':
                return (
                    <div className="space-y-6">
                        <div className="grid grid-cols-2 gap-4">
                            <div>
                                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Séparateur</label>
                                <div className="flex rounded-lg p-1 app-input bg-gray-100 dark:bg-[#121212] border-none shadow-none">
                                    <button
                                        onClick={() => setSeparator(';')}
                                        className={`flex-1 py-1.5 text-sm font-medium rounded-md transition-all ${separator === ';' ? 'bg-white dark:bg-neutral-700 shadow-sm' : 'text-gray-500'}`}
                                    >
                                        Point-virgule (;)
                                    </button>
                                    <button
                                        onClick={() => setSeparator(',')}
                                        className={`flex-1 py-1.5 text-sm font-medium rounded-md transition-all ${separator === ',' ? 'bg-white dark:bg-neutral-700 shadow-sm' : 'text-gray-500'}`}
                                    >
                                        Virgule (,)
                                    </button>
                                </div>
                            </div>
                            <div className="flex items-end pb-2">
                                <label className="flex items-center gap-2 cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={hasHeader}
                                        onChange={e => setHasHeader(e.target.checked)}
                                        className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                    />
                                    <span className="text-sm text-gray-700 dark:text-gray-300">La première ligne est un en-tête</span>
                                </label>
                            </div>
                        </div>

                        <div className="border border-gray-200 dark:border-neutral-700 rounded-lg overflow-hidden">
                            <div className="overflow-x-auto">
                                <table className="w-full text-sm text-left">
                                    <thead className="bg-gray-50 dark:bg-[#121212] text-xs uppercase text-gray-500">
                                        <tr>
                                            {Array.from({ length: maxColumns }).map((_, i) => (
                                                <th key={i} className="px-4 py-2 min-w-[150px]">
                                                    <select
                                                        value={Object.entries(mapping).find(([_, col]) => col === i)?.[0] || ''}
                                                        onChange={(e) => {
                                                            const val = e.target.value;
                                                            if (val) {
                                                                setMapping(prev => ({ ...prev, [val]: i }));
                                                            } else {
                                                                // Clear mapping for this column if needed
                                                                // Find key that has this value and reset it? 
                                                                // Actually the select value logic handles display, 
                                                                // but to clear we'd need to find which key maps to 'i' and set it to -1
                                                                const key = Object.entries(mapping).find(([_, col]) => col === i)?.[0];
                                                                if (key) {
                                                                    setMapping(prev => ({ ...prev, [key]: -1 }));
                                                                }
                                                            }
                                                        }}
                                                        className="w-full text-xs p-1 border-none bg-transparent focus:ring-0 font-bold text-primary-600"
                                                    >
                                                        <option value="">Ignorer</option>
                                                        <option value="date">Date</option>
                                                        <option value="amount">Montant</option>
                                                        <option value="description">Description</option>
                                                        <option value="category">Catégorie</option>
                                                    </select>
                                                </th>
                                            ))}
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
                                        {previewData.map((row, i) => (
                                            <tr key={i}>
                                                {Array.from({ length: maxColumns }).map((_, j) => (
                                                    <td key={j} className={`px-4 py-2 truncate max-w-[200px] ${Object.values(mapping).includes(j) ? 'bg-primary-50/30 dark:bg-primary-900/10' : ''
                                                        }`}>
                                                        {row[j] || ''}
                                                    </td>
                                                ))}
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                        <p className="text-xs text-gray-500">Assignez les colonnes en utilisant les listes déroulantes ci-dessus.</p>
                    </div>
                );

            case 'account':
                return (
                    <div className="space-y-6">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                Vers quel compte importer ?
                            </label>
                            <div className="space-y-3">
                                {accounts.map(acc => (
                                    <label key={acc.id} className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-all ${selectedAccountId === acc.id
                                        ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                                        : 'border-gray-200 dark:border-neutral-700 hover:border-gray-300'
                                        }`}>
                                        <input
                                            type="radio"
                                            name="account"
                                            value={acc.id}
                                            checked={selectedAccountId === acc.id}
                                            onChange={() => setSelectedAccountId(acc.id)}
                                            className="text-primary-600 focus:ring-primary-500"
                                        />
                                        <div className="w-8 h-8 rounded-full flex items-center justify-center text-white" style={{ backgroundColor: acc.color }}>
                                            <Database className="w-4 h-4" />
                                        </div>
                                        <span className="font-medium text-gray-900 dark:text-gray-200">{acc.name}</span>
                                    </label>
                                ))}
                                <label className={`flex items-center gap-3 p-3 rounded-lg border cursor-pointer transition-all ${selectedAccountId === 'new'
                                    ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                                    : 'border-gray-200 dark:border-neutral-700 hover:border-gray-300'
                                    }`}>
                                    <input
                                        type="radio"
                                        name="account"
                                        value="new"
                                        checked={selectedAccountId === 'new'}
                                        onChange={() => setSelectedAccountId('new')}
                                        className="text-primary-600 focus:ring-primary-500"
                                    />
                                    <div className="w-8 h-8 rounded-full bg-gray-200 dark:bg-neutral-700 flex items-center justify-center text-gray-500">
                                        <Upload className="w-4 h-4" />
                                    </div>
                                    <div className="flex-1">
                                        <span className="font-medium text-gray-900 dark:text-gray-200 block">Nouveau compte</span>
                                        {selectedAccountId === 'new' && (
                                            <div className="mt-2 space-y-2">
                                                <input
                                                    type="text"
                                                    placeholder="Nom du compte"
                                                    value={newAccountName}
                                                    onChange={e => setNewAccountName(e.target.value)}
                                                    className="w-full px-3 py-1.5 text-sm border rounded-md dark:bg-[#121212] dark:border-neutral-600"
                                                    autoFocus
                                                />
                                                <SearchableSelect
                                                    value={newAccountType}
                                                    onChange={(value) => setNewAccountType(value)}
                                                    options={[
                                                        { id: 'Courant', label: 'Courant', icon: 'Wallet', color: '#3b82f6' },
                                                        { id: 'Épargne', label: 'Épargne', icon: 'PiggyBank', color: '#10b981' },
                                                        { id: 'Espèces', label: 'Espèces', icon: 'Banknote', color: '#f59e0b' },
                                                        { id: 'Investissement', label: 'Investissement', icon: 'TrendingUp', color: '#8b5cf6' }
                                                    ]}
                                                    placeholder="Sélectionner un type"
                                                />
                                                <div className="flex items-center gap-2">
                                                    <input
                                                        type="number"
                                                        step="0.01"
                                                        placeholder="Solde final (optionnel)"
                                                        value={finalBalance}
                                                        onChange={e => setFinalBalance(e.target.value)}
                                                        className="flex-1 px-3 py-1.5 text-sm border rounded-md dark:bg-[#121212] dark:border-neutral-600"
                                                    />
                                                    <span className="text-xs text-gray-500" title="Permet de calculer le solde initial automatiquement">
                                                        €
                                                    </span>
                                                </div>
                                                <p className="text-xs text-gray-500">
                                                    Saisissez le solde final du relevé pour calculer automatiquement le solde initial.
                                                </p>
                                            </div>
                                        )}
                                    </div>
                                </label>
                            </div>
                        </div>
                    </div>
                );

            case 'categories':
                return (
                    <div className="space-y-4">
                        <p className="text-sm text-gray-600 dark:text-gray-400">
                            Associez les catégories du fichier CSV à vos catégories existantes.
                        </p>
                        <div className="space-y-2">
                            {uniqueCsvCategories.map(csvCat => (
                                <div key={csvCat} className="flex items-center gap-4 p-3 bg-gray-50 dark:bg-neutral-800 rounded-lg">
                                    <div className="flex-1 font-medium text-sm text-gray-900 dark:text-gray-200 truncate" title={csvCat}>
                                        {csvCat}
                                    </div>
                                    <ArrowRight className="w-4 h-4 text-gray-400" />
                                    <div className="flex-1 min-w-[200px]">
                                        <SearchableSelect
                                            value={categoryMapping[csvCat] || 'new'}
                                            onChange={(value) => setCategoryMapping(prev => ({ ...prev, [csvCat]: value }))}
                                            options={[
                                                { id: 'new', label: `+ Créer "${csvCat}"`, icon: 'Plus' },
                                                ...categories.map(c => ({ id: c.id, label: c.name, icon: c.icon, color: c.color }))
                                            ]}
                                            placeholder="Sélectionner une catégorie"
                                        />
                                    </div>
                                </div>
                            ))}
                        </div>
                    </div>
                );

            case 'confirm':
                return (
                    <div className="text-center space-y-6 py-4">
                        <div className="w-16 h-16 bg-green-100 dark:bg-green-900/30 rounded-full flex items-center justify-center mx-auto text-green-600 dark:text-green-400">
                            <Check className="w-8 h-8" />
                        </div>
                        <div>
                            <h3 className="text-xl font-bold text-gray-900 dark:text-gray-100 mb-2">Prêt à importer</h3>
                            <p className="text-gray-600 dark:text-gray-400">
                                <span className="font-bold text-gray-900 dark:text-gray-200">{parsedData.length}</span> transactions seront importées dans le compte <span className="font-bold text-gray-900 dark:text-gray-200">{selectedAccountId === 'new' ? newAccountName : accounts.find(a => a.id === selectedAccountId)?.name}</span>.
                            </p>
                        </div>
                        {mapping.category === -1 && (
                            <div className="bg-yellow-50 dark:bg-yellow-900/20 p-4 rounded-lg flex items-start gap-3 text-left">
                                <AlertTriangle className="w-5 h-5 text-yellow-600 dark:text-yellow-400 shrink-0 mt-0.5" />
                                <p className="text-sm text-yellow-700 dark:text-yellow-300">
                                    Aucune colonne catégorie n'a été sélectionnée. Toutes les transactions seront marquées comme "Non catégorisé".
                                </p>
                            </div>
                        )}
                    </div>
                );
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 app-modal-overlay">
            <div className="app-card w-full max-w-2xl flex flex-col max-h-[90vh] animate-in fade-in zoom-in duration-200 app-modal-content app-import-wizard-modal">
                {/* Header */}
                <div className="p-6 border-b border-gray-100 dark:border-neutral-800 flex items-center justify-between app-modal-header">
                    <div className="flex items-center gap-3">
                        <div className="p-2 bg-primary-100 dark:bg-primary-900/30 rounded-lg text-primary-600 dark:text-primary-400 app-modal-icon-container">
                            <FileText className="w-5 h-5" />
                        </div>
                        <div>
                            <h3 className="text-lg font-bold text-gray-900 dark:text-gray-100 app-modal-title">Assistant d'import CSV</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400 app-modal-subtitle">{file.name}</p>
                        </div>
                    </div>
                    <button onClick={onClose} className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 app-modal-close-btn">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Steps Indicator */}
                <div className="px-6 py-4 app-subtle-bg border-b border-gray-100 dark:border-neutral-800 flex justify-between app-wizard-steps-container">
                    {[
                        { id: 'preview', label: 'Colonnes', icon: Settings },
                        { id: 'account', label: 'Compte', icon: Database },
                        { id: 'categories', label: 'Catégories', icon: Tag },
                        { id: 'confirm', label: 'Confirmation', icon: Check }
                    ].map((step, idx) => {
                        const isActive = step.id === currentStep;
                        const isPast = ['preview', 'account', 'categories', 'confirm'].indexOf(currentStep) > idx;

                        // Skip categories step in indicator if skipped in flow? No, keep it for consistency but maybe disable
                        if (step.id === 'categories' && mapping.category === -1 && currentStep === 'confirm') {
                            // Visual indication that it was skipped?
                        }

                        return (
                            <div key={step.id} className={`flex items-center gap-2 app-wizard-step ${isActive ? 'text-primary-600 dark:text-primary-400 app-wizard-step-active' : isPast ? 'text-green-600 dark:text-green-400 app-wizard-step-completed' : 'text-gray-400'}`}>
                                <div className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold border ${isActive ? 'border-primary-600 bg-primary-50 dark:bg-primary-900/20' :
                                    isPast ? 'border-green-600 bg-green-50 dark:bg-green-900/20' :
                                        'border-gray-300 dark:border-neutral-600'
                                    } app-wizard-step-bubble`}>
                                    {isPast ? <Check className="w-3 h-3" /> : idx + 1}
                                </div>
                                <span className="text-sm font-medium hidden sm:block app-wizard-step-label">{step.label}</span>
                            </div>
                        );
                    })}
                </div>

                {/* Content */}
                <div className={`p-6 flex-1 ${currentStep === 'preview' ? 'overflow-y-auto' : 'overflow-visible'} app-modal-body`}>
                    {renderStepContent()}
                </div>

                {/* Footer */}
                <div className="p-6 border-t border-gray-100 dark:border-neutral-800 flex justify-between app-subtle-bg app-modal-footer">
                    <Button
                        variant="ghost"
                        onClick={currentStep === 'preview' ? onClose : handleBack}
                        disabled={isImporting}
                    >
                        {currentStep === 'preview' ? 'Annuler' : 'Retour'}
                    </Button>

                    {currentStep === 'confirm' ? (
                        <Button
                            onClick={handleFinalImport}
                            isLoading={isImporting}
                            className="bg-green-600 hover:bg-green-700 text-white"
                        >
                            Importer maintenant
                        </Button>
                    ) : (
                        <Button
                            onClick={handleNext}
                            isLoading={isImporting}
                            disabled={
                                (currentStep === 'account' && !selectedAccountId) ||
                                (currentStep === 'account' && selectedAccountId === 'new' && !newAccountName)
                            }
                        >
                            Suivant
                        </Button>
                    )}
                </div>
            </div>
        </div>
    );
};

export default CsvImportModal;
