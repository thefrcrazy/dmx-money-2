import React, { useState } from 'react';
import { Wallet, LayoutDashboard, PieChart, TrendingUp, Settings, Receipt, CalendarClock, Tag, Calculator, ChevronLeft, ChevronRight, MoreHorizontal, X, RefreshCw, Wifi, WifiOff, Power } from 'lucide-react';
import { useBank } from '../context/BankContext';
import { useUpdater } from '../hooks/useUpdater';
import MultiSelect from '../components/ui/MultiSelect';
import TitleBar from '../components/ui/TitleBar';
import { useFinancialMetrics } from '../hooks/useFinancialMetrics';
import { formatCurrency } from '../utils/format';
import { hasTauriRuntime, isMobileCompanion } from '../utils/runtime';
import { ICONS } from '../constants/icons';


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

  const isMoreActive = mobileMoreItems.some(item => item.id === activePage);
  const pageTitle = [
    ...navGroups.flatMap(group => group.items),
    ...desktopFooterItems,
  ].find(item => item.id === activePage)?.label || 'DmxMoney';
  const mobileSyncLabel = mobileConnectionState === 'offline' ? 'Offline' : 'Sync';
  const MobileSyncIcon = mobileConnectionState === 'offline' ? WifiOff : Wifi;

  const navigateToPage = (page: string) => {
    setActivePage(page);
    setIsMobileMenuOpen(false);
  };

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
      {/* Glow Orbs cosmiques d'arrière-plan Gemini (masqués sur desktop) */}
      <div className="absolute top-[-15%] left-[-15%] w-[60%] aspect-square rounded-full bg-gradient-to-br from-indigo-500/10 via-purple-500/8 to-pink-500/5 dark:from-indigo-500/15 dark:via-purple-500/10 dark:to-transparent blur-[140px] pointer-events-none z-0 animate-pulse duration-[10s] md:hidden" />
      <div className="absolute bottom-[-15%] right-[-15%] w-[60%] aspect-square rounded-full bg-gradient-to-br from-pink-500/5 via-cyan-500/8 to-indigo-500/10 dark:from-purple-500/8 dark:via-cyan-500/10 dark:to-transparent blur-[140px] pointer-events-none z-0 animate-pulse duration-[10s] md:hidden" />

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
            <div className="text-right">
              <div className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-0.5">Actuel</div>
              <div className="text-base sm:text-xl font-bold text-gray-900 dark:text-gray-100">
                {formatCurrency(currentBalance)}
              </div>
            </div>
          </div>
        </header>

        <header className={`md:hidden flex-shrink-0 bg-white/90 dark:bg-neutral-950/90 backdrop-blur-2xl border-b border-black/[0.05] dark:border-white/10 px-4 pt-[calc(env(safe-area-inset-top)+8px)] pb-3 z-40 transition-all duration-300 ${
          hasAccountFilter ? '' : '!pb-1.5'
        }`}>
          {/* Barre de navigation style iOS avec titre centré */}
          <div className="relative flex items-center justify-between min-h-[32px] w-full">
            {/* Espace vide à gauche pour laisser la place aux contrôles Tauri sans aucun chevauchement */}
            <div className="w-16 h-1 flex-shrink-0 z-20" />
            
            {/* Titre centré absolu de la barre de titre */}
            <div className="absolute inset-x-0 flex flex-col items-center justify-center text-center pointer-events-none z-10">
              <span className="text-[9px] font-extrabold uppercase tracking-[0.22em] text-gray-400 dark:text-neutral-500 leading-none">DmxMoney</span>
              <h1 className="text-[16px] font-black tracking-tight text-gray-950 dark:text-white truncate leading-tight mt-0.5 pointer-events-auto">
                {pageTitle}
              </h1>
            </div>

            {/* Badge de synchro positionné à l'extrême droite */}
            <div className="z-20 min-w-16 flex justify-end">
              {isMobileMode && (
                <div className={`inline-flex shrink-0 items-center gap-1 rounded-full px-2.5 py-0.5 text-[9px] font-extrabold uppercase tracking-wider ${
                  mobileConnectionState === 'offline'
                    ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400'
                    : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                }`}>
                  <MobileSyncIcon className="h-2.5 w-2.5" />
                  {mobileSyncLabel}
                </div>
              )}
            </div>
          </div>

          {/* Widget de solde unifié style iOS Wallet (uniquement Dashboard & Journal) */}
          {hasBalanceWidget && (
            <div className="mt-3 bg-gradient-to-br from-gray-50 to-gray-100 dark:from-white/[0.01] dark:to-white/[0.03] rounded-2xl border border-black/[0.04] dark:border-white/[0.06] px-4 py-2.5 flex items-center justify-between shadow-sm relative overflow-hidden">
              <div className="absolute -right-6 -bottom-6 w-24 h-24 rounded-full bg-primary-500/5 blur-2xl pointer-events-none" />
              <div>
                <span className="text-[9px] font-extrabold uppercase tracking-[0.15em] text-gray-400 dark:text-neutral-500">Solde Actuel</span>
                <div className="text-[20px] font-extrabold tracking-tight text-gray-950 dark:text-white leading-none mt-0.5">
                  {formatCurrency(currentBalance)}
                </div>
              </div>
              <div className="text-right flex flex-col items-end justify-center">
                <div className="inline-flex items-center gap-1 bg-emerald-500/10 dark:bg-emerald-500/20 px-2 py-0.5 rounded-lg text-emerald-700 dark:text-emerald-400 border border-emerald-500/10">
                  <span className="text-[7.5px] font-extrabold uppercase tracking-wider">Pointé</span>
                  <span className="text-[11px] font-bold font-mono">
                    {formatCurrency(checkedBalance)}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* Filtre horizontal des comptes tactile (Chips) */}
          {hasAccountFilter && (
            <div className="mt-3 -mx-4 px-4 overflow-x-auto whitespace-nowrap scrollbar-hide flex gap-1.5 py-0.5" data-no-pull-refresh="true">
              <button
                onClick={() => setFilterAccount([])}
                className={`inline-flex items-center gap-1 px-3 py-1.5 rounded-full text-[10px] font-extrabold tracking-wider uppercase transition-all tap-bounce cursor-pointer ${
                  filterAccount.length === 0
                    ? 'bg-primary-500 text-white shadow-sm shadow-primary-500/20'
                    : 'bg-gray-100 dark:bg-neutral-900 text-gray-600 dark:text-neutral-400 border border-black/[0.03] dark:border-white/[0.02]'
                }`}
              >
                Tous
              </button>
              {accounts.map(acc => {
                const isSelected = filterAccount.includes(acc.id);
                const Icon = ICONS[acc.icon || 'Wallet'] || Wallet;
                return (
                  <button
                    key={acc.id}
                    onClick={() => {
                      if (isSelected) {
                        setFilterAccount(filterAccount.filter(id => id !== acc.id));
                      } else {
                        setFilterAccount([...filterAccount, acc.id]);
                      }
                    }}
                    className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[10px] font-extrabold tracking-wider uppercase transition-all tap-bounce cursor-pointer border ${
                      isSelected
                        ? 'text-white shadow-sm'
                        : 'bg-gray-100 dark:bg-neutral-900 text-gray-600 dark:text-neutral-400 border-black/[0.03] dark:border-white/[0.02]'
                    }`}
                    style={{
                      backgroundColor: isSelected ? acc.color : undefined,
                      borderColor: isSelected ? acc.color : undefined,
                      boxShadow: isSelected ? `0 4px 10px ${acc.color}25` : undefined
                    }}
                  >
                    <Icon className="w-3.5 h-3.5 shrink-0" />
                    <span>{acc.name}</span>
                  </button>
                );
              })}
            </div>
          )}
        </header>

        <main
          ref={mainRef}
          className="relative flex-1 overflow-y-auto overscroll-y-contain scrollbar-thin px-4 py-4 pb-[calc(96px+env(safe-area-inset-bottom))] md:px-8 md:py-4 md:pb-4"
        >
          <div
            className="pointer-events-none sticky top-2 z-30 flex h-0 justify-center md:hidden"
            style={{
              opacity: isRefreshing || pullDistance > 8 ? 1 : 0,
              transform: `translateY(${Math.min(8, pullDistance * 0.08)}px)`,
            }}
          >
            <div className="flex h-9 items-center gap-2 rounded-full border border-black/[0.06] dark:border-white/10 bg-white/95 dark:bg-neutral-950/95 px-3 text-[12px] font-semibold text-gray-500 dark:text-neutral-300 shadow-lg backdrop-blur">
              <RefreshCw
                className={`h-4 w-4 text-primary-500 ${isRefreshing ? 'animate-spin' : ''}`}
                style={!isRefreshing ? { transform: `rotate(${Math.min(180, (pullDistance / pullThreshold) * 180)}deg)` } : undefined}
              />
              {isRefreshing ? 'Actualisation' : pullDistance >= pullThreshold ? 'Relâcher' : 'Tirer'}
            </div>
          </div>

          <div
            className="w-full md:max-w-7xl md:mx-auto"
            style={{
              transform: `translateY(${isRefreshing ? 28 : Math.min(34, pullDistance * 0.38)}px)`,
              transition: isPulling ? 'none' : 'transform 200ms ease-out',
            }}
          >
            {children}
          </div>
        </main>
      </div>

      {isMobileMenuOpen && (
        <>
          {/* Backdrop sombre flouté satiné interactif */}
          <button
            className="fixed inset-0 z-[55] bg-black/35 backdrop-blur-[3px] md:hidden animate-backdrop-fade-in"
            onClick={() => setIsMobileMenuOpen(false)}
            aria-label="Fermer le menu"
          />
          
          {/* Bottom Sheet coulissante */}
          <div className="fixed inset-x-0 bottom-0 z-[65] md:hidden pb-[calc(env(safe-area-inset-bottom)+12px)] animate-bottom-sheet-slide-in">
            <div className="mx-3 rounded-[32px] border border-black/[0.08] dark:border-white/[0.08] bg-white/80 dark:bg-neutral-950/80 backdrop-blur-2xl shadow-[0_-20px_50px_rgba(0,0,0,0.15)] dark:shadow-[0_-20px_50px_rgba(0,0,0,0.4)] overflow-hidden">
              
              {/* Drag Handle (tirette visuelle mobile) */}
              <div className="flex justify-center py-2.5 cursor-pointer" onClick={() => setIsMobileMenuOpen(false)}>
                <div className="w-12 h-1.5 rounded-full bg-gray-300 dark:bg-neutral-800" />
              </div>

              <div className="flex items-center justify-between px-6 pb-3 border-b border-black/[0.04] dark:border-white/[0.04]">
                <span className="text-[11px] font-extrabold uppercase tracking-[0.18em] text-gray-400 dark:text-neutral-500">Menu Plus</span>
                <button
                  onClick={() => setIsMobileMenuOpen(false)}
                  className="p-1.5 rounded-full bg-gray-100 dark:bg-neutral-900 text-gray-500 dark:text-neutral-400 hover:scale-95 transition-transform"
                  aria-label="Fermer"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>

              {/* Présentation en Grille Moderne */}
              <div className="grid grid-cols-3 gap-3 p-4">
                {mobileMoreItems.map((item) => {
                  const Icon = item.icon;
                  const isActive = activePage === item.id;
                  return (
                    <button
                      key={item.id}
                      onClick={() => navigateToPage(item.id)}
                      className={`flex flex-col items-center justify-center aspect-square rounded-[22px] p-3 text-center transition-all tap-bounce cursor-pointer border ${
                        isActive
                          ? 'bg-primary-500 text-white border-primary-500 shadow-md shadow-primary-500/20'
                          : 'bg-white/40 dark:bg-neutral-900/30 text-gray-700 dark:text-neutral-300 border-black/[0.03] dark:border-white/[0.02] hover:bg-white/60 dark:hover:bg-neutral-900/50'
                      }`}
                      aria-current={isActive ? 'page' : undefined}
                    >
                      <div className="relative p-2.5 rounded-2xl bg-black/[0.03] dark:bg-white/[0.03] mb-2">
                        <Icon className="w-5 h-5 shrink-0" />
                        {item.id === 'settings' && updateAvailable && (
                          <span className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 bg-red-500 rounded-full border-2 border-white dark:border-neutral-950" />
                        )}
                      </div>
                      <span className="w-full text-[11px] font-extrabold tracking-wide truncate">{item.label}</span>
                    </button>
                  );
                })}
              </div>
              {showQuitAction && (
                <div className="px-4 pb-4">
                  <button
                    onClick={() => {
                      setIsMobileMenuOpen(false);
                      void handleQuitApp();
                    }}
                    className="flex w-full items-center justify-center gap-2 rounded-2xl border border-red-500/15 bg-red-500/10 px-4 py-3 text-sm font-extrabold text-red-600 dark:text-red-400 transition-all tap-bounce hover:bg-red-500/15"
                    aria-label="Quitter"
                  >
                    <Power className="w-4 h-4 shrink-0" />
                    Quitter
                  </button>
                </div>
              )}
            </div>
          </div>
        </>
      )}

      <nav className="mobile-bottom-nav md:hidden flex-shrink-0 h-[calc(68px+env(safe-area-inset-bottom))] border-t border-black/[0.06] dark:border-white/10 bg-white/90 dark:bg-neutral-950/90 backdrop-blur-2xl px-2 pt-2.5 pb-[env(safe-area-inset-bottom)] z-[60] shadow-[0_-12px_40px_rgba(0,0,0,0.06)] dark:shadow-[0_-12px_40px_rgba(0,0,0,0.3)]">
        <div className="grid h-full grid-cols-5 gap-1.5">
          {mobilePrimaryItems.map((item) => {
            const Icon = item.icon;
            const isActive = activePage === item.id;
            return (
              <button
                key={item.id}
                onClick={() => navigateToPage(item.id)}
                className={`relative min-w-0 rounded-2xl flex flex-col items-center justify-center gap-1 text-[10px] font-extrabold tracking-wide transition-all tap-bounce cursor-pointer ${
                  isActive
                    ? 'text-primary-600 dark:text-primary-400 bg-primary-500/10 dark:bg-primary-500/15'
                    : 'text-gray-500 dark:text-neutral-400 hover:bg-gray-50 dark:hover:bg-neutral-900/50'
                }`}
                aria-current={isActive ? 'page' : undefined}
              >
                <Icon className="w-5 h-5 shrink-0" />
                <span className="w-full px-1 truncate">{item.label}</span>
              </button>
            );
          })}
          <button
            onClick={() => setIsMobileMenuOpen(prev => !prev)}
            className={`relative min-w-0 rounded-2xl flex flex-col items-center justify-center gap-1 text-[10px] font-extrabold tracking-wide transition-all tap-bounce cursor-pointer ${
              isMoreActive || isMobileMenuOpen
                ? 'text-primary-600 dark:text-primary-400 bg-primary-500/10 dark:bg-primary-500/15'
                : 'text-gray-500 dark:text-neutral-400 hover:bg-gray-50 dark:hover:bg-neutral-900/50'
            }`}
            aria-expanded={isMobileMenuOpen}
            aria-current={isMoreActive ? 'page' : undefined}
          >
            <div className="relative">
              <MoreHorizontal className="w-5 h-5 shrink-0" />
              {updateAvailable && (
                <span className="absolute -top-1 -right-1 w-2 h-2 bg-red-500 rounded-full border border-white dark:border-black" />
              )}
            </div>
            <span className="w-full px-1 truncate">Plus</span>
          </button>
        </div>
      </nav>
    </div>
  );
};

export default Layout;
