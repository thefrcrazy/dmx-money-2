import React, { useState, useEffect } from 'react';
import Layout from './layouts/Layout';
import Dashboard from './pages/Dashboard';
import Transactions from './pages/Transactions';
import Scheduled from './pages/Scheduled';
import Analytics from './pages/Analytics';
import Accounts from './pages/Accounts';
import Predictions from './pages/Predictions';
import Settings from './pages/Settings';
import Budget from './pages/Budget';
import Categories from './pages/Categories';
import { BankProvider } from './context/BankContext';
import { SettingsProvider, useSettings } from './context/SettingsContext';
import { NavigationProvider, useNavigation } from './context/NavigationContext';
import { ToastProvider } from './context/ToastContext';
import { useUpdater } from './hooks/useUpdater';
import { LATEST_VERSION } from './constants/changelog';
import ReleaseNotesModal from './components/ui/ReleaseNotesModal';
import { useBank } from './context/BankContext';
import { applyMobileCompanionPairingUrl, getMobilePlatform, hasMobileCompanionSetup, hasMobilePasskeySetup, isMobileCompanion, isStandalonePwa } from './utils/runtime';
import { LOGO_PATH } from './utils/assets';
import type { IScannerControls } from '@zxing/browser';
import { ArrowDown, Camera, ClipboardPaste, Download, Home, KeyRound, Link2, LogOut, MoreHorizontal, PlusSquare, QrCode, RefreshCw, Share2, ShieldCheck, VideoOff, WifiOff, X } from 'lucide-react';

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed'; platform: string }>;
};

const MobileInstallDemo: React.FC<{ platform: ReturnType<typeof getMobilePlatform> }> = ({ platform }) => (
  <div className="mt-6 rounded-2xl border border-black/[0.06] dark:border-white/10 bg-white dark:bg-[#121212] p-4 shadow-sm">
    <div className={`pwa-demo pwa-demo-install ${platform === 'ios' ? 'pwa-demo-install-ios' : ''}`} aria-label="Démonstration d’installation PWA">
      <div className="pwa-install-browser">
        <div className="pwa-install-address" />
        <div className="pwa-install-page">
          <img src={LOGO_PATH} alt="" className="h-9 w-9 rounded-xl" />
          <div className="pwa-install-page-lines">
            <span />
            <span />
          </div>
        </div>
        <div className="pwa-install-toolbar">
          <MoreHorizontal className="h-4 w-4" />
        </div>
      </div>
      <div className="pwa-install-menu" aria-hidden="true">
        <div>
          {platform === 'ios' ? <Share2 className="h-4 w-4" /> : <Download className="h-4 w-4" />}
          <span>{platform === 'ios' ? 'Partager' : 'Installer'}</span>
        </div>
      </div>
      {platform === 'ios' && (
        <div className="pwa-install-scroll" aria-hidden="true">
          <ArrowDown className="h-4 w-4" />
          <span>Descendre</span>
        </div>
      )}
      <div className="pwa-install-sheet" aria-hidden="true">
        <PlusSquare className="h-4 w-4" />
        <span>{platform === 'ios' ? 'Sur l’écran d’accueil' : 'Ajouter à l’écran'}</span>
      </div>
      <div className="pwa-install-home" aria-hidden="true">
        <div className="pwa-install-grid">
          <span />
          <span />
          <span />
          <span />
        </div>
        <div className="pwa-install-app-icon">
          <img src={LOGO_PATH} alt="" />
        </div>
      </div>
    </div>
  </div>
);

const MobilePairingDemo: React.FC = () => (
  <div className="mt-5 overflow-hidden rounded-[24px] border border-black/[0.06] bg-white p-4 shadow-sm dark:border-white/10 dark:bg-[#121212]">
    <div className="pwa-pairing-hero" aria-label="Démonstration de liaison QR code">
      <div className="pwa-pairing-phone">
        <div className="pwa-pairing-speaker" />
        <div className="pwa-pairing-app-row">
          <img src={LOGO_PATH} alt="" />
          <div>
            <span />
            <span />
          </div>
        </div>
        <div className="pwa-pairing-qr-frame">
          <QrCode className="h-20 w-20" />
        </div>
        <p>QR du desktop</p>
      </div>
      <div className="pwa-pairing-corners" aria-hidden="true" />
    </div>
  </div>
);

const isSetupRequiredMobileError = (message: string | null) => {
  if (!message) return false;
  const lowerMessage = message.toLowerCase();
  return lowerMessage.includes('configuration mobile manquante')
    || lowerMessage.includes('url api')
    || lowerMessage.includes('aucune passkey')
    || lowerMessage.includes('session mobile expirée')
    || lowerMessage.includes('non finalisée')
    || lowerMessage.includes('ouvrez le lien depuis le qr');
};

const MobileInstallGuide: React.FC = () => {
  const platform = getMobilePlatform();
  const isIos = platform === 'ios';
  const isAndroid = platform === 'android';
  const [installPrompt, setInstallPrompt] = useState<BeforeInstallPromptEvent | null>(null);
  const [installMessage, setInstallMessage] = useState<string | null>(null);

  useEffect(() => {
    const handleBeforeInstallPrompt = (event: Event) => {
      event.preventDefault();
      setInstallPrompt(event as BeforeInstallPromptEvent);
      setInstallMessage(null);
    };
    const handleAppInstalled = () => {
      setInstallPrompt(null);
      setInstallMessage('DmxMoney est installée. Ouvre maintenant l’app depuis ton écran d’accueil.');
    };

    window.addEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
    window.addEventListener('appinstalled', handleAppInstalled);
    return () => {
      window.removeEventListener('beforeinstallprompt', handleBeforeInstallPrompt);
      window.removeEventListener('appinstalled', handleAppInstalled);
    };
  }, []);

  const handleInstallClick = async () => {
    if (!installPrompt) {
      setInstallMessage(isIos
        ? 'iOS ne permet pas à une page web d’ajouter automatiquement une PWA à l’écran d’accueil. Utilise le menu Partage de Safari.'
        : 'Ce navigateur ne propose pas l’installation automatique pour le moment. Utilise le menu du navigateur.');
      return;
    }

    try {
      await installPrompt.prompt();
      const choice = await installPrompt.userChoice;
      setInstallPrompt(null);
      setInstallMessage(choice.outcome === 'accepted'
        ? 'Installation lancée. Ouvre ensuite DmxMoney depuis l’icône ajoutée.'
        : 'Installation annulée. Tu peux relancer l’installation depuis le menu du navigateur.');
    } catch (error) {
      setInstallMessage(error instanceof Error ? error.message : 'Installation automatique indisponible.');
    }
  };

  const steps = isIos
    ? [
      { icon: MoreHorizontal, label: 'Appuie sur le bouton “…” du navigateur' },
      { icon: Share2, label: 'Choisis “Partager”' },
      { icon: ArrowDown, label: 'Descends dans le menu de partage' },
      { icon: PlusSquare, label: 'Appuie sur “Sur l’écran d’accueil” puis “Ajouter”' },
    ]
    : [
      { icon: MoreHorizontal, label: isAndroid ? 'Ouvre le menu Chrome' : 'Ouvre le menu du navigateur mobile' },
      { icon: Download, label: 'Choisis “Installer l’application” ou “Ajouter à l’écran d’accueil”' },
      { icon: Home, label: 'Lance DmxMoney depuis l’icône installée' },
    ];

  return (
    <div className="h-[100dvh] overflow-y-auto overscroll-contain bg-[var(--color-bg-tertiary)] dark:bg-[var(--color-bg-tertiary)] text-gray-900 dark:text-gray-100 px-5 pt-[calc(env(safe-area-inset-top)+24px)] pb-[calc(env(safe-area-inset-bottom)+32px)]">
      <div className="mx-auto flex min-h-full w-full max-w-sm flex-col justify-center">
        <div className="flex flex-col items-center text-center">
          <img src={LOGO_PATH} alt="DmxMoney" className="w-20 h-20 rounded-2xl shadow-sm mb-5" />
          <div className="inline-flex items-center gap-2 rounded-full bg-primary-50 dark:bg-primary-500/10 px-3 py-1 text-[12px] font-semibold text-primary-700 dark:text-primary-300 mb-4">
            <Download className="w-3.5 h-3.5" />
            Installation requise
          </div>
          <h1 className="text-2xl font-bold tracking-tight">Ajouter DmxMoney</h1>
          <p className="mt-2 text-sm leading-relaxed text-gray-500 dark:text-gray-400">
            Installe la PWA avant de connecter ce mobile. Le scan QR et la clé d’accès s’ouvrent ensuite depuis l’app installée.
          </p>
        </div>

        <MobileInstallDemo platform={platform} />

        {!isIos && (
          <button
            type="button"
            onClick={handleInstallClick}
            disabled={!installPrompt && !isAndroid}
            className="mt-6 w-full h-12 rounded-xl bg-primary-500 text-white text-sm font-semibold flex items-center justify-center gap-2 shadow-sm hover:bg-primary-600 disabled:bg-gray-200 disabled:text-gray-500 dark:disabled:bg-white/10 transition-colors"
          >
            <Download className="w-4 h-4" />
            {installPrompt ? 'Installer automatiquement' : 'Essayer l’installation'}
          </button>
        )}

        {installMessage && (
          <div className="mt-3 rounded-xl border border-primary-100 dark:border-primary-500/20 bg-primary-50 dark:bg-primary-500/10 p-3 text-[12px] leading-relaxed text-primary-700 dark:text-primary-200">
            {installMessage}
          </div>
        )}

        <div className="mt-6 rounded-2xl border border-black/[0.06] dark:border-white/10 bg-white dark:bg-[#121212] p-4 shadow-sm">
          <div className="space-y-3">
            {steps.map(({ icon: Icon, label }, index) => (
              <div key={label} className="flex items-center gap-3 rounded-xl bg-gray-50 dark:bg-white/[0.04] px-3.5 py-3">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-white dark:bg-black/30 text-primary-600 dark:text-primary-300">
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-gray-400">Étape {index + 1}</p>
                  <p className="text-sm font-semibold text-gray-900 dark:text-white">{label}</p>
                </div>
              </div>
            ))}
          </div>

          <div className="mt-4 rounded-xl bg-amber-50 dark:bg-amber-500/10 px-3.5 py-3 text-[12px] leading-relaxed text-amber-700 dark:text-amber-200">
            Une fois ouverte depuis l’écran d’accueil, appuie sur “Scanner le QR code” et scanne le QR affiché dans l’application desktop.
          </div>
        </div>
      </div>
    </div>
  );
};

const MobileConnectionScreen: React.FC = () => {
  const { connectMobileCompanion, unlinkMobileCompanion, mobileConnectionState, mobileConnectionError, isLoading } = useBank();
  const launchedAsPwa = isStandalonePwa();
  const isConnecting = mobileConnectionState === 'connecting' || isLoading;
  const [hasPairingSetup, setHasPairingSetup] = useState(() => hasMobileCompanionSetup());
  const [isPairingOpen, setIsPairingOpen] = useState(false);
  const [isManualPairingOpen, setIsManualPairingOpen] = useState(false);
  const [pairingUrl, setPairingUrl] = useState('');
  const [pairingError, setPairingError] = useState<string | null>(null);
  const [connectionHint, setConnectionHint] = useState<string | null>(null);
  const [isApplyingPairing, setIsApplyingPairing] = useState(false);
  const [isScannerOpen, setIsScannerOpen] = useState(false);
  const [isScannerStarting, setIsScannerStarting] = useState(false);
  const [isUnlinking, setIsUnlinking] = useState(false);
  const videoRef = React.useRef<HTMLVideoElement>(null);
  const scannerControlsRef = React.useRef<IScannerControls | null>(null);
  const pairingInProgressRef = React.useRef(false);
  const autoReconnectAttemptedRef = React.useRef(false);
  const visibleConnectionError = isSetupRequiredMobileError(mobileConnectionError) ? null : mobileConnectionError;
  const hasPasskeySetup = hasMobilePasskeySetup();
  const showQrButton = !hasPasskeySetup || !isConnecting;

  const stopQrScanner = React.useCallback(() => {
    scannerControlsRef.current?.stop();
    scannerControlsRef.current = null;
    setIsScannerOpen(false);
    setIsScannerStarting(false);
  }, []);

  useEffect(() => () => stopQrScanner(), [stopQrScanner]);

  useEffect(() => {
    setHasPairingSetup(hasMobileCompanionSetup());
  }, [mobileConnectionError, mobileConnectionState]);

  useEffect(() => {
    if (!launchedAsPwa || !hasMobilePasskeySetup()) return;
    if (autoReconnectAttemptedRef.current) return;
    if (mobileConnectionState !== 'error') return;
    if (isConnecting) return;

    autoReconnectAttemptedRef.current = true;
    setConnectionHint('Session expirée. Reconnexion avec la clé d’accès...');
    connectMobileCompanion()
      .catch(() => {
        // The normal error UI remains available if the user cancels or the bridge is offline.
      })
      .finally(() => setConnectionHint(null));
  }, [connectMobileCompanion, isConnecting, launchedAsPwa, mobileConnectionState]);

  const applyPairingAndConnect = React.useCallback(async (value: string) => {
    if (pairingInProgressRef.current) return;

    setPairingError(null);
    const result = applyMobileCompanionPairingUrl(value);
    if (!result.ok) {
      setPairingError(result.error);
      return;
    }

    pairingInProgressRef.current = true;
    stopQrScanner();
    setPairingUrl(value);
    setHasPairingSetup(true);
    setIsPairingOpen(false);
    setIsManualPairingOpen(false);
    try {
      setIsApplyingPairing(true);
      setConnectionHint('QR détecté. Configuration de la clé d’accès...');
      await connectMobileCompanion();
    } finally {
      pairingInProgressRef.current = false;
      setConnectionHint(null);
      setIsApplyingPairing(false);
    }
  }, [connectMobileCompanion, stopQrScanner]);

  const handleScannedPairingUrl = (value: string) => {
    void applyPairingAndConnect(value);
  };

  const handleStartQrScanner = async () => {
    stopQrScanner();
    setPairingError(null);
    setIsManualPairingOpen(false);
    setIsPairingOpen(true);
    setIsScannerOpen(true);
    setIsScannerStarting(true);

    try {
      if (!navigator.mediaDevices?.getUserMedia) {
        throw new Error('Caméra indisponible dans ce navigateur.');
      }

      await new Promise(resolve => window.setTimeout(resolve, 80));
      if (!videoRef.current) throw new Error('Aperçu caméra indisponible.');

      const { BrowserQRCodeReader } = await import('@zxing/browser');
      const reader = new BrowserQRCodeReader();
      scannerControlsRef.current = await reader.decodeFromVideoDevice(undefined, videoRef.current, (result) => {
        if (!result) return;
        handleScannedPairingUrl(result.getText());
      });
    } catch (error) {
      stopQrScanner();
      setIsPairingOpen(true);
      setPairingError(error instanceof Error ? error.message : 'Impossible d’ouvrir la caméra.');
    } finally {
      setIsScannerStarting(false);
    }
  };

  const handlePastePairingUrl = async () => {
    setPairingError(null);
    setIsManualPairingOpen(true);
    try {
      const text = await navigator.clipboard.readText();
      setPairingUrl(text);
    } catch {
      setPairingError('Collage automatique refusé. Colle le lien manuellement dans le champ.');
    }
  };

  const handleApplyPairingUrl = async () => {
    await applyPairingAndConnect(pairingUrl);
  };

  const handleConnectWithPasskey = async () => {
    setPairingError(null);

    if (!hasPairingSetup) {
      await handleStartQrScanner();
      return;
    }

    try {
      setConnectionHint('Ouverture de la clé d’accès...');
      await connectMobileCompanion();
      setHasPairingSetup(hasMobileCompanionSetup());
      autoReconnectAttemptedRef.current = false;
    } finally {
      setConnectionHint(null);
    }
  };

  const handleToggleManualPairing = () => {
    setPairingError(null);
    if (!isManualPairingOpen) {
      stopQrScanner();
      setIsPairingOpen(false);
    }
    setIsManualPairingOpen(prev => !prev);
  };

  const handleUnlinkMobile = async () => {
    const confirmed = window.confirm(
      'Déconnecter cette PWA ? La session, les infos de pairing, le cache offline et les modifications en attente seront supprimés sur ce mobile.'
    );
    if (!confirmed) return;

    stopQrScanner();
    setIsUnlinking(true);
    try {
      await unlinkMobileCompanion();
      setPairingUrl('');
      setHasPairingSetup(false);
      setPairingError(null);
      setConnectionHint(null);
      setIsPairingOpen(false);
      setIsManualPairingOpen(false);
      autoReconnectAttemptedRef.current = false;
    } finally {
      setIsUnlinking(false);
    }
  };

  if (!launchedAsPwa) {
    return <MobileInstallGuide />;
  }

  return (
    <div className="h-[100dvh] overflow-y-auto overscroll-contain bg-[var(--color-bg-tertiary)] dark:bg-[var(--color-bg-tertiary)] text-gray-900 dark:text-gray-100 px-5 pt-[calc(env(safe-area-inset-top)+24px)] pb-[calc(env(safe-area-inset-bottom)+32px)]">
      <div className="mx-auto flex min-h-full w-full max-w-sm flex-col justify-center">
        <div className="flex flex-col items-center text-center">
          <img src={LOGO_PATH} alt="DmxMoney" className="w-20 h-20 rounded-2xl shadow-sm mb-5" />
          <div className={`inline-flex items-center gap-2 rounded-full px-3 py-1 text-[12px] font-semibold mb-4 ${
            visibleConnectionError
              ? 'bg-red-50 text-red-700 dark:bg-red-500/10 dark:text-red-300'
              : 'bg-primary-50 text-primary-700 dark:bg-primary-500/10 dark:text-primary-300'
          }`}
          >
            {visibleConnectionError ? <WifiOff className="w-3.5 h-3.5" /> : <ShieldCheck className="w-3.5 h-3.5" />}
            {visibleConnectionError ? 'Connexion à vérifier' : 'Liaison mobile'}
          </div>
          <h1 className="text-2xl font-bold tracking-tight">
            {hasPasskeySetup ? 'Reprendre la synchronisation' : hasPairingSetup ? 'Connecter ce mobile' : 'Liez ce mobile'}
          </h1>
          <p className="mt-2 text-sm leading-relaxed text-gray-500 dark:text-gray-400">
            {hasPasskeySetup
              ? 'Valide la clé d’accès enregistrée pour rouvrir la session mobile.'
              : hasPairingSetup
              ? 'Valide la clé d’accès pour reprendre la synchronisation.'
              : 'Scanne le QR affiché dans DmxMoney desktop pour synchroniser cette PWA.'}
          </p>
        </div>

        {!visibleConnectionError && (
          <div className="mt-5 rounded-2xl border border-primary-100 bg-primary-50 px-4 py-3 text-sm leading-relaxed text-primary-800 dark:border-primary-500/20 dark:bg-primary-500/10 dark:text-primary-200">
            {hasPasskeySetup
              ? 'Votre mobile est déjà lié. La clé d’accès est la méthode recommandée pour reprendre la session.'
              : hasPairingSetup
                ? 'Votre mobile est déjà lié. Vous pouvez scanner un nouveau QR ou vous reconnecter avec la clé d’accès.'
              : 'Liez votre mobile pour synchroniser vos données.'}
          </div>
        )}

        {!hasPairingSetup && !isPairingOpen && !isManualPairingOpen && <MobilePairingDemo />}

        {visibleConnectionError && (
          <div className="mt-5 rounded-xl border border-red-100 dark:border-red-500/20 bg-red-50 dark:bg-red-500/10 p-3 text-[12px] leading-relaxed text-red-700 dark:text-red-200">
            {visibleConnectionError}
          </div>
        )}

        {hasPasskeySetup && (
          <button
            type="button"
            onClick={handleConnectWithPasskey}
            disabled={isConnecting}
            className="mt-5 flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-primary-500 text-sm font-bold uppercase tracking-wide text-white shadow-sm transition-colors hover:bg-primary-600 disabled:bg-gray-200 disabled:text-gray-500 dark:disabled:bg-white/10"
          >
            {isConnecting ? <RefreshCw className="w-4 h-4 animate-spin" /> : <KeyRound className="w-4 h-4" />}
            {isConnecting ? 'Connexion...' : 'Connecter via clé d’accès'}
          </button>
        )}

        {showQrButton && (
          <button
            onClick={handleStartQrScanner}
            disabled={isConnecting || isScannerStarting}
            className={`${hasPasskeySetup ? 'mt-3 border border-gray-200 bg-white text-gray-800 shadow-sm dark:border-white/10 dark:bg-white/[0.04] dark:text-gray-100' : 'mt-5 bg-primary-500 text-white shadow-sm hover:bg-primary-600'} flex h-12 w-full items-center justify-center gap-2 rounded-xl text-sm font-bold uppercase tracking-wide transition-colors disabled:bg-gray-200 disabled:text-gray-500 dark:disabled:bg-white/10`}
          >
            {isConnecting || isScannerStarting
              ? <RefreshCw className="w-4 h-4 animate-spin" />
              : <Camera className="w-4 h-4" />}
            {isConnecting
              ? 'Connexion...'
              : isScannerStarting
                ? 'Ouverture caméra...'
                : hasPasskeySetup
                  ? 'Scanner un nouveau QR'
                  : 'Scanner le QR code'}
          </button>
        )}

        <button
          type="button"
          onClick={handleToggleManualPairing}
          className="mx-auto mt-3 text-sm font-medium text-gray-500 underline underline-offset-4 transition hover:text-gray-900 dark:text-gray-400 dark:hover:text-white"
        >
          {isManualPairingOpen ? 'Masquer le lien manuel' : 'Saisir le lien manuellement'}
        </button>

        {hasPairingSetup && !hasPasskeySetup && (
          <button
            type="button"
            onClick={handleConnectWithPasskey}
            disabled={isConnecting}
            className="mt-3 w-full h-11 rounded-xl border border-gray-200 dark:border-white/10 bg-white dark:bg-white/[0.04] text-gray-800 dark:text-gray-100 text-sm font-semibold flex items-center justify-center gap-2 shadow-sm active:scale-[0.99] transition disabled:opacity-60"
          >
            <KeyRound className="w-4 h-4" />
            Ou : connecter via clé d’accès
          </button>
        )}

        {connectionHint && (
          <div className="mt-3 rounded-xl border border-primary-100 dark:border-primary-500/20 bg-primary-50 dark:bg-primary-500/10 p-3 text-[12px] leading-relaxed text-primary-700 dark:text-primary-200">
            {connectionHint}
          </div>
        )}

        {isPairingOpen && (
          <div className="mt-4 rounded-2xl border border-black/[0.06] dark:border-white/10 bg-white dark:bg-[#121212] p-4 text-left shadow-sm">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary-50 text-primary-600 dark:bg-primary-500/10 dark:text-primary-300">
                <QrCode className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-bold text-gray-950 dark:text-white">Scanner le QR code</p>
                <p className="mt-1 text-[12px] leading-relaxed text-gray-500 dark:text-gray-400">
                  Autorise la caméra, puis pointe le téléphone vers le QR affiché dans l’application desktop.
                </p>
              </div>
            </div>

            <div className="mt-4 overflow-hidden rounded-2xl border border-gray-200 dark:border-white/10 bg-gray-950">
              {isScannerOpen ? (
                <div className="relative aspect-[4/3]">
                  <video
                    ref={videoRef}
                    muted
                    playsInline
                    className="h-full w-full object-cover"
                  />
                  <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
                    <div className="h-44 w-44 rounded-3xl border-2 border-white/90 shadow-[0_0_0_999px_rgba(0,0,0,0.28)]" />
                  </div>
                  <div className="absolute inset-x-0 top-0 flex items-center justify-between p-3">
                    <span className="rounded-full bg-black/50 px-3 py-1 text-[12px] font-semibold text-white backdrop-blur">
                      {isScannerStarting ? 'Ouverture caméra...' : 'Cadre le QR'}
                    </span>
                    <button
                      type="button"
                      onClick={stopQrScanner}
                      className="flex h-9 w-9 items-center justify-center rounded-full bg-black/50 text-white backdrop-blur"
                      aria-label="Fermer le scanner"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>
                </div>
              ) : (
                <button
                  type="button"
                  onClick={handleStartQrScanner}
                  className="flex min-h-36 w-full flex-col items-center justify-center gap-3 bg-gray-50 dark:bg-white/[0.04] px-4 py-6 text-center"
                >
                  <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary-50 text-primary-600 dark:bg-primary-500/10 dark:text-primary-300">
                    <Camera className="h-6 w-6" />
                  </div>
                  <div>
                    <p className="text-sm font-bold text-gray-950 dark:text-white">Relancer le scanner</p>
                    <p className="mt-1 text-[12px] leading-relaxed text-gray-500 dark:text-gray-400">
                      Utilise cette option si la caméra a été interrompue.
                    </p>
                  </div>
                </button>
              )}
            </div>

            {pairingError && (
              <div className="mt-3 rounded-xl border border-red-100 dark:border-red-500/20 bg-red-50 dark:bg-red-500/10 p-3 text-[12px] leading-relaxed text-red-700 dark:text-red-200">
                {pairingError}
              </div>
            )}

            <button
              type="button"
              onClick={stopQrScanner}
              className="mt-3 h-11 w-full rounded-xl border border-gray-200 dark:border-white/10 bg-white dark:bg-white/[0.04] text-sm font-semibold text-gray-700 dark:text-gray-200 flex items-center justify-center gap-2"
            >
              <VideoOff className="h-4 w-4" />
              Fermer le scanner
            </button>
          </div>
        )}

        {isManualPairingOpen && (
          <div className="mt-4 rounded-2xl border border-black/[0.06] dark:border-white/10 bg-white dark:bg-[#121212] p-4 text-left shadow-sm">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary-50 text-primary-600 dark:bg-primary-500/10 dark:text-primary-300">
                <Link2 className="h-4 w-4" />
              </div>
              <div className="min-w-0">
                <p className="text-sm font-bold text-gray-950 dark:text-white">Saisir le lien du QR</p>
                <p className="mt-1 text-[12px] leading-relaxed text-gray-500 dark:text-gray-400">
                  Colle le lien complet si la caméra n’est pas disponible.
                </p>
              </div>
            </div>

            <textarea
              value={pairingUrl}
              onChange={(event) => {
                setPairingUrl(event.target.value);
                setPairingError(null);
              }}
              placeholder="https://dmxmoney.develop-max.com/mobile#pairing=...&api=..."
              className="mt-4 min-h-24 w-full resize-none rounded-xl border border-gray-200 dark:border-white/10 bg-gray-50 dark:bg-black/20 px-3 py-2 text-sm text-gray-900 dark:text-white placeholder:text-gray-400 focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            />

            {pairingError && (
              <div className="mt-3 rounded-xl border border-red-100 dark:border-red-500/20 bg-red-50 dark:bg-red-500/10 p-3 text-[12px] leading-relaxed text-red-700 dark:text-red-200">
                {pairingError}
              </div>
            )}

            <div className="mt-3 grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={handlePastePairingUrl}
                className="h-11 rounded-xl border border-gray-200 dark:border-white/10 bg-white dark:bg-white/[0.04] text-sm font-semibold text-gray-700 dark:text-gray-200 flex items-center justify-center gap-2"
              >
                <ClipboardPaste className="h-4 w-4" />
                Coller
              </button>
              <button
                type="button"
                onClick={handleApplyPairingUrl}
                disabled={isApplyingPairing || isConnecting}
                className="h-11 rounded-xl bg-primary-500 text-sm font-semibold text-white flex items-center justify-center gap-2 disabled:bg-gray-200 disabled:text-gray-500 dark:disabled:bg-white/10"
              >
                {isApplyingPairing || isConnecting ? <RefreshCw className="h-4 w-4 animate-spin" /> : <KeyRound className="h-4 w-4" />}
                Configurer
              </button>
            </div>
          </div>
        )}

        {hasPairingSetup && (
          <button
            type="button"
            onClick={handleUnlinkMobile}
            disabled={isUnlinking || isConnecting}
            className="mt-4 w-full h-11 rounded-xl border border-red-100 dark:border-red-500/20 bg-red-50 dark:bg-red-500/10 text-red-600 dark:text-red-300 text-sm font-semibold flex items-center justify-center gap-2 disabled:opacity-60"
          >
            {isUnlinking ? <RefreshCw className="w-4 h-4 animate-spin" /> : <LogOut className="w-4 h-4" />}
            {isUnlinking ? 'Déconnexion...' : 'Déconnecter cette PWA'}
          </button>
        )}
      </div>
    </div>
  );
};

const AppContent: React.FC = () => {
  const { activePage, setActivePage } = useNavigation();
  const { settings, updateLastSeenVersion } = useSettings();
  const { mobileConnectionState } = useBank();
  const [showReleaseNotes, setShowReleaseNotes] = useState(false);

  // Initialize updater polling (silent check at startup + interval)
  useUpdater();

  // Check for new version at startup
  useEffect(() => {
    if (settings.lastSeenVersion !== undefined && settings.lastSeenVersion !== LATEST_VERSION) {
      // Small delay to ensure smooth transition
      const timer = setTimeout(() => {
        setShowReleaseNotes(true);
      }, 1000);
      return () => clearTimeout(timer);
    }
  }, [settings.lastSeenVersion]);

  const handleCloseReleaseNotes = async () => {
    setShowReleaseNotes(false);
    await updateLastSeenVersion(LATEST_VERSION);
  };

  if (isMobileCompanion() && mobileConnectionState !== 'connected' && mobileConnectionState !== 'offline') {
    return <MobileConnectionScreen />;
  }

  return (
    <>
      <Layout activePage={activePage} setActivePage={setActivePage}>
        {activePage === 'dashboard' && <Dashboard />}
        {activePage === 'accounts' && <Accounts />}
        {activePage === 'transactions' && <Transactions />}
        {activePage === 'scheduled' && <Scheduled />}
        {activePage === 'analytics' && <Analytics />}
        {activePage === 'predictions' && <Predictions />}
        {activePage === 'settings' && <Settings />}
        {activePage === 'budget' && <Budget />}
        {activePage === 'categories' && <Categories />}
      </Layout>

      <ReleaseNotesModal 
        isOpen={showReleaseNotes} 
        onClose={handleCloseReleaseNotes} 
      />
    </>
  );
};

function App() {
  return (
    <SettingsProvider>
      <ToastProvider>
        <BankProvider>
          <NavigationProvider>
            <AppContent />
          </NavigationProvider>
        </BankProvider>
      </ToastProvider>
    </SettingsProvider>
  );
}

export default App;
