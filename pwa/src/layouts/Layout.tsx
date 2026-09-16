import React, { useState } from 'react';
import { Wallet, LayoutDashboard, PieChart, TrendingUp, Settings, Receipt, CalendarClock, Tag, Calculator, ChevronLeft, ChevronRight, MoreHorizontal, RefreshCw, Wifi, WifiOff, Power, CheckCircle2, Sparkles } from 'lucide-react';
import { useBank } from '../context/BankContext';
import { useUpdater } from '../hooks/useUpdater';
import MultiSelect from '../components/ui/MultiSelect';
import TitleBar from '../components/ui/TitleBar';
import { useFinancialMetrics } from '../hooks/useFinancialMetrics';
import { formatCurrency } from '../utils/format';
import { hasTauriRuntime, isMobileCompanion } from '../utils/runtime';
import AssistantModal from '../components/AssistantModal';


interface LayoutProps {
  children: React.ReactNode;
  activePage: string;
  setActivePage: (page: string) => void;
}

type TrayNavigationPayload = string | {
  page?: string;
  accountId?: string | null;
};

const Layout: React.FC<LayoutProps> = ({ children, activePage, setActivePage }) => {
  const { accounts, filterAccount, setFilterAccount, mobileConnectionState } = useBank();
  const { currentBalance, checkedBalance } = useFinancialMetrics();
  const { updateAvailable } = useUpdater();
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [isPulling, setIsPulling] = useState(false);
  const [pullDistance, setPullDistance] = useState(0);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [sidebarTooltip, setSidebarTooltip] = useState<{ label: string; top: number } | null>(null);
  const [isScrolled, setIsScrolled] = useState(false);
  const [isAssistantOpen, setIsAssistantOpen] = useState(false);
  const mainRef = React.useRef<HTMLElement>(null);
  const pullStartXRef = React.useRef(0);
  const pullStartYRef = React.useRef(0);
  const pullDistanceRef = React.useRef(0);
  const isPullingRef = React.useRef(false);
  const pullGestureModeRef = React.useRef<'pending' | 'pull' | 'ignore'>('ignore');
  const isMobileMode = isMobileCompanion();
  const showTitleBar = hasTauriRuntime();
  const showQuitAction = showTitleBar && !isMobileMode;
  const pullThreshold = 96;

  const hasAccountFilter = ['dashboard', 'transactions', 'budget', 'analytics', 'predictions', 'scheduled'].includes(activePage);
  const hasBalanceWidget = ['dashboard', 'transactions'].includes(activePage);

  React.useEffect(() => {
    if (!hasTauriRuntime()) return;
    import('@tauri-apps/api/app').then(app => {
      app.getVersion().then(setAppVersion).catch(() => { });
    });
  }, []);

  React.useEffect(() => {
    if (!hasTauriRuntime()) return;

    let unlisten: (() => void) | undefined;

    import('@tauri-apps/api/event')
      .then(({ listen }) => listen<TrayNavigationPayload>('dmxmoney-navigate-to-page', (event) => {
        const payload = event.payload;
        const page = typeof payload === 'string' ? payload : payload?.page;

        if (page) {
          if (typeof payload !== 'string') {
            setFilterAccount(payload.accountId ? [payload.accountId] : []);
          }
          setActivePage(page);
          setIsMobileMenuOpen(false);
        }
      }))
      .then((cleanup) => {
        unlisten = cleanup;
      })
      .catch((error) => {
        console.error('Failed to listen for tray navigation:', error);
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, [setActivePage, setFilterAccount]);

  const navGroups = [
    {
      title: "Général",
      items: [
        { id: 'dashboard', label: 'Vue d\'ensemble', icon: LayoutDashboard },
        { id: 'accounts', label: 'Mes Comptes', icon: Wallet },
        { id: 'transactions', label: 'Journal', icon: Receipt },
      ]
    },
    {
      title: "Finances",
      items: [
        { id: 'budget', label: 'Budget', icon: Calculator },
        { id: 'scheduled', label: 'Échéancier', icon: CalendarClock },
      ]
    },
    {
      title: "Analyses",
      items: [
        { id: 'analytics', label: 'Analyses', icon: PieChart },
        { id: 'predictions', label: 'Prédictions', icon: TrendingUp },
      ]
    }
  ];

  const desktopFooterItems = [
    { id: 'categories', label: 'Catégories', icon: Tag },
    { id: 'settings', label: 'Paramètres', icon: Settings },
  ];

  const mobilePrimaryItems = [
    { id: 'dashboard', label: 'Accueil', icon: LayoutDashboard },
    { id: 'accounts', label: 'Comptes', icon: Wallet },
    { id: 'transactions', label: 'Journal', icon: Receipt },
    { id: 'budget', label: 'Budget', icon: Calculator },
  ];

  const mobileMoreItems = [
    { id: 'scheduled', label: 'Échéancier', icon: CalendarClock },
    { id: 'analytics', label: 'Analyses', icon: PieChart },
    { id: 'predictions', label: 'Prédictions', icon: TrendingUp },
    ...desktopFooterItems,
  ];

  // Pastilles de couleur des réglages iOS pour les pages du menu « Plus ».
  const mobileMoreColors: Record<string, string> = {
    scheduled: '#FF9500',
    analytics: '#AF52DE',
    predictions: '#5856D6',
    categories: '#FF2D55',
    settings: '#8E8E93',
  };

  const isMoreActive = mobileMoreItems.some(item => item.id === activePage);
  const pageTitle = [
    ...navGroups.flatMap(group => group.items),
    ...desktopFooterItems,
  ].find(item => item.id === activePage)?.label || 'DmxMoney';
  // Sur iPhone, le grand titre reprend le nom de l'onglet (« Accueil », « Comptes »…).
  const mobileTitle = mobilePrimaryItems.find(item => item.id === activePage)?.label || pageTitle;
  const mobileSyncLabel = mobileConnectionState === 'offline' ? 'Hors ligne' : 'Connecté';
  const MobileSyncIcon = mobileConnectionState === 'offline' ? WifiOff : Wifi;

  const navigateToPage = (page: string) => {
    setActivePage(page);
    setIsMobileMenuOpen(false);
  };

  // Barre de navigation iOS : le petit titre apparaît quand le grand titre sort de l'écran.
  const handleMainScroll = (event: React.UIEvent<HTMLElement>) => {
    setIsScrolled(event.currentTarget.scrollTop > 40);
  };

  // Chaque page s'ouvre en haut, avec son grand titre.
  React.useEffect(() => {
    if (mainRef.current) mainRef.current.scrollTop = 0;
  }, [activePage]);

  const handleQuitApp = React.useCallback(async () => {
    if (!hasTauriRuntime()) return;

    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('quit_app');
    } catch (error) {
      console.error('Failed to quit DmxMoney:', error);
    }
  }, []);

  const setPullDistanceValue = (value: number) => {
    pullDistanceRef.current = value;
    setPullDistance(value);
  };

  const resetPullRefreshGesture = () => {
    isPullingRef.current = false;
    pullGestureModeRef.current = 'ignore';
    setIsPulling(false);
    setPullDistanceValue(0);
  };

  const hasScrollableAncestorBeforeMain = (target: HTMLElement, main: HTMLElement) => {
    let current: HTMLElement | null = target;

    while (current && current !== main) {
      const style = window.getComputedStyle(current);
      const canScrollY = /(auto|scroll|overlay)/.test(style.overflowY)
        && current.scrollHeight > current.clientHeight + 1;
      const canScrollX = /(auto|scroll|overlay)/.test(style.overflowX)
        && current.scrollWidth > current.clientWidth + 1;

      if (canScrollY || canScrollX) return true;
      current = current.parentElement;
    }

    return false;
  };

  const shouldIgnorePullRefreshTarget = (target: HTMLElement | null) => {
    const main = mainRef.current;
    if (!target || !main || !main.contains(target)) return true;
    if (main.scrollTop > 1) return true;
    if (target.closest([
      '[data-no-pull-refresh="true"]',
      '.app-modal-overlay',
      '.app-modal-content',
      '.app-modal-body',
      '.app-form-popup-content',
      'table',
      'thead',
      'tbody',
      'tr',
      'td',
      'th',
      'input',
      'textarea',
      'select',
      'button',
      '[role="button"]',
      '[contenteditable="true"]'
    ].join(','))) return true;

    return hasScrollableAncestorBeforeMain(target, main);
  };

  const handleTouchStart = (event: React.TouchEvent<HTMLElement>) => {
    if (isRefreshing || isMobileMenuOpen) return;
    const target = event.target as HTMLElement | null;
    if (shouldIgnorePullRefreshTarget(target)) {
      resetPullRefreshGesture();
      return;
    }

    pullStartXRef.current = event.touches[0]?.clientX || 0;
    pullStartYRef.current = event.touches[0]?.clientY || 0;
    isPullingRef.current = true;
    pullGestureModeRef.current = 'pending';
  };

  const handleTouchMove = (event: React.TouchEvent<HTMLElement>) => {
    if (!isPullingRef.current || isRefreshing) return;
    if ((mainRef.current?.scrollTop || 0) > 1) {
      resetPullRefreshGesture();
      return;
    }

    const currentX = event.touches[0]?.clientX || 0;
    const currentY = event.touches[0]?.clientY || 0;
    const deltaX = currentX - pullStartXRef.current;
    const delta = currentY - pullStartYRef.current;

    if (pullGestureModeRef.current === 'pending') {
      if (Math.abs(deltaX) > Math.abs(delta) || Math.abs(deltaX) > 10) {
        pullGestureModeRef.current = 'ignore';
        isPullingRef.current = false;
        setPullDistanceValue(0);
        return;
      }
      if (delta > 18) {
        pullGestureModeRef.current = 'pull';
        setIsPulling(true);
      }
    }

    if (pullGestureModeRef.current !== 'pull') return;

    if (delta <= 0) {
      setPullDistanceValue(0);
      return;
    }

    const nextDistance = Math.min(112, Math.pow(delta, 0.86) * 1.45);
    setPullDistanceValue(nextDistance);

    if (nextDistance > 6 && event.cancelable) {
      event.preventDefault();
    }
  };

  const handleTouchEnd = () => {
    if (!isPullingRef.current) return;
    isPullingRef.current = false;
    pullGestureModeRef.current = 'ignore';
    setIsPulling(false);

    if (pullDistanceRef.current < pullThreshold) {
      setPullDistanceValue(0);
      return;
    }

    setIsRefreshing(true);
    setPullDistanceValue(pullThreshold);
    window.setTimeout(() => {
      window.location.reload();
    }, 120);
  };

  const showSidebarTooltip = (label: string, element: HTMLElement) => {
    if (!isCollapsed || isMobileMode) return;

    const rect = element.getBoundingClientRect();
    setSidebarTooltip({
      label,
      top: rect.top + rect.height / 2
    });
  };

  const hideSidebarTooltip = () => setSidebarTooltip(null);

  return (
    <div className="relative flex h-[100dvh] w-screen flex-col md:flex-row text-gray-900 dark:text-gray-100 font-sans overflow-hidden bg-[var(--color-bg-primary)] dark:bg-[var(--color-bg-primary)]">

      {showTitleBar && <TitleBar />}

      {/* Sidebar */}
      <aside className={`hidden md:flex flex-shrink-0 flex-col bg-[var(--color-bg-secondary)] dark:bg-[var(--color-bg-secondary)] border-r border-black/[0.05] dark:border-white/10 z-40 transition-all duration-300 ${isCollapsed ? 'w-[82px]' : 'w-56'}`}>
        <div className={`h-16 w-full flex-shrink-0 flex ${isCollapsed ? 'relative' : 'items-center justify-end px-4'}`} data-tauri-drag-region>
          <button
            onClick={() => setIsCollapsed(!isCollapsed)}
            className={`p-1.5 rounded-lg hover:bg-gray-200 dark:hover:bg-neutral-900 text-gray-400 transition-colors ${isCollapsed ? 'absolute left-[82px] top-[32px] z-50 -translate-y-1/2' : ''}`}
            aria-label={isCollapsed ? 'Agrandir la barre latérale' : 'Réduire la barre latérale'}
          >
            {isCollapsed ? <ChevronRight className="w-4 h-4" /> : <ChevronLeft className="w-4 h-4" />}
          </button>
        </div>

        <div className="flex-1 overflow-y-auto overflow-x-hidden px-3 py-4 scrollbar-hide">
          {navGroups.map((group, idx) => (
            <div key={idx} className={idx > 0 ? "mt-6" : ""}>
              <h3
                onMouseEnter={(event) => showSidebarTooltip(group.title, event.currentTarget)}
                onMouseLeave={hideSidebarTooltip}
                className={`px-4 text-[10px] font-bold text-gray-400 dark:text-gray-600 uppercase tracking-widest mb-2 truncate animate-in fade-in duration-300 ${isCollapsed ? 'px-1 text-center tracking-normal cursor-default' : ''}`}
              >
                {group.title}
              </h3>
              <div className="space-y-0.5">
                {group.items.map((item) => {
                  const Icon = item.icon;
                  const isActive = activePage === item.id;
                  return (
                    <button
                      key={item.id}
                      onClick={() => setActivePage(item.id)}
                      onMouseEnter={(event) => showSidebarTooltip(item.label, event.currentTarget)}
                      onMouseLeave={hideSidebarTooltip}
                      onFocus={(event) => showSidebarTooltip(item.label, event.currentTarget)}
                      onBlur={hideSidebarTooltip}
                      aria-label={item.label}
                      className={`w-full flex items-center gap-3 px-4 py-1.5 rounded-lg text-[13px] font-medium transition-all cursor-pointer ${isActive
                        ? 'bg-primary-500 text-white shadow-sm'
                        : 'text-gray-600 dark:text-neutral-400 hover:bg-gray-200 dark:hover:bg-neutral-900'
                        } ${isCollapsed ? 'justify-center px-0' : ''}`}
                    >
                      <Icon className="w-4 h-4 shrink-0" />
                      {!isCollapsed && <span className="min-w-0 truncate">{item.label}</span>}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        <div className="p-4 border-t border-black/[0.05] dark:border-white/10 overflow-x-hidden">
          <div className="space-y-0.5">
            {desktopFooterItems.map((item) => {
              const Icon = item.icon;
              const isActive = activePage === item.id;
              return (
                <button
                  key={item.id}
                  onClick={() => setActivePage(item.id)}
                  onMouseEnter={(event) => showSidebarTooltip(item.label, event.currentTarget)}
                  onMouseLeave={hideSidebarTooltip}
                  onFocus={(event) => showSidebarTooltip(item.label, event.currentTarget)}
                  onBlur={hideSidebarTooltip}
                  aria-label={item.label}
                  className={`w-full flex items-center gap-3 px-4 py-1.5 rounded-lg text-[13px] font-medium transition-all cursor-pointer relative ${isActive ? 'bg-primary-500 text-white' : 'text-gray-600 dark:text-neutral-400 hover:bg-gray-200 dark:hover:bg-neutral-900'
                    } ${isCollapsed ? 'justify-center px-0' : ''}`}
                >
                  <div className="relative">
                    <Icon className="w-4 h-4 shrink-0" />
                    {item.id === 'settings' && updateAvailable && (
                      <span className="absolute -top-0.5 -right-0.5 w-2 h-2 bg-red-500 rounded-full border border-white dark:border-black animate-pulse" />
                    )}
                  </div>
                  {!isCollapsed && <div className="min-w-0 flex-1 flex justify-between items-center">
                    <span className="min-w-0 truncate">{item.label}</span>
                    {item.id === 'settings' && updateAvailable && (
                      <span className="w-2 h-2 bg-red-500 rounded-full" title="Mise à jour disponible" />
                    )}
                  </div>}
                </button>
              );
            })}
            {showQuitAction && (
              <button
                onClick={() => void handleQuitApp()}
                onMouseEnter={(event) => showSidebarTooltip('Quitter', event.currentTarget)}
                onMouseLeave={hideSidebarTooltip}
                onFocus={(event) => showSidebarTooltip('Quitter', event.currentTarget)}
                onBlur={hideSidebarTooltip}
                aria-label="Quitter"
                className={`w-full flex items-center gap-3 px-4 py-1.5 rounded-lg text-[13px] font-medium transition-all cursor-pointer text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 ${isCollapsed ? 'justify-center px-0' : ''}`}
              >
                <Power className="w-4 h-4 shrink-0" />
                {!isCollapsed && <span className="min-w-0 truncate">Quitter</span>}
              </button>
            )}
          </div>
          {!isCollapsed && (
            <div className="mt-4 text-[9px] text-gray-400 text-center font-bold uppercase tracking-widest opacity-60 animate-in fade-in duration-500">
              DmxMoney{appVersion ? ` • v${appVersion}` : ''}
            </div>
          )}
        </div>
      </aside>

      {isCollapsed && sidebarTooltip && (
        <div
          className="fixed left-[94px] z-[80] -translate-y-1/2 rounded-md border border-black/10 dark:border-white/10 bg-white dark:bg-neutral-900 px-2.5 py-1.5 text-xs font-medium text-gray-900 dark:text-gray-100 shadow-lg pointer-events-none whitespace-nowrap"
          style={{ top: sidebarTooltip.top }}
        >
          {sidebarTooltip.label}
        </div>
      )}

      {/* Main Area */}
      <div
        className="flex-1 flex flex-col min-w-0 min-h-0 bg-[var(--color-bg-tertiary)] dark:bg-[var(--color-bg-tertiary)] overflow-hidden"
        onTouchStart={handleTouchStart}
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onTouchCancel={() => {
          isPullingRef.current = false;
          pullGestureModeRef.current = 'ignore';
          setIsPulling(false);
          setPullDistanceValue(0);
        }}
      >
        <header className="hidden min-h-16 flex-shrink-0 md:flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-4 sm:px-8 py-3 md:py-0 bg-[var(--color-bg-tertiary)] dark:bg-[var(--color-bg-tertiary)] z-30 border-b border-black/[0.05] dark:border-white/10" data-tauri-drag-region>
          <div className="flex items-center gap-4 min-w-0">
            <span className="text-xs font-medium text-gray-500 hidden sm:inline">Compte :</span>
            <MultiSelect
              value={filterAccount}
              onChange={setFilterAccount}
              options={accounts.map(acc => ({ id: acc.id, label: acc.name, icon: acc.icon, color: acc.color }))}
              placeholder="Tous les comptes"
              className="w-full sm:w-64"
            />
          </div>

          <div className="flex items-center justify-between sm:justify-end gap-4 sm:gap-10">
            <div className="text-right">
              <div className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-0.5">Pointé</div>
              <div className="text-sm sm:text-lg font-bold text-emerald-600">
                {formatCurrency(checkedBalance)}
              </div>
            </div>
            <div className="h-8 w-px bg-gray-200 dark:bg-neutral-700 opacity-50 hidden xs:block"></div>
            <button
              type="button"
              onClick={() => setIsAssistantOpen(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-primary-500/10 text-primary-600 dark:text-primary-400 hover:bg-primary-500/20 text-xs font-semibold transition-colors cursor-pointer"
              title="Ouvrir l'assistant"
            >
              <Sparkles className="h-3.5 w-3.5" />
              <span>Assistant</span>
            </button>
            <div className="text-right">
              <div className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-0.5">Actuel</div>
              <div className="text-base sm:text-xl font-bold text-gray-900 dark:text-gray-100">
                {formatCurrency(currentBalance)}
              </div>
            </div>
          </div>
        </header>

        {/* Barre de navigation iOS : transparente en haut de page, en verre dépoli dès que la page défile. */}
        <header
          className={`md:hidden fixed inset-x-0 top-0 z-40 pt-[env(safe-area-inset-top)] transition-[background-color,box-shadow] duration-200 ${
            isScrolled ? 'ios-material shadow-[inset_0_-0.5px_0_var(--ios-separator)]' : 'bg-transparent'
          }`}
        >
          <div className="relative flex h-11 items-center justify-center px-28">
            <div
              className={`truncate text-[17px] font-semibold text-[var(--ios-label)] transition-opacity duration-200 ${isScrolled ? 'opacity-100' : 'opacity-0'}`}
              aria-hidden="true"
            >
              {mobileTitle}
            </div>
            {isMobileMode && (
              <span className={`absolute right-4 inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[13px] font-medium ${
                mobileConnectionState === 'offline'
                  ? 'bg-[#FF9500]/15 text-[#C93400] dark:text-[#FF9F0A]'
                  : 'bg-[var(--ios-fill-tertiary)] text-[var(--ios-secondary-label)]'
              }`}>
                <MobileSyncIcon className="h-3.5 w-3.5" />
                {mobileSyncLabel}
              </span>
            )}
          </div>
        </header>

        {/* Bouton assistant mobile : accessible partout, placé sous le badge de connexion */}
        {isMobileMode && (
          <button
            type="button"
            onClick={() => setIsAssistantOpen(true)}
            aria-label="Ouvrir l'assistant"
            title="Assistant DmxMoney"
            className="md:hidden fixed right-4 top-[calc(env(safe-area-inset-top)+48px)] z-40 flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-primary-500 text-white shadow-lg shadow-primary-500/30 active:scale-95 transition-transform cursor-pointer text-[12px] font-semibold"
          >
            <Sparkles className="h-3.5 w-3.5 shrink-0" />
            <span>Assistant</span>
          </button>
        )}

        <main
          ref={mainRef}
          onScroll={handleMainScroll}
          className="relative flex-1 overflow-y-auto overscroll-y-contain scrollbar-thin px-4 pt-[calc(env(safe-area-inset-top)+44px)] pb-[calc(104px+env(safe-area-inset-bottom))] md:px-8 md:py-4 md:pb-4"
        >
          <div
            className="pointer-events-none sticky top-2 z-30 flex h-0 justify-center md:hidden"
            style={{
              opacity: isRefreshing || pullDistance > 8 ? 1 : 0,
              transform: `translateY(${Math.min(8, pullDistance * 0.08)}px)`,
            }}
            aria-hidden={!isRefreshing}
          >
            <RefreshCw
              className={`h-5 w-5 text-[var(--ios-secondary-label)] ${isRefreshing ? 'animate-spin' : ''}`}
              style={!isRefreshing ? { transform: `rotate(${Math.min(180, (pullDistance / pullThreshold) * 180)}deg)` } : undefined}
            />
          </div>

          <div
            className="w-full md:max-w-7xl md:mx-auto"
            style={{
              transform: `translateY(${isRefreshing ? 28 : Math.min(34, pullDistance * 0.38)}px)`,
              transition: isPulling ? 'none' : 'transform 200ms ease-out',
            }}
          >
            {/* Grand titre iOS, solde et filtre des comptes : ils défilent avec la page. */}
            <div className="md:hidden">
              <h1 className="truncate pb-3 text-[34px] font-bold leading-[41px] tracking-[-0.022em] text-[var(--ios-label)]">
                {mobileTitle}
              </h1>

              {hasBalanceWidget && (
                <div className="mb-4 rounded-[22px] bg-[var(--ios-card)] px-4 py-3.5">
                  <div className="text-[13px] font-medium text-[var(--ios-secondary-label)]">Solde actuel</div>
                  <div className="mt-0.5 text-[34px] font-bold leading-[40px] tracking-[-0.022em] tabular-nums text-[var(--ios-label)]">
                    {formatCurrency(currentBalance)}
                  </div>
                  <div className="mt-1.5 flex items-center gap-1.5 text-[15px] text-[var(--ios-secondary-label)]">
                    <CheckCircle2 className="h-4 w-4 text-[#34C759]" />
                    <span>Pointé</span>
                    <span className="font-semibold tabular-nums text-[var(--ios-label)]">{formatCurrency(checkedBalance)}</span>
                  </div>
                </div>
              )}

              {hasAccountFilter && accounts.length > 0 && (
                <div className="-mx-4 mb-4 flex gap-2 overflow-x-auto px-4 pb-0.5 scrollbar-hide" data-no-pull-refresh="true">
                  <button
                    onClick={() => setFilterAccount([])}
                    className={`shrink-0 rounded-full px-4 py-[7px] text-[15px] font-medium transition-colors cursor-pointer ${
                      filterAccount.length === 0 ? 'bg-primary-500 text-white' : 'bg-[var(--ios-card)] text-[var(--ios-label)]'
                    }`}
                    aria-pressed={filterAccount.length === 0}
                  >
                    Tous
                  </button>
                  {accounts.map(acc => {
                    const isSelected = filterAccount.includes(acc.id);
                    return (
                      <button
                        key={acc.id}
                        onClick={() => setFilterAccount(isSelected ? filterAccount.filter(id => id !== acc.id) : [...filterAccount, acc.id])}
                        className={`inline-flex shrink-0 items-center gap-2 rounded-full px-4 py-[7px] text-[15px] font-medium transition-colors cursor-pointer ${
                          isSelected ? 'text-white' : 'bg-[var(--ios-card)] text-[var(--ios-label)]'
                        }`}
                        style={isSelected ? { backgroundColor: acc.color } : undefined}
                        aria-pressed={isSelected}
                      >
                        {!isSelected && <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: acc.color }} />}
                        {acc.name}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            {children}
          </div>
        </main>
      </div>

      {isMobileMenuOpen && (
        <div className="fixed inset-0 z-[70] md:hidden">
          <button
            className="absolute inset-0 bg-black/30 animate-backdrop-fade-in"
            onClick={() => setIsMobileMenuOpen(false)}
            aria-label="Fermer le menu"
          />
          <div className="absolute inset-x-0 bottom-0 rounded-t-[28px] bg-[var(--ios-grouped-bg)] pb-[calc(env(safe-area-inset-bottom)+20px)] shadow-[0_-10px_40px_rgba(0,0,0,0.12)] animate-bottom-sheet-slide-in">
            <div className="flex justify-center pt-2">
              <div className="h-[5px] w-9 rounded-full bg-[var(--ios-fill)]" />
            </div>
            <div className="relative flex h-12 items-center justify-center px-4">
              <span className="text-[17px] font-semibold">Plus</span>
              <button
                onClick={() => setIsMobileMenuOpen(false)}
                className="absolute right-4 text-[17px] font-semibold text-primary-500 cursor-pointer"
              >
                OK
              </button>
            </div>
            <div className="mx-4 mt-1 overflow-hidden rounded-[22px] bg-[var(--ios-card)]">
              {mobileMoreItems.map((item, index) => {
                const Icon = item.icon;
                const isActive = activePage === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => navigateToPage(item.id)}
                    className="flex w-full items-center gap-3 pl-4 text-left transition-colors active:bg-[var(--ios-fill-tertiary)] cursor-pointer"
                    aria-current={isActive ? 'page' : undefined}
                  >
                    <span
                      className="flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-[8px] text-white"
                      style={{ backgroundColor: mobileMoreColors[item.id] }}
                    >
                      <Icon className="h-[18px] w-[18px]" strokeWidth={2} />
                    </span>
                    <span className={`flex min-h-[52px] flex-1 items-center justify-between gap-2 pr-4 ${index > 0 ? 'border-t-[0.5px] border-[var(--ios-separator)]' : ''}`}>
                      <span className={`text-[17px] ${isActive ? 'font-semibold text-primary-500' : 'text-[var(--ios-label)]'}`}>{item.label}</span>
                      <span className="flex items-center gap-2">
                        {item.id === 'settings' && updateAvailable && (
                          <span className="rounded-full bg-[#FF3B30] px-2 py-0.5 text-[12px] font-semibold text-white">1</span>
                        )}
                        <ChevronRight className="h-[18px] w-[18px] text-[var(--ios-tertiary-label)]" />
                      </span>
                    </span>
                  </button>
                );
              })}
            </div>
            {showQuitAction && (
              <div className="mx-4 mt-4 overflow-hidden rounded-[22px] bg-[var(--ios-card)]">
                <button
                  onClick={() => {
                    setIsMobileMenuOpen(false);
                    void handleQuitApp();
                  }}
                  className="flex min-h-[52px] w-full items-center justify-center text-[17px] text-[#FF3B30] cursor-pointer"
                >
                  Quitter
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      <nav className="md:hidden fixed inset-x-0 bottom-0 z-[60] px-4 pb-[max(env(safe-area-inset-bottom),12px)] pointer-events-none" aria-label="Navigation principale">
        <div className="ios-material pointer-events-auto mx-auto flex h-[62px] max-w-md items-stretch gap-0.5 rounded-full border border-[var(--ios-material-border)] p-[5px] shadow-[0_10px_30px_rgba(0,0,0,0.14)]">
          {mobilePrimaryItems.map((item) => {
            const Icon = item.icon;
            const isActive = activePage === item.id;
            return (
              <button
                key={item.id}
                onClick={() => navigateToPage(item.id)}
                className={`relative flex min-w-0 flex-1 flex-col items-center justify-center gap-[3px] rounded-full text-[10.5px] font-medium leading-none transition-colors cursor-pointer ${
                  isActive ? 'bg-[var(--ios-fill-tertiary)] text-primary-500' : 'text-[var(--ios-label)]'
                }`}
                aria-current={isActive ? 'page' : undefined}
              >
                <Icon className="h-[22px] w-[22px] shrink-0" strokeWidth={isActive ? 2.2 : 1.8} />
                <span className="w-full truncate px-0.5">{item.label}</span>
              </button>
            );
          })}
          <button
            onClick={() => setIsMobileMenuOpen(prev => !prev)}
            className={`relative flex min-w-0 flex-1 flex-col items-center justify-center gap-[3px] rounded-full text-[10.5px] font-medium leading-none transition-colors cursor-pointer ${
              isMoreActive || isMobileMenuOpen ? 'bg-[var(--ios-fill-tertiary)] text-primary-500' : 'text-[var(--ios-label)]'
            }`}
            aria-expanded={isMobileMenuOpen}
            aria-current={isMoreActive ? 'page' : undefined}
          >
            <span className="relative">
              <MoreHorizontal className="h-[22px] w-[22px] shrink-0" strokeWidth={isMoreActive ? 2.2 : 1.8} />
              {updateAvailable && (
                <span className="absolute -right-1 -top-1 h-2 w-2 rounded-full bg-[#FF3B30]" />
              )}
            </span>
            <span className="w-full truncate px-0.5">Plus</span>
          </button>
        </div>
      </nav>

      {/* Assistant vocal universel modal / feuille mobile */}
      <AssistantModal
        isOpen={isAssistantOpen}
        onClose={() => setIsAssistantOpen(false)}
      />
    </div>
  );
};

export default Layout;
