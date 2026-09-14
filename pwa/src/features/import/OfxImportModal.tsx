import React, { useState, useEffect, useMemo } from 'react';
import { X, Upload, ArrowRight, Check, FileText, Settings, Database, Tag, AlertTriangle } from 'lucide-react';
import Button from '../../components/ui/Button';
import SearchableSelect from '../../components/ui/SearchableSelect';
import { useBank } from '../../context/BankContext';
import { useSettings } from '../../context/SettingsContext';
import { useToast } from '../../context/ToastContext';
import { parseOfxTransactions } from '../../utils/importParsers';

interface OfxImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    file: { name: string; content: string } | null;
    onImport: (transactions: any[], accountId: string) => Promise<void>;
}

type Step = 'preview' | 'account' | 'categories' | 'confirm';

const OfxImportModal: React.FC<OfxImportModalProps> = ({ isOpen, onClose, file, onImport }) => {
    const { accounts, categories, addAccount, addCategory } = useBank();
    const { settings } = useSettings();
    const { showToast } = useToast();

    const [currentStep, setCurrentStep] = useState<Step>('preview');
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
            return parseOfxTransactions(file.content).map(transaction => ({ ...transaction, category: '' }));
        } catch (e: any) {
            console.error("Parsing error:", e);
            setError(e.message || "Erreur lors de la lecture du fichier OFX");
            return [];
        }
    }, [file]);

    const uniqueOfxCategories = useMemo(() => {
        // OFX rarely has categories, but if we extracted some (maybe from MEMO?), we'd list them here.
        // For now, it's likely empty or just based on description heuristics if we added that.
        // Let's keep the logic generic in case we improve parsing later.
        const cats = new Set<string>();
        parsedData.forEach(row => {
            if (row.category) cats.add(row.category);
        });
        return Array.from(cats).sort();
    }, [parsedData]);

    const handleNext = () => {
        if (currentStep === 'preview') {
            if (parsedData.length === 0) {
                showToast("Aucune transaction à importer", "error");
                return;
            }
            setCurrentStep('account');
        }
        else if (currentStep === 'account') {
            if (uniqueOfxCategories.length > 0) {
                // Pre-fill mapping with exact matches
                const newMapping = { ...categoryMapping };
                uniqueOfxCategories.forEach(cat => {
                    const match = categories.find(c => c.name.toLowerCase() === cat.toLowerCase());
                    if (match) newMapping[cat] = match.id;
                    else newMapping[cat] = 'new'; // Default to create new
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
            if (uniqueOfxCategories.length > 0) setCurrentStep('categories');
            else setCurrentStep('account');
        }
    };

    const handleFinalImport = async () => {
        setIsImporting(true);
        setError(null);
        try {
            // 1. Prepare transactions
            let processedTransactions = parsedData.map(tx => ({
                ...tx,
                amount: Math.abs(tx.amount),
                type: tx.amount >= 0 ? 'income' : 'expense',
                checked: true
            }));

            // 2. Handle New Account Logic
            let targetAccountId = selectedAccountId;

            if (selectedAccountId === 'new') {
                if (!newAccountName) throw new Error("Nom du compte requis");

                let initialBalance = 0;

                if (finalBalance) {
                    // Remove first and last logic if applicable, or keep all
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

            // 3. Create new categories
            const finalCategoryMapping = { ...categoryMapping };
            for (const cat of Object.keys(categoryMapping)) {
                if (categoryMapping[cat] === 'new') {
                    const newCatId = await addCategory({
                        name: cat,
                        icon: 'Tag',
                        color: '#9ca3af'
                    });
                    finalCategoryMapping[cat] = newCatId;
                }
            }

            // 4. Finalize transactions
            const transactionsToImport = processedTransactions.map(tx => {
                let categoryId = categories[0]?.id || 'uncategorized';
                if (tx.category) {
                    categoryId = finalCategoryMapping[tx.category] || categoryId;
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
                        <div className="bg-blue-50 dark:bg-blue-900/20 p-4 rounded-lg flex items-start gap-3">
                            <Database className="w-5 h-5 text-blue-600 dark:text-blue-400 shrink-0 mt-0.5" />
                            <div>
                                <h4 className="font-medium text-blue-900 dark:text-blue-100">Aperçu du fichier OFX</h4>
                                <p className="text-sm text-blue-700 dark:text-blue-300 mt-1">
                                    {parsedData.length} transactions trouvées. Vérifiez que les données semblent correctes.
                                </p>
                            </div>
                        </div>

                        <div className="border border-gray-200 dark:border-neutral-700 rounded-lg overflow-hidden">
                            <div className="overflow-x-auto">
                                <table className="w-full text-sm text-left">
                                                                            <thead className="bg-gray-50 dark:bg-[#121212] text-xs uppercase text-gray-500">                                        <tr>
                                            <th className="px-4 py-2">Date</th>
                                            <th className="px-4 py-2">Payee / Description</th>
                                            <th className="px-4 py-2">Catégorie</th>
                                            <th className="px-4 py-2 text-right">Montant</th>
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
                                        {parsedData.slice(0, 10).map((row, i) => (
                                            <tr key={i}>
                                                <td className="px-4 py-2">{row.date}</td>
                                                <td className="px-4 py-2">{row.description}</td>
                                                <td className="px-4 py-2">
                                                    {row.category ? (
                                                        <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 dark:bg-neutral-800 text-gray-800 dark:text-gray-200">
                                                            {row.category}
                                                        </span>
                                                    ) : (
                                                        <span className="text-gray-400 italic">Aucune</span>
                                                    )}
                                                </td>
                                                <td className={`px-4 py-2 text-right font-medium ${row.amount >= 0 ? 'text-green-600' : 'text-red-600'}`}>
                                                    {new Intl.NumberFormat('fr-FR', { style: 'currency', currency: 'EUR' }).format(row.amount)}
                                                </td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                            {parsedData.length > 10 && (
                                <div className="bg-gray-50 dark:bg-neutral-800 p-2 text-center text-xs text-gray-500">
                                    ... et {parsedData.length - 10} autres transactions
                                </div>
                            )}
                        </div>
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
                            Associez les catégories du fichier OFX à vos catégories existantes.
                        </p>
                        <div className="space-y-2">
                            {uniqueOfxCategories.map(cat => (
                                <div key={cat} className="flex items-center gap-4 p-3 bg-gray-50 dark:bg-neutral-800 rounded-lg">
                                    <div className="flex-1 font-medium text-sm text-gray-900 dark:text-gray-200 truncate" title={cat}>
                                        {cat}
                                    </div>
                                    <ArrowRight className="w-4 h-4 text-gray-400" />
                                    <div className="flex-1 min-w-[200px]">
                                        <SearchableSelect
                                            value={categoryMapping[cat] || 'new'}
                                            onChange={(value) => setCategoryMapping(prev => ({ ...prev, [cat]: value }))}
                                            options={[
                                                { id: 'new', label: `+ Créer "${cat}"`, icon: 'Plus' },
                                                ...categories.map(c => ({ id: c.id, label: c.name, icon: c.icon, color: c.color }))
                                            ]}
                                            placeholder="Sélectionner une catégorie"
                                        />
                                    </div>
                                </div>
                            ))}
                            {uniqueOfxCategories.length === 0 && (
                                <div className="text-center py-8 text-gray-500 dark:text-gray-400">
                                    Aucune catégorie trouvée dans le fichier OFX.
                                    <br />
                                    Toutes les transactions seront importées dans la catégorie par défaut.
                                </div>
                            )}
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
                            <h3 className="text-lg font-bold text-gray-900 dark:text-gray-100 app-modal-title">Assistant d'import OFX</h3>
                            <p className="text-xs text-gray-500 dark:text-gray-400 app-modal-subtitle">{file.name}</p>
                        </div>
                    </div>
                    <button onClick={onClose} className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 app-modal-close-btn">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Steps Indicator */}
                <div className="px-6 py-4 bg-gray-50 dark:bg-[#1a1a1a] border-b border-gray-100 dark:border-neutral-800 flex justify-between app-wizard-steps-container">
                    {[
                        { id: 'preview', label: 'Aperçu', icon: Settings },
                        { id: 'account', label: 'Compte', icon: Database },
                        { id: 'categories', label: 'Catégories', icon: Tag },
                        { id: 'confirm', label: 'Confirmation', icon: Check }
                    ].map((step, idx) => {
                        {/* Step icon removed as it was unused */}
                        const isActive = currentStep === step.id;
                        const isPast = ['preview', 'account', 'categories', 'confirm'].indexOf(currentStep) > idx;

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
                <div className="p-6 border-t border-gray-100 dark:border-neutral-800 flex justify-between bg-gray-50 dark:bg-[#1a1a1a] app-modal-footer">
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

export default OfxImportModal;
