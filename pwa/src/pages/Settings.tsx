import React, { useState, useEffect } from 'react';
import { Moon, Sun, Monitor, Download, Upload, RefreshCw, ChevronRight, Palette, HardDrive, Info, Smartphone, Copy, Wifi, KeyRound, Lock, CheckCircle2, AlertTriangle, Globe2, Server, ShieldCheck, LogOut } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { dbService } from '../services/db';
import { useSettings } from '../context/SettingsContext';
import { useBank } from '../context/BankContext';
import { useUpdater } from '../hooks/useUpdater';
import { mobileCompanionService, type MobileCompanionStatus, type MobilePasskeyInfo } from '../services/mobileCompanion';
import ImportModal from '../features/import/ImportModal';
import CsvImportModal from '../features/import/CsvImportModal';
import QifImportModal from '../features/import/QifImportModal';
import OfxImportModal from '../features/import/OfxImportModal';
import AlertModal from '../components/ui/AlertModal';
import ReleaseNotesModal from '../components/ui/ReleaseNotesModal';
import { LOGO_PATH } from '../utils/assets';
import { filterDuplicateTransactions, type ImportTransactionInput } from '../utils/importParsers';
import { hasTauriRuntime, isMobileCompanion } from '../utils/runtime';

const SettingsPage: React.FC = () => {
    const { settings, updateTheme, updatePrimaryColor } = useSettings();
    const { addTransaction, transactions: existingTransactions, unlinkMobileCompanion } = useBank();
    const { checkUpdate, isChecking, updateAvailable } = useUpdater();
    const isDesktopRuntime = hasTauriRuntime();
    const isMobileMode = isMobileCompanion();
    const [appVersion, setAppVersion] = useState<string | null>(null);
    const [mobileStatus, setMobileStatus] = useState<MobileCompanionStatus | null>(null);
    const [isMobileStatusLoading, setIsMobileStatusLoading] = useState(false);
    const [isUnlinkingMobile, setIsUnlinkingMobile] = useState(false);

    useEffect(() => {
        if (!isDesktopRuntime) return;
        import('@tauri-apps/api/app')
            .then(({ getVersion }) => getVersion())
            .then(setAppVersion)
            .catch(() => setAppVersion(null));
    }, [isDesktopRuntime]);

    useEffect(() => {
        if (!isDesktopRuntime) return;
        refreshMobileStatus();
    }, [isDesktopRuntime]);

    useEffect(() => {
        if (!isDesktopRuntime) return;
        let unlisten: (() => void) | undefined;
        import('@tauri-apps/api/event')
            .then(({ listen }) => listen('mobile-companion-status-changed', () => {
                refreshMobileStatus();
            }))
            .then(cleanup => {
                unlisten = cleanup;
            })
            .catch(error => console.error('Mobile companion listener failed:', error));
        return () => {
            if (unlisten) unlisten();
        };
    }, [isDesktopRuntime]);

    useEffect(() => {
        if (!isDesktopRuntime || !mobileStatus?.secureBridge?.enabled || mobileStatus.secureBridge.active) return;
        const interval = window.setInterval(() => {
            refreshMobileStatus();
        }, 5000);
        return () => window.clearInterval(interval);
    }, [isDesktopRuntime, mobileStatus?.secureBridge?.enabled, mobileStatus?.secureBridge?.active]);

    const [isImportModalOpen, setIsImportModalOpen] = useState(false);
    const [isCsvImportModalOpen, setIsCsvImportModalOpen] = useState(false);
    const [isQifImportModalOpen, setIsQifImportModalOpen] = useState(false);
    const [isOfxImportModalOpen, setIsOfxImportModalOpen] = useState(false);
    const [isReleaseNotesOpen, setIsReleaseNotesOpen] = useState(false);
    const [importFile, setImportFile] = useState<{ name: string; content: string } | null>(null);
    const [alertState, setAlertState] = useState<{
        isOpen: boolean;
        title: string;
        message: string;
        type: 'success' | 'error';
        technicalDetails?: string;
    }>({
        isOpen: false,
        title: '',
        message: '',
        type: 'success',
        technicalDetails: ''
    });

    const handleExportData = async () => {
        try {
            const [{ save }, { writeTextFile }] = await Promise.all([
                import('@tauri-apps/plugin-dialog'),
                import('@tauri-apps/plugin-fs')
            ]);
            const data = await dbService.exportData();
            const jsonString = JSON.stringify(data, null, 2);
            const encodedData = btoa(unescape(encodeURIComponent(jsonString)));

            const filePath = await save({
                filters: [{ name: 'DMX Money Backup', extensions: ['dmx'] }],
                defaultPath: `dmxmoney_backup_${new Date().toISOString().split('T')[0]}.dmx`
            });

            if (filePath) {
                await writeTextFile(filePath, encodedData);
                setAlertState({
                    isOpen: true,
                    title: 'Export réussi',
                    message: 'Vos données ont été exportées avec succès.',
                    type: 'success'
                });
            }
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Erreur d\'export',
                message: 'Impossible de créer la sauvegarde.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        }
    };

    const handleImportClick = async () => {
        try {
            const [{ open }, { readTextFile }] = await Promise.all([
                import('@tauri-apps/plugin-dialog'),
                import('@tauri-apps/plugin-fs')
            ]);
            const filePath = await open({
                filters: [{ name: 'Fichiers supportés', extensions: ['dmx', 'json', 'csv', 'qif', 'ofx'] }]
            });

            if (filePath) {
                const content = await readTextFile(filePath as string);
                const fileName = (filePath as string).split(/[/\\]/).pop() || 'backup.dmx';
                setImportFile({ name: fileName, content });

                if (fileName.toLowerCase().endsWith('.csv')) setIsCsvImportModalOpen(true);
                else if (fileName.toLowerCase().endsWith('.qif')) setIsQifImportModalOpen(true);
                else if (fileName.toLowerCase().endsWith('.ofx')) setIsOfxImportModalOpen(true);
                else setIsImportModalOpen(true);
            }
        } catch (error) {
            console.error('File selection failed:', error);
            setAlertState({
                isOpen: true,
                title: 'Erreur de lecture',
                message: 'Le fichier n\'a pas pu être lu.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        }
    };

    async function refreshMobileStatus() {
        setIsMobileStatusLoading(true);
        try {
            setMobileStatus(await mobileCompanionService.getStatus());
        } catch (error) {
            console.error('Mobile companion status failed:', error);
            setMobileStatus(null);
        } finally {
            setIsMobileStatusLoading(false);
        }
    }

    const handleToggleSecureBridge = async (enabled: boolean) => {
        setIsMobileStatusLoading(true);
        try {
            console.info('Mobile companion toggle requested:', enabled);
            setMobileStatus(await mobileCompanionService.setSecureBridgeEnabled(enabled));
        } catch (error) {
            console.error('Mobile companion toggle failed:', error);
            setAlertState({
                isOpen: true,
                title: 'Pont sécurisé indisponible',
                message: 'Impossible de modifier le pont HTTPS.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsMobileStatusLoading(false);
        }
    };

    const handleRegenerateSecurePairing = async () => {
        setIsMobileStatusLoading(true);
        try {
            setMobileStatus(await mobileCompanionService.regenerateSecurePairingToken());
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Pairing impossible',
                message: 'Le QR sécurisé n’a pas pu être généré.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsMobileStatusLoading(false);
        }
    };

    const handleCopySecurePairingUrl = async () => {
        const url = mobileStatus?.secureBridge?.pairingUrl;
        if (!url) return;
        try {
            await navigator.clipboard.writeText(url);
            setAlertState({
                isOpen: true,
                title: 'URL copiée',
                message: 'Le lien de pairing sécurisé est dans le presse-papiers.',
                type: 'success'
            });
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Copie impossible',
                message: 'Le lien n’a pas pu être copié automatiquement.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        }
    };

    const getMobilePasskeyName = (passkey: MobilePasskeyInfo) => {
        const label = passkey.deviceLabel?.trim();
        if (label && label !== 'Mobile') {
            if (label === 'iOS') return 'iPhone ou iPad - PWA';
            return label;
        }
        return 'Mobile appairé';
    };

    const getMobilePasskeyMeta = (passkey: MobilePasskeyInfo) => {
        if (passkey.lastUsedAt) return `Utilisé ${new Date(passkey.lastUsedAt).toLocaleString()}`;
        return `Appairé ${new Date(passkey.createdAt).toLocaleString()}`;
    };

    const handleRevokeMobilePasskey = async (passkey: MobilePasskeyInfo) => {
        const deviceName = getMobilePasskeyName(passkey);
        const confirmed = window.confirm(
            `Désappairer ${deviceName} ? Ce mobile devra scanner un nouveau QR pour se reconnecter.`
        );
        if (!confirmed) return;

        setIsMobileStatusLoading(true);
        try {
            setMobileStatus(await mobileCompanionService.revokeMobilePasskey(passkey.id));
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Désappairage impossible',
                message: 'Le mobile n’a pas pu être désappairé.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsMobileStatusLoading(false);
        }
    };

    const handleUnlinkMobilePwa = async () => {
        const confirmed = window.confirm(
            'Déconnecter cette PWA ? La session, les infos de pairing, le cache offline et les modifications en attente seront supprimés sur ce mobile.'
        );
        if (!confirmed) return;

        setIsUnlinkingMobile(true);
        try {
            await unlinkMobileCompanion();
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Déconnexion impossible',
                message: 'La PWA n’a pas pu supprimer ses données locales.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsUnlinkingMobile(false);
        }
    };

    const handleConfirmImport = async (mode: 'replace' | 'merge') => {
        if (!importFile) return;
        try {
            let data;
            try { data = JSON.parse(importFile.content); }
            catch (e) {
                const jsonString = decodeURIComponent(escape(atob(importFile.content)));
                data = JSON.parse(jsonString);
            }
            if (mode === 'merge') await dbService.mergeData(data);
            else await dbService.importData(data);

            setAlertState({
                isOpen: true,
                title: 'Import réussi',
                message: 'Les données ont été restaurées. Redémarrage...',
                type: 'success'
            });
            setTimeout(() => window.location.reload(), 2000);
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Fichier invalide',
                message: 'Le fichier de sauvegarde est corrompu.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsImportModalOpen(false);
            setImportFile(null);
        }
    };

    const handleTransactionImport = async (transactions: ImportTransactionInput[], accountId: string) => {
        try {
            const preparedTransactions = transactions.map(transaction => ({ ...transaction, accountId }));
            const { unique, duplicateCount } = filterDuplicateTransactions(preparedTransactions, existingTransactions, accountId);

            for (const tx of unique) {
                await addTransaction({ ...tx, checked: tx.checked ?? true });
            }
            setAlertState({
                isOpen: true,
                title: 'Import terminé',
                message: duplicateCount > 0
                    ? `${unique.length} transactions importées, ${duplicateCount} doublon${duplicateCount > 1 ? 's' : ''} ignoré${duplicateCount > 1 ? 's' : ''}.`
                    : `${unique.length} transactions importées.`,
                type: 'success'
            });
        } catch (error) {
            setAlertState({
                isOpen: true,
                title: 'Erreur d\'import',
                message: 'Certaines transactions ont échoué.',
                type: 'error',
                technicalDetails: error instanceof Error ? error.message : String(error)
            });
        } finally {
            setIsCsvImportModalOpen(false);
            setIsQifImportModalOpen(false);
            setIsOfxImportModalOpen(false);
            setImportFile(null);
        }
    };

    const colors = [
        '#007AFF', '#AF52DE', '#FF2D55', '#FF3B30', '#FF9500',
        '#FFCC00', '#34C759', '#5AC8FA', '#5856D6', '#8E8E93'
    ];
    const secureBridge = mobileStatus?.secureBridge;
    const secureBridgeEnabled = Boolean(secureBridge?.enabled);
    const secureBridgeActive = Boolean(secureBridge?.active);
    const certificateReady = Boolean(secureBridge?.certificateReady);
    const activePasskeys = secureBridge?.passkeys?.filter(item => !item.revokedAt) ?? [];
    const bridgeDegraded = Boolean(secureBridge?.degraded);
    const provisioningReady = Boolean(secureBridge?.configured);
    const provisioningPending = !secureBridgeEnabled && !provisioningReady;
    const provisioningLabel = provisioningReady
        ? 'Prêt'
        : provisioningPending
            ? 'En attente d’activation'
            : 'En cours ou indisponible';
    const dnsLabel = secureBridge?.dnsRecordId
        ? 'Configuré'
        : secureBridge?.managedCredentialReady
            ? 'Prêt'
            : secureBridgeEnabled
                ? 'En attente'
                : 'En attente d’activation';
    const localLabel = mobileStatus?.active
        ? 'Actif'
        : secureBridgeEnabled
            ? 'Démarrage'
            : 'Inactif';
    const companionState = !secureBridgeEnabled
        ? {
            label: 'Désactivé',
            detail: 'Active le mode pour préparer le pont HTTPS et le QR mobile.',
            className: 'bg-gray-100 text-gray-600 dark:bg-white/10 dark:text-gray-300'
        }
        : secureBridgeActive
            ? {
                label: 'Prêt à appairer',
                detail: 'La PWA peut se connecter à l’API locale sécurisée.',
                className: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/10 dark:text-emerald-300'
            }
            : certificateReady
                ? {
                    label: 'Démarrage local',
                    detail: 'Le certificat est prêt, le serveur local termine son démarrage.',
                    className: 'bg-blue-50 text-blue-700 dark:bg-blue-500/10 dark:text-blue-300'
                }
                : {
                    label: 'Préparation HTTPS',
                    detail: 'DNS et certificat sont préparés automatiquement en arrière-plan.',
                    className: 'bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:text-amber-300'
                };
    const pairingButtonLabel = secureBridgeActive
        ? 'Nouveau QR'
        : secureBridgeEnabled
            ? 'Préparation HTTPS'
            : 'Activer d’abord';
    const qrEmptyMessage = !secureBridgeEnabled
        ? 'Le QR sera disponible après activation.'
        : !certificateReady
            ? 'Certificat HTTPS en cours de génération.'
            : !secureBridgeActive
                ? 'Serveur local en démarrage.'
                : 'Génère un QR pour appairer un mobile.';
    const companionSteps = [
        {
            label: 'PWA publique',
            value: secureBridge?.appUrl ? 'Disponible' : 'En attente',
            ready: Boolean(secureBridge?.appUrl),
            icon: Globe2
        },
        {
            label: 'Provisionnement',
            value: provisioningLabel,
            ready: provisioningReady,
            icon: KeyRound
        },
        {
            label: 'DNS local',
            value: dnsLabel,
            ready: Boolean(secureBridge?.dnsRecordId),
            icon: Wifi
        },
        {
            label: 'Certificat HTTPS',
            value: certificateReady ? 'Prêt' : secureBridgeEnabled ? 'En génération' : 'Absent',
            ready: certificateReady,
            icon: ShieldCheck
        },
        {
            label: 'API locale',
            value: secureBridge?.apiUrl ? localLabel : 'Non active',
            ready: secureBridgeActive,
            icon: Server
        }
    ];

    return (
        <div className="max-w-3xl mx-auto pb-20 animate-in fade-in duration-300">
            <header className="hidden md:block mb-10 px-2">
                <h1 className="text-3xl font-bold text-gray-900 dark:text-white tracking-tight">Paramètres</h1>
            </header>

            <div className="space-y-10">
                {/* SECTION APPARENCE */}
                <section>
                    <div className="flex items-center gap-2 mb-3 px-2">
                        <Palette className="w-4 h-4 text-gray-400" />
                        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">Apparence</h2>
                    </div>
                    <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl border border-black/[0.05] dark:border-white/[0.05] shadow-sm overflow-hidden divide-y divide-black/[0.05] dark:divide-white/[0.05]">
                        
                        {/* Thème */}
                        <div className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                            <div>
                                <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Thème de l'interface</h3>
                                <p className="text-[13px] text-gray-500 mt-0.5">Choisissez l'aspect visuel de l'application</p>
                            </div>
                            <div className="flex bg-gray-100 dark:bg-black/50 p-1 rounded-xl">
                                {[
                                    { id: 'light', icon: Sun },
                                    { id: 'dark', icon: Moon },
                                    { id: 'system', icon: Monitor }
                                ].map(t => (
                                    <button
                                        key={t.id}
                                        onClick={() => updateTheme(t.id as any)}
                                        className={`px-4 py-1.5 rounded-lg flex items-center justify-center transition-all duration-200 ${
                                            settings.theme === t.id 
                                            ? 'bg-white dark:bg-[#2C2C2E] text-gray-900 dark:text-white shadow-sm ring-1 ring-black/5 dark:ring-white/10' 
                                            : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
                                        }`}
                                    >
                                        <t.icon className="w-4 h-4" />
                                    </button>
                                ))}
                            </div>
                        </div>

                        {/* Couleurs */}
                        <div className="p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                            <div>
                                <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Couleur d'accentuation</h3>
                                <p className="text-[13px] text-gray-500 mt-0.5">Personnalisez la couleur principale</p>
                            </div>
                            <div className="flex flex-wrap items-center gap-2">
                                <button
                                    onClick={() => updatePrimaryColor('default')}
                                    className={`w-7 h-7 rounded-full border-2 transition-transform duration-200 flex items-center justify-center text-[10px] font-bold ${
                                        settings.primaryColor === 'default' 
                                        ? 'border-gray-900 dark:border-white scale-110 text-gray-900 dark:text-white' 
                                        : 'border-gray-200 dark:border-gray-700 text-gray-400 hover:scale-110'
                                    }`}
                                >
                                    Def
                                </button>
                                <div className="w-px h-5 bg-gray-200 dark:bg-gray-800 mx-1"></div>
                                {colors.map(color => (
                                    <button
                                        key={color}
                                        onClick={() => updatePrimaryColor(color)}
                                        className={`w-7 h-7 rounded-full border-2 transition-all duration-200 ${
                                            settings.primaryColor === color 
                                            ? 'border-gray-900 dark:border-white scale-110' 
                                            : 'border-transparent hover:scale-110'
                                        }`}
                                        style={{ backgroundColor: color }}
                                    />
                                ))}
                            </div>
                        </div>
                    </div>
                </section>

                {isMobileMode && (
                    <section>
                        <div className="flex items-center gap-2 mb-3 px-2">
                            <Smartphone className="w-4 h-4 text-gray-400" />
                            <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">Cette PWA</h2>
                        </div>
                        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl border border-black/[0.05] dark:border-white/[0.05] shadow-sm overflow-hidden">
                            <div className="p-4 flex flex-col gap-4">
                                <div className="flex items-start gap-4">
                                    <div className="p-2 bg-red-50 dark:bg-red-500/10 rounded-lg text-red-600 dark:text-red-300">
                                        <LogOut className="w-5 h-5" />
                                    </div>
                                    <div>
                                        <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Déconnecter ce mobile</h3>
                                        <p className="text-[13px] text-gray-500 mt-0.5 leading-relaxed">
                                            Supprime la session, les infos de pairing, le cache offline et les modifications en attente sur cette PWA.
                                        </p>
                                    </div>
                                </div>
                                <button
                                    onClick={handleUnlinkMobilePwa}
                                    disabled={isUnlinkingMobile}
                                    className="w-full inline-flex items-center justify-center gap-2 px-4 py-3 rounded-xl bg-red-50 dark:bg-red-500/10 text-[13px] font-semibold text-red-600 dark:text-red-300 hover:bg-red-100 dark:hover:bg-red-500/15 transition-colors disabled:opacity-60"
                                >
                                    {isUnlinkingMobile ? <RefreshCw className="w-4 h-4 animate-spin" /> : <LogOut className="w-4 h-4" />}
                                    {isUnlinkingMobile ? 'Déconnexion...' : 'Déconnecter et effacer'}
                                </button>
                            </div>
                        </div>
                    </section>
                )}

                {isDesktopRuntime && (
                    <section>
                        <div className="flex items-center gap-2 mb-3 px-2">
                            <Smartphone className="w-4 h-4 text-gray-400" />
                            <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">Mode compagnon mobile</h2>
                        </div>
                        <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl border border-black/[0.05] dark:border-white/[0.05] shadow-sm overflow-hidden">
                            <div className="p-4 border-b border-black/[0.05] dark:border-white/[0.05]">
                                <div className="flex flex-col sm:flex-row sm:items-start justify-between gap-3">
                                    <div className="flex items-start gap-3 min-w-0">
                                        <div className="shrink-0 p-2 bg-emerald-50 dark:bg-emerald-500/10 rounded-xl text-emerald-600 dark:text-emerald-300">
                                            <Lock className="w-4 h-4" />
                                        </div>
                                        <div className="min-w-0">
                                            <div className="flex flex-wrap items-center gap-2">
                                                <h3 className="text-[15px] font-semibold text-gray-900 dark:text-white">Accès mobile local + PWA</h3>
                                                <span className={`px-2 py-0.5 rounded-full text-[10px] font-semibold ${companionState.className}`}>
                                                    {companionState.label}
                                                </span>
                                            </div>
                                            <p className="text-[12px] text-gray-500 mt-0.5 leading-snug">
                                                {companionState.detail}
                                            </p>
                                        </div>
                                    </div>
                                    <label className="inline-flex items-center gap-3 text-[13px] font-semibold text-gray-700 dark:text-gray-300">
                                        <input
                                            type="checkbox"
                                            checked={secureBridgeEnabled}
                                            disabled={isMobileStatusLoading}
                                            onChange={(event) => handleToggleSecureBridge(event.target.checked)}
                                            className="h-5 w-5 rounded border-gray-300 text-primary-500 focus:ring-primary-500"
                                        />
                                        Activer
                                    </label>
                                </div>

                                <div className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-2">
                                    <div className="rounded-xl bg-gray-50 dark:bg-black/30 px-3 py-2 min-w-0">
                                        <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase text-gray-400 tracking-wider">
                                            <Globe2 className="w-3 h-3" />
                                            PWA mobile
                                        </div>
                                        <p className="mt-1 text-[12px] text-gray-700 dark:text-gray-200 truncate">
                                            {secureBridge?.appUrl || 'Provisionnement automatique en attente'}
                                        </p>
                                    </div>
                                    <div className="rounded-xl bg-gray-50 dark:bg-black/30 px-3 py-2 min-w-0">
                                        <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase text-gray-400 tracking-wider">
                                            <Server className="w-3 h-3" />
                                            API locale sécurisée
                                        </div>
                                        <p className="mt-1 text-[12px] text-gray-700 dark:text-gray-200 truncate">
                                            {secureBridge?.apiUrl || 'Non active'}
                                        </p>
                                    </div>
                                </div>
                            </div>

                            <div className="p-4 grid grid-cols-1 lg:grid-cols-[1fr,176px] gap-4">
                                <div className="space-y-2">
                                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                                        {companionSteps.map(({ label, value, ready, icon: Icon }) => (
                                            <div key={label} className="flex items-center gap-2 rounded-xl bg-gray-50 dark:bg-black/30 px-3 py-2 min-w-0">
                                                <div className={`shrink-0 w-6 h-6 rounded-lg flex items-center justify-center ${
                                                    ready
                                                        ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-500/10 dark:text-emerald-300'
                                                        : secureBridgeEnabled
                                                            ? 'bg-amber-50 text-amber-600 dark:bg-amber-500/10 dark:text-amber-300'
                                                            : 'bg-gray-100 text-gray-400 dark:bg-white/10 dark:text-gray-500'
                                                }`}>
                                                    {ready ? <CheckCircle2 className="w-3.5 h-3.5" /> : <Icon className="w-3.5 h-3.5" />}
                                                </div>
                                                <div className="min-w-0 flex-1">
                                                    <p className="text-[12px] font-medium text-gray-900 dark:text-white truncate">{label}</p>
                                                    <p className="text-[11px] text-gray-500 dark:text-gray-400 truncate">{value}</p>
                                                </div>
                                            </div>
                                        ))}
                                    </div>

                                    <div className="flex flex-wrap gap-1.5 text-[11px]">
                                        <span className="rounded-full bg-gray-50 dark:bg-black/30 px-2.5 py-1 text-gray-500 dark:text-gray-400">
                                            Local <strong className="ml-1 font-semibold text-gray-800 dark:text-gray-200">{localLabel}</strong>
                                        </span>
                                        <span className="rounded-full bg-gray-50 dark:bg-black/30 px-2.5 py-1 text-gray-500 dark:text-gray-400">
                                            Certificat <strong className="ml-1 font-semibold text-gray-800 dark:text-gray-200">{certificateReady ? 'Prêt' : 'Absent'}</strong>
                                        </span>
                                        <span className="rounded-full bg-gray-50 dark:bg-black/30 px-2.5 py-1 text-gray-500 dark:text-gray-400">
                                            Mobiles <strong className="ml-1 font-semibold text-gray-800 dark:text-gray-200">{activePasskeys.length}</strong>
                                        </span>
                                    </div>

                                    {secureBridgeEnabled && secureBridge?.lastError && (
                                        <div className={`rounded-xl px-3 py-2 text-[12px] leading-relaxed ${
                                            bridgeDegraded
                                                ? 'bg-amber-50 text-amber-800 dark:bg-amber-500/10 dark:text-amber-200'
                                                : 'bg-red-50 text-red-700 dark:bg-red-500/10 dark:text-red-200'
                                        }`}>
                                            <div className="flex items-start gap-2">
                                                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
                                                <div className="space-y-1">
                                                    {bridgeDegraded && (
                                                        <p className="font-semibold">Appairage toujours possible</p>
                                                    )}
                                                    <p>{secureBridge.lastError}</p>
                                                </div>
                                            </div>
                                        </div>
                                    )}
                                </div>

                                <div className="space-y-2">
                                    <div className="rounded-xl border border-black/10 dark:border-white/10 bg-white dark:bg-black/20 p-3 flex flex-col items-center text-center">
                                        <div className="w-28 h-28 rounded-xl bg-white border border-black/10 flex items-center justify-center">
                                            {secureBridge?.pairingUrl ? (
                                                <QRCodeSVG value={secureBridge.pairingUrl} size={96} level="M" />
                                            ) : (
                                                <div className="px-3 text-center">
                                                    <KeyRound className="w-6 h-6 text-gray-300 mx-auto mb-1.5" />
                                                    <p className="text-[11px] leading-snug text-gray-400">{qrEmptyMessage}</p>
                                                </div>
                                            )}
                                        </div>
                                        <div className="mt-2.5 w-full space-y-1.5">
                                            <button
                                                onClick={handleRegenerateSecurePairing}
                                                disabled={isMobileStatusLoading || !secureBridgeActive}
                                                className="w-full inline-flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg bg-primary-500 text-[12px] font-semibold text-white hover:bg-primary-600 transition-colors disabled:bg-gray-100 disabled:text-gray-400 dark:disabled:bg-white/10"
                                            >
                                                <KeyRound className="w-3.5 h-3.5" />
                                                {pairingButtonLabel}
                                            </button>
                                            {secureBridge?.pairingUrl && (
                                                <button
                                                    onClick={handleCopySecurePairingUrl}
                                                    className="w-full inline-flex items-center justify-center gap-2 px-3 py-1.5 rounded-lg bg-gray-100 dark:bg-black/40 text-[12px] font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-black/60 transition-colors"
                                                >
                                                    <Copy className="w-3.5 h-3.5" />
                                                    Copier
                                                </button>
                                            )}
                                        </div>
                                    </div>
                                    <p className="text-[11px] text-gray-500 dark:text-gray-400 leading-snug">
                                        Ouvre la PWA sur mobile, puis appaire le téléphone avec ce QR. Les données apparaissent après cette étape.
                                        Génère un QR par appareil : plusieurs mobiles peuvent rester appairés en même temps.
                                    </p>
                                </div>
                            </div>

                            {secureBridge && (
                                <div className="px-4 pb-4 space-y-2">
                                    <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">
                                        Mobiles appairés ({activePasskeys.length})
                                    </p>
                                    {activePasskeys.length > 0 ? (
                                        activePasskeys.map(passkey => (
                                            <div key={passkey.id} className="flex items-center justify-between gap-3 px-3 py-2 rounded-xl bg-gray-50 dark:bg-black/30">
                                                <div className="min-w-0 flex items-center gap-2.5">
                                                    <div className="w-8 h-8 rounded-lg bg-white dark:bg-white/10 flex items-center justify-center text-gray-500 dark:text-gray-300">
                                                        <Smartphone className="w-3.5 h-3.5" />
                                                    </div>
                                                    <div className="min-w-0">
                                                        <p className="text-[12px] font-medium text-gray-900 dark:text-white truncate">{getMobilePasskeyName(passkey)}</p>
                                                        <p className="text-[11px] text-gray-500 truncate">{getMobilePasskeyMeta(passkey)}</p>
                                                    </div>
                                                </div>
                                                <button
                                                    onClick={() => handleRevokeMobilePasskey(passkey)}
                                                    disabled={isMobileStatusLoading}
                                                    className="px-2.5 py-1.5 rounded-lg bg-red-50 dark:bg-red-500/10 text-[11px] font-medium text-red-600 dark:text-red-300 disabled:opacity-60"
                                                >
                                                    Désappairer
                                                </button>
                                            </div>
                                        ))
                                    ) : (
                                        <div className="rounded-xl bg-gray-50 dark:bg-black/30 px-3 py-2 text-[12px] text-gray-500 dark:text-gray-400">
                                            Aucun mobile appairé pour l’instant.
                                        </div>
                                    )}
                                    {activePasskeys.length === 1 && (
                                        <p className="px-1 text-[11px] text-gray-500 dark:text-gray-400">
                                            Pour ajouter un second appareil, génère un nouveau QR et scanne-le depuis celui-ci : le premier reste appairé.
                                        </p>
                                    )}
                                </div>
                            )}
                        </div>
                    </section>
                )}

                {/* SECTION DONNÉES */}
                {!isMobileMode && (
                <section>
                    <div className="flex items-center gap-2 mb-3 px-2">
                        <HardDrive className="w-4 h-4 text-gray-400" />
                        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">Données & Stockage</h2>
                    </div>
                    <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl border border-black/[0.05] dark:border-white/[0.05] shadow-sm overflow-hidden divide-y divide-black/[0.05] dark:divide-white/[0.05]">
                        
                        <button 
                            onClick={handleExportData}
                            className="w-full flex items-center justify-between p-4 hover:bg-gray-50 dark:hover:bg-[#2C2C2E]/50 transition-colors text-left"
                        >
                            <div className="flex items-center gap-4">
                                <div className="p-2 bg-gray-100 dark:bg-black/50 rounded-lg text-gray-600 dark:text-gray-400">
                                    <Download className="w-5 h-5" />
                                </div>
                                <div>
                                    <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Exporter les données</h3>
                                    <p className="text-[13px] text-gray-500 mt-0.5">Créer une sauvegarde locale (.dmx)</p>
                                </div>
                            </div>
                            <ChevronRight className="w-5 h-5 text-gray-300 dark:text-gray-600" />
                        </button>

                        <button 
                            onClick={handleImportClick}
                            className="w-full flex items-center justify-between p-4 hover:bg-gray-50 dark:hover:bg-[#2C2C2E]/50 transition-colors text-left"
                        >
                            <div className="flex items-center gap-4">
                                <div className="p-2 bg-gray-100 dark:bg-black/50 rounded-lg text-gray-600 dark:text-gray-400">
                                    <Upload className="w-5 h-5" />
                                </div>
                                <div>
                                    <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Importer ou Restaurer</h3>
                                    <p className="text-[13px] text-gray-500 mt-0.5">Depuis un backup ou un fichier CSV/OFX/QIF</p>
                                </div>
                            </div>
                            <ChevronRight className="w-5 h-5 text-gray-300 dark:text-gray-600" />
                        </button>
                    </div>
                </section>
                )}

                {/* SECTION A PROPOS */}
                <section>
                    <div className="flex items-center gap-2 mb-3 px-2">
                        <Info className="w-4 h-4 text-gray-400" />
                        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">À propos</h2>
                    </div>
                    <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl border border-black/[0.05] dark:border-white/[0.05] shadow-sm overflow-hidden divide-y divide-black/[0.05] dark:divide-white/[0.05]">
                        
                        <div className="p-4 flex items-center justify-between">
                            <div className="flex items-center gap-4">
                                <img src={LOGO_PATH} alt="Logo" className="w-10 h-10 rounded-xl" />
                                <div>
                                    <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">DmxMoney</h3>
                                    <p className="text-[13px] text-gray-500 mt-0.5">{appVersion ? `Version ${appVersion}` : 'Version indisponible'}</p>
                                </div>
                            </div>
                            <button
                                onClick={() => setIsReleaseNotesOpen(true)}
                                className="text-[13px] font-medium text-primary-500 hover:text-primary-600 dark:hover:text-primary-400 bg-primary-50 dark:bg-primary-500/10 px-3 py-1.5 rounded-lg transition-colors"
                            >
                                Nouveautés
                            </button>
                        </div>

                        {!isMobileMode && (
                        <div className="p-4 flex items-center justify-between">
                            <div>
                                <h3 className="text-[15px] font-medium text-gray-900 dark:text-white">Mise à jour logicielle</h3>
                                <div className="flex items-center gap-2 mt-0.5">
                                    <span className="text-[13px] text-gray-500">
                                        {updateAvailable ? "Nouvelle version disponible" : "L'application est à jour"}
                                    </span>
                                    {updateAvailable && <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />}
                                </div>
                            </div>
                            <button
                                onClick={() => checkUpdate()}
                                disabled={isChecking}
                                className={`flex items-center gap-2 text-[13px] font-medium px-4 py-2 rounded-xl transition-all ${
                                    updateAvailable 
                                    ? 'bg-primary-500 text-white hover:bg-primary-600 shadow-sm' 
                                    : 'bg-gray-100 dark:bg-black/50 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-black/70'
                                }`}
                            >
                                <RefreshCw className={`w-4 h-4 ${isChecking ? 'animate-spin' : ''}`} />
                                {updateAvailable ? "Installer" : "Vérifier"}
                            </button>
                        </div>
                        )}
                    </div>
                </section>
            </div>

            <AlertModal
                isOpen={alertState.isOpen}
                onClose={() => setAlertState({ ...alertState, isOpen: false })}
                title={alertState.title}
                message={alertState.message}
                type={alertState.type}
                technicalDetails={alertState.technicalDetails}
            />
            <ReleaseNotesModal
                isOpen={isReleaseNotesOpen}
                onClose={() => setIsReleaseNotesOpen(false)}
            />
            <ImportModal isOpen={isImportModalOpen} onClose={() => setIsImportModalOpen(false)} onImport={handleConfirmImport} fileName={importFile?.name || ''} />
            <CsvImportModal isOpen={isCsvImportModalOpen} onClose={() => setIsCsvImportModalOpen(false)} file={importFile} onImport={handleTransactionImport} />
            <QifImportModal isOpen={isQifImportModalOpen} onClose={() => setIsQifImportModalOpen(false)} file={importFile} onImport={handleTransactionImport} />
            <OfxImportModal isOpen={isOfxImportModalOpen} onClose={() => setIsOfxImportModalOpen(false)} file={importFile} onImport={handleTransactionImport} />
        </div>
    );
};

export default SettingsPage;
