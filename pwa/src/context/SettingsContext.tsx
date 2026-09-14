import React, { createContext, useContext, useState, useEffect, useRef } from 'react';
import { PredictionFakeTransaction, PredictionTimeRange, ScheduledDueRange, Settings, SettingsContextType } from '../types';
import { dbService } from '../services/db';
import { generatePalette, formatRgb } from '../utils/colors';
import { LATEST_VERSION } from '../constants/changelog';
import { LOGO_PATH, publicAsset } from '../utils/assets';
import { hasTauriRuntime } from '../utils/runtime';
import { selectNewestVersion } from '../utils/version';
import {
    applySettingsMutation,
    createSettingsMutation,
    hasSettingsMutationChanges,
    SettingsMutation,
} from '../services/settingsSync';

const iconCache: Record<string, Uint8Array> = {};

const loadIcon = async (isDark: boolean): Promise<Uint8Array | null> => {
    const iconName = isDark ? 'icon-dark.png' : 'icon-light.png';
    if (iconCache[iconName]) return iconCache[iconName];
    try {
        const response = await fetch(publicAsset(`icons/${iconName}`));
        const blob = await response.blob();
        const arrayBuffer = await blob.arrayBuffer();
        const uint8Array = new Uint8Array(arrayBuffer);
        iconCache[iconName] = uint8Array;
        return uint8Array;
    } catch (error) {
        console.error('Failed to load icon:', error);
        return null;
    }
};

const DEFAULT_SETTINGS: Settings = {
    settingsRevision: 0,
    theme: 'system',
    primaryColor: 'default',
    windowPosition: null,
    windowSize: null,
    componentSpacing: 6,
    componentPadding: 6,
    lastSeenVersion: LATEST_VERSION,
    dismissedBudgetSuggestions: [],
    dismissedScheduledSuggestions: []
};

const DISMISSED_BUDGET_SUGGESTIONS_STORAGE_KEY = 'dmxmoney.dismissedBudgetSuggestions';
const DISMISSED_SCHEDULED_SUGGESTIONS_STORAGE_KEY = 'dmxmoney.dismissedScheduledSuggestions';
const PREDICTION_TIME_RANGE_STORAGE_KEY = 'dmxmoney.predictions.timeRange';
const PREDICTION_CUSTOM_END_DATE_STORAGE_KEY = 'dmxmoney.predictions.customEndDate';
const PREDICTION_ALERT_THRESHOLD_STORAGE_KEY = 'dmxmoney.predictions.alertThreshold';
const PREDICTION_MONTH_START_STORAGE_KEY = 'dmxmoney.predictions.monthStartsOnFirst';
const PREDICTION_FAKE_TRANSACTIONS_STORAGE_KEY = 'dmxmoney.predictions.fakeTransactions';
const ANALYTICS_TIME_RANGE_STORAGE_KEY = 'dmxmoney.analytics.timeRange';
const ANALYTICS_CUSTOM_START_DATE_STORAGE_KEY = 'dmxmoney.analytics.customStartDate';
const ANALYTICS_CUSTOM_END_DATE_STORAGE_KEY = 'dmxmoney.analytics.customEndDate';
const ANALYTICS_MONTH_START_STORAGE_KEY = 'dmxmoney.analytics.monthStartsOnFirst';
const ANALYTICS_HIDDEN_EXPENSE_CATEGORIES_STORAGE_KEY = 'dmxmoney.analytics.hiddenExpenseCategories';
const ANALYTICS_HIDDEN_INCOME_CATEGORIES_STORAGE_KEY = 'dmxmoney.analytics.hiddenIncomeCategories';
const SCHEDULED_DUE_RANGE_STORAGE_KEY = 'dmxmoney.scheduled.dueRange';

const TIME_RANGES: PredictionTimeRange[] = ['week', 'month', '2months', '3months', '6months', '9months', 'year', 'custom'];
const SCHEDULED_DUE_RANGES: ScheduledDueRange[] = ['all', 'month', '2months', '3months', '6months', 'year'];

const formatDateInput = (date: Date) => {
    const year = date.getFullYear();
    const month = `${date.getMonth() + 1}`.padStart(2, '0');
    const day = `${date.getDate()}`.padStart(2, '0');
    return `${year}-${month}-${day}`;
};

const addMonths = (date: Date, months: number) => {
    const next = new Date(date);
    next.setMonth(next.getMonth() + months);
    return next;
};

const getDefaultPredictionCustomEndDate = () => formatDateInput(addMonths(new Date(), 1));
const getDefaultAnalyticsCustomStartDate = () => formatDateInput(addMonths(new Date(), -1));
const getDefaultAnalyticsCustomEndDate = () => formatDateInput(new Date());

const isPredictionTimeRange = (value: unknown): value is PredictionTimeRange => (
    typeof value === 'string' && TIME_RANGES.includes(value as PredictionTimeRange)
);

const isScheduledDueRange = (value: unknown): value is ScheduledDueRange => (
    typeof value === 'string' && SCHEDULED_DUE_RANGES.includes(value as ScheduledDueRange)
);

const isPredictionFakeTransaction = (value: unknown): value is PredictionFakeTransaction => {
    if (!value || typeof value !== 'object') return false;
    const item = value as Partial<PredictionFakeTransaction>;
    return (
        typeof item.id === 'string'
        && typeof item.date === 'string'
        && typeof item.accountId === 'string'
        && (item.type === 'income' || item.type === 'expense' || item.type === 'transfer')
        && typeof item.amount === 'number'
        && Number.isFinite(item.amount)
        && item.amount > 0
        && typeof item.description === 'string'
        && typeof item.category === 'string'
        && (item.enabled === undefined || typeof item.enabled === 'boolean')
    );
};

const normalizeSettings = (settings: Settings | null | undefined): Settings => ({
    ...DEFAULT_SETTINGS,
    ...(settings || {}),
    dismissedBudgetSuggestions: settings?.dismissedBudgetSuggestions || [],
    dismissedScheduledSuggestions: settings?.dismissedScheduledSuggestions || [],
    predictionTimeRange: isPredictionTimeRange(settings?.predictionTimeRange) ? settings.predictionTimeRange : 'year',
    predictionCustomEndDate: settings?.predictionCustomEndDate || getDefaultPredictionCustomEndDate(),
    predictionAlertThreshold: Number.isFinite(settings?.predictionAlertThreshold) ? Number(settings?.predictionAlertThreshold) : 0,
    predictionMonthStartsOnFirst: settings?.predictionMonthStartsOnFirst ?? true,
    predictionFakeTransactions: (settings?.predictionFakeTransactions || [])
        .filter(isPredictionFakeTransaction)
        .map(transaction => ({ ...transaction, enabled: transaction.enabled !== false })),
    analyticsTimeRange: isPredictionTimeRange(settings?.analyticsTimeRange) ? settings.analyticsTimeRange : 'year',
    analyticsCustomStartDate: settings?.analyticsCustomStartDate || getDefaultAnalyticsCustomStartDate(),
    analyticsCustomEndDate: settings?.analyticsCustomEndDate || getDefaultAnalyticsCustomEndDate(),
    analyticsMonthStartsOnFirst: settings?.analyticsMonthStartsOnFirst ?? true,
    analyticsHiddenExpenseCategories: settings?.analyticsHiddenExpenseCategories || [],
    analyticsHiddenIncomeCategories: settings?.analyticsHiddenIncomeCategories || [],
    scheduledDueRange: isScheduledDueRange(settings?.scheduledDueRange) ? settings.scheduledDueRange : 'all'
});

const parseStoredSuggestionKeys = (value: string | null) => {
    if (!value) return [];
    try {
        const parsed = JSON.parse(value);
        return Array.isArray(parsed) ? parsed.filter((key): key is string => typeof key === 'string') : [];
    } catch {
        return [];
    }
};

const readAndClearStoredSuggestionKeys = (key: string) => {
    try {
        const keys = parseStoredSuggestionKeys(localStorage.getItem(key));
        localStorage.removeItem(key);
        return keys;
    } catch {
        return [];
    }
};

const mergeSuggestionKeys = (...sources: Array<string[] | undefined>) => (
    Array.from(new Set(sources.flatMap(source => source || [])))
);

const readAndClearStoredValue = (key: string) => {
    try {
        const value = localStorage.getItem(key);
        localStorage.removeItem(key);
        return value;
    } catch {
        return null;
    }
};

const readAndClearStoredStringArray = (key: string) => {
    const value = readAndClearStoredValue(key);
    if (!value) return [];
    try {
        const parsed = JSON.parse(value);
        return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : [];
    } catch {
        return [];
    }
};

const readAndClearStoredFakeTransactions = () => {
    const value = readAndClearStoredValue(PREDICTION_FAKE_TRANSACTIONS_STORAGE_KEY);
    if (!value) return [];
    try {
        const parsed = JSON.parse(value);
        if (!Array.isArray(parsed)) return [];
        return parsed
            .filter(isPredictionFakeTransaction)
            .map(transaction => ({ ...transaction, enabled: transaction.enabled !== false }));
    } catch {
        return [];
    }
};

const mergeFakeTransactions = (...sources: Array<PredictionFakeTransaction[] | undefined>) => {
    const byId = new Map<string, PredictionFakeTransaction>();
    sources.flatMap(source => source || []).forEach(transaction => {
        byId.set(transaction.id, transaction);
    });
    return Array.from(byId.values());
};

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export const SettingsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
    const [isInitialLoadDone, setIsInitialLoadDone] = useState(false);
    const [isTransitioning, setIsTransitioning] = useState(false);
    const isLoadedRef = useRef(false);
    const isRestoringRef = useRef(false);
    const settingsRef = useRef<Settings>(DEFAULT_SETTINGS);
    const saveChainRef = useRef<Promise<void>>(Promise.resolve());
    const pendingMutationsRef = useRef(new Map<number, SettingsMutation>());
    const nextMutationIdRef = useRef(0);

    const [isSystemDark] = useState(() => {
        try {
            return window.matchMedia('(prefers-color-scheme: dark)').matches;
        } catch (e) {
            return false;
        }
    });

    const persistSettingsMutation = (
        mutation: SettingsMutation,
        localSettings: Settings,
    ) => {
        if (!hasSettingsMutationChanges(mutation)) return Promise.resolve();

        const mutationId = ++nextMutationIdRef.current;
        pendingMutationsRef.current.set(mutationId, mutation);
        const operation = saveChainRef.current
            .catch(() => undefined)
            .then(() => dbService.patchSettings(mutation, localSettings));
        saveChainRef.current = operation.catch(() => undefined);

        return operation.finally(() => {
            pendingMutationsRef.current.delete(mutationId);
        });
    };

    const applySettingsPatch = (
        patch: Partial<Settings>,
        applyVisual = false,
    ) => {
        const previous = settingsRef.current;
        const next = normalizeSettings({ ...previous, ...patch });
        const mutation = createSettingsMutation(
            previous,
            next,
            Object.keys(patch) as Array<keyof Settings>,
        );

        settingsRef.current = next;
        setSettings(next);
        if (applyVisual) applyVisualSettings(next);
        return persistSettingsMutation(mutation, next);
    };

    const migrateLocalSettings = (savedSettings: Settings | null) => {
        const initial = normalizeSettings(savedSettings);
        const dismissedBudgetSuggestions = mergeSuggestionKeys(
            initial.dismissedBudgetSuggestions,
            readAndClearStoredSuggestionKeys(DISMISSED_BUDGET_SUGGESTIONS_STORAGE_KEY)
        );
        const dismissedScheduledSuggestions = mergeSuggestionKeys(
            initial.dismissedScheduledSuggestions,
            readAndClearStoredSuggestionKeys(DISMISSED_SCHEDULED_SUGGESTIONS_STORAGE_KEY)
        );

        const localPredictionTimeRange = readAndClearStoredValue(PREDICTION_TIME_RANGE_STORAGE_KEY);
        const localPredictionCustomEndDate = readAndClearStoredValue(PREDICTION_CUSTOM_END_DATE_STORAGE_KEY);
        const localPredictionAlertThreshold = Number(readAndClearStoredValue(PREDICTION_ALERT_THRESHOLD_STORAGE_KEY));
        const localPredictionMonthStartsOnFirst = readAndClearStoredValue(PREDICTION_MONTH_START_STORAGE_KEY);
        const localPredictionFakeTransactions = readAndClearStoredFakeTransactions();
        const localAnalyticsTimeRange = readAndClearStoredValue(ANALYTICS_TIME_RANGE_STORAGE_KEY);
        const localAnalyticsCustomStartDate = readAndClearStoredValue(ANALYTICS_CUSTOM_START_DATE_STORAGE_KEY);
        const localAnalyticsCustomEndDate = readAndClearStoredValue(ANALYTICS_CUSTOM_END_DATE_STORAGE_KEY);
        const localAnalyticsMonthStartsOnFirst = readAndClearStoredValue(ANALYTICS_MONTH_START_STORAGE_KEY);
        const localAnalyticsHiddenExpenseCategories = readAndClearStoredStringArray(ANALYTICS_HIDDEN_EXPENSE_CATEGORIES_STORAGE_KEY);
        const localAnalyticsHiddenIncomeCategories = readAndClearStoredStringArray(ANALYTICS_HIDDEN_INCOME_CATEGORIES_STORAGE_KEY);
        const localScheduledDueRange = readAndClearStoredValue(SCHEDULED_DUE_RANGE_STORAGE_KEY);

        const migrated = {
            ...initial,
            dismissedBudgetSuggestions,
            dismissedScheduledSuggestions,
            predictionTimeRange: isPredictionTimeRange(localPredictionTimeRange) && initial.predictionTimeRange === 'year'
                ? localPredictionTimeRange
                : initial.predictionTimeRange,
            predictionCustomEndDate: localPredictionCustomEndDate && initial.predictionCustomEndDate === getDefaultPredictionCustomEndDate()
                ? localPredictionCustomEndDate
                : initial.predictionCustomEndDate,
            predictionAlertThreshold: Number.isFinite(localPredictionAlertThreshold) && initial.predictionAlertThreshold === 0
                ? localPredictionAlertThreshold
                : initial.predictionAlertThreshold,
            predictionMonthStartsOnFirst: localPredictionMonthStartsOnFirst !== null && initial.predictionMonthStartsOnFirst === true
                ? localPredictionMonthStartsOnFirst === 'true'
                : initial.predictionMonthStartsOnFirst,
            predictionFakeTransactions: mergeFakeTransactions(initial.predictionFakeTransactions, localPredictionFakeTransactions),
            analyticsTimeRange: isPredictionTimeRange(localAnalyticsTimeRange) && initial.analyticsTimeRange === 'year'
                ? localAnalyticsTimeRange
                : initial.analyticsTimeRange,
            analyticsCustomStartDate: localAnalyticsCustomStartDate && initial.analyticsCustomStartDate === getDefaultAnalyticsCustomStartDate()
                ? localAnalyticsCustomStartDate
                : initial.analyticsCustomStartDate,
            analyticsCustomEndDate: localAnalyticsCustomEndDate && initial.analyticsCustomEndDate === getDefaultAnalyticsCustomEndDate()
                ? localAnalyticsCustomEndDate
                : initial.analyticsCustomEndDate,
            analyticsMonthStartsOnFirst: localAnalyticsMonthStartsOnFirst !== null && initial.analyticsMonthStartsOnFirst === true
                ? localAnalyticsMonthStartsOnFirst === 'true'
                : initial.analyticsMonthStartsOnFirst,
            analyticsHiddenExpenseCategories: mergeSuggestionKeys(initial.analyticsHiddenExpenseCategories, localAnalyticsHiddenExpenseCategories),
            analyticsHiddenIncomeCategories: mergeSuggestionKeys(initial.analyticsHiddenIncomeCategories, localAnalyticsHiddenIncomeCategories),
            scheduledDueRange: isScheduledDueRange(localScheduledDueRange) && initial.scheduledDueRange === 'all'
                ? localScheduledDueRange
                : initial.scheduledDueRange
        };
        if (JSON.stringify(migrated) !== JSON.stringify(initial)) {
            const changedKeys = (Object.keys(migrated) as Array<keyof Settings>)
                .filter(key => JSON.stringify(initial[key]) !== JSON.stringify(migrated[key]));
            const mutation = createSettingsMutation(initial, migrated, changedKeys);
            persistSettingsMutation(mutation, migrated).catch(() => { });
        }
        return migrated;
    };

    useEffect(() => {
        if (isSystemDark && !isLoadedRef.current) {
            document.documentElement.classList.add('dark');
        }
    }, [isSystemDark]);

    useEffect(() => {
        settingsRef.current = settings;
    }, [settings]);

    const updateWindowIcon = (isDark: boolean) => {
        if (!hasTauriRuntime()) return;
        loadIcon(isDark).then(icon => {
            if (!icon) return;
            import('@tauri-apps/api/window')
                .then(({ getCurrentWindow }) => getCurrentWindow().setIcon(icon))
                .catch(() => { });
        });
    };

    const setCssVariables = (color: string) => {
        const palette = generatePalette(color);
        if (palette) {
            document.documentElement.style.setProperty('--color-primary', color);
            document.documentElement.style.setProperty('--color-primary-custom', color);

            // For legacy Safari (Catalina), we NEED commas for rgba() to work in Tailwind 3
            // But Tailwind 4 (Modern) also accepts them.
            const rgbStr = formatRgb(palette[500]);
            const rgbWithCommas = rgbStr.replace(/ /g, ', ');

            document.documentElement.style.setProperty('--color-primary-rgb', rgbWithCommas);
            document.documentElement.style.setProperty('--color-primary-rgb-custom', rgbWithCommas);

            Object.entries(palette).forEach(([shade, rgb]) => {
                const shadeRgb = formatRgb(rgb).replace(/ /g, ', ');
                document.documentElement.style.setProperty(`--color-primary-${shade}-rgb`, shadeRgb);
            });
        }
    };

    const removeCssVariables = () => {
        const props = ['--color-primary', '--color-primary-custom', '--color-primary-rgb', '--color-primary-rgb-custom'];
        props.forEach(p => document.documentElement.style.removeProperty(p));
        [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950].forEach(shade => {
            document.documentElement.style.removeProperty(`--color-primary-${shade}-rgb`);
        });
    };

    const applyVisualSettings = (s: Settings) => {
        let isDark = false;
        if (s.theme === 'dark') {
            document.documentElement.classList.add('dark');
            isDark = true;
        } else if (s.theme === 'light') {
            document.documentElement.classList.remove('dark');
            isDark = false;
        } else {
            isDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            document.documentElement.classList.toggle('dark', isDark);
        }

        if (s.primaryColor && s.primaryColor !== 'default') {
            setCssVariables(s.primaryColor);
        } else {
            removeCssVariables();
        }
        updateWindowIcon(isDark);
    };

    const restoreWindow = async (currentSettings: Settings) => {
        if (!hasTauriRuntime()) return;
        if (isRestoringRef.current) return;
        isRestoringRef.current = true;
        try {
            const { getCurrentWindow, PhysicalPosition, PhysicalSize, LogicalSize } = await import('@tauri-apps/api/window');
            const appWindow = getCurrentWindow();
            await appWindow.setResizable(true);
            await appWindow.setDecorations(true);
            await appWindow.setShadow(true);

            if (currentSettings.windowSize && currentSettings.windowSize.width > 500) {
                await appWindow.setSize(new PhysicalSize(currentSettings.windowSize.width, currentSettings.windowSize.height));
            } else {
                await appWindow.setSize(new LogicalSize(1320, 790));
            }

            if (currentSettings.windowPosition) {
                await appWindow.setPosition(new PhysicalPosition(currentSettings.windowPosition.x, currentSettings.windowPosition.y));
            } else {
                await appWindow.center();
            }
        } catch (error) {
            console.error('Failed to restore window:', error);
        } finally {
            setTimeout(() => {
                isRestoringRef.current = false;
            }, 1000);
        }
    };

    useEffect(() => {
        let completed = false;
        let transitionTimer: ReturnType<typeof setTimeout> | undefined;
        let doneTimer: ReturnType<typeof setTimeout> | undefined;

        const finishInitialLoad = (initial: Settings, delay = 1500) => {
            if (completed) return;
            completed = true;

            applyVisualSettings(initial);
            settingsRef.current = initial;
            setSettings(initial);
            isLoadedRef.current = true;

            transitionTimer = setTimeout(() => {
                setIsTransitioning(true);
                doneTimer = setTimeout(() => {
                    setIsInitialLoadDone(true);
                }, 500);
            }, delay);
        };

        const fallbackTimer = setTimeout(() => {
            console.warn('Settings load timed out; continuing with default settings.');
            finishInitialLoad(DEFAULT_SETTINGS, 0);
        }, 6000);

        dbService.getSettings()
            .then(savedSettings => {
                clearTimeout(fallbackTimer);
                const initial = migrateLocalSettings(savedSettings);
                finishInitialLoad(initial);
            })
            .catch(() => {
                clearTimeout(fallbackTimer);
                finishInitialLoad(DEFAULT_SETTINGS, 0);
            });

        return () => {
            completed = true;
            clearTimeout(fallbackTimer);
            if (transitionTimer) clearTimeout(transitionTimer);
            if (doneTimer) clearTimeout(doneTimer);
        };
    }, []);

    useEffect(() => {
        const refreshSyncedSettings = () => {
            dbService.getSettings()
                .then(savedSettings => {
                    if (!savedSettings) return;
                    const next = normalizeSettings(savedSettings);
                    setSettings(current => {
                        const serverMerged = {
                            ...next,
                            lastSeenVersion: selectNewestVersion(
                                current.lastSeenVersion,
                                next.lastSeenVersion
                            )
                        };
                        const merged = Array.from(pendingMutationsRef.current.values())
                            .reduce(applySettingsMutation, serverMerged);
                        settingsRef.current = merged;
                        applyVisualSettings(merged);
                        return merged;
                    });
                })
                .catch(() => { });
        };

        window.addEventListener('dmxmoney-settings-refresh', refreshSyncedSettings);
        return () => window.removeEventListener('dmxmoney-settings-refresh', refreshSyncedSettings);
    }, []);

    useEffect(() => {
        if (!hasTauriRuntime()) return;
        if (isInitialLoadDone && isLoadedRef.current) {
            setTimeout(() => {
                restoreWindow(settingsRef.current);
            }, 500);
        }
    }, [isInitialLoadDone]);

    useEffect(() => {
        if (!hasTauriRuntime()) return;
        let unlistenMove: (() => void) | undefined;
        let unlistenResize: (() => void) | undefined;
        const setupListeners = async () => {
            const { getCurrentWindow } = await import('@tauri-apps/api/window');
            const appWindow = getCurrentWindow();
            unlistenMove = await appWindow.listen('tauri://move', async () => {
                if (isRestoringRef.current) return;
                const pos = await appWindow.innerPosition();
                debouncedSaveWindowPosition(pos.x, pos.y);
            });
            unlistenResize = await appWindow.listen('tauri://resize', async () => {
                if (isRestoringRef.current) return;
                const size = await appWindow.innerSize();
                if (size.width > 500 && size.height > 500) {
                    debouncedSaveWindowSize(size.width, size.height);
                }
            });
        };
        setupListeners();
        return () => {
            if (unlistenMove) unlistenMove();
            if (unlistenResize) unlistenResize();
        };
    }, []);

    useEffect(() => {
        const mq = window.matchMedia('(prefers-color-scheme: dark)');
        const handler = (e: MediaQueryListEvent) => {
            if (settingsRef.current.theme === 'system') {
                document.documentElement.classList.toggle('dark', e.matches);
                updateWindowIcon(e.matches);
            }
        };
        try {
            mq.addEventListener('change', handler);
            return () => mq.removeEventListener('change', handler);
        } catch (e) {
            mq.addListener(handler);
            return () => mq.removeListener(handler);
        }
    }, []);

    const savePositionTimeoutRef = useRef<any>(null);
    const saveSizeTimeoutRef = useRef<any>(null);

    const debouncedSaveWindowPosition = (x: number, y: number) => {
        if (savePositionTimeoutRef.current) clearTimeout(savePositionTimeoutRef.current);
        savePositionTimeoutRef.current = setTimeout(() => {
            applySettingsPatch({ windowPosition: { x, y } }).catch(() => { });
        }, 1000);
    };

    const debouncedSaveWindowSize = (width: number, height: number) => {
        if (saveSizeTimeoutRef.current) clearTimeout(saveSizeTimeoutRef.current);
        saveSizeTimeoutRef.current = setTimeout(() => {
            applySettingsPatch({ windowSize: { width, height } }).catch(() => { });
        }, 1000);
    };

    return (
        <SettingsContext.Provider value={{
            settings,
            updateTheme: theme => applySettingsPatch({ theme }, true),
            updatePrimaryColor: color => applySettingsPatch({ primaryColor: color }, true),
            updateWindowPosition: (x, y) => applySettingsPatch({ windowPosition: { x, y } }),
            updateWindowSize: (width, height) => applySettingsPatch({ windowSize: { width, height } }),
            updateAccountGroup: (id, group) => {
                const groups = { ...(settingsRef.current.accountGroups || {}) };
                if (group) groups[id] = group; else delete groups[id];
                return applySettingsPatch({ accountGroups: groups });
            },
            updateCustomGroups: groups => applySettingsPatch({ customGroups: groups }),
            renameCustomGroup: (oldName, newName) => {
                const current = settingsRef.current;
                const customGroups = (current.customGroups || []).map(group => group === oldName ? newName : group);
                const accountGroups = { ...(current.accountGroups || {}) };
                Object.keys(accountGroups).forEach(id => {
                    if (accountGroups[id] === oldName) accountGroups[id] = newName;
                });
                return applySettingsPatch({ customGroups, accountGroups });
            },
            updateCustomGroupsOrder: order => applySettingsPatch({ customGroupsOrder: order }),
            updateAccountsOrder: order => applySettingsPatch({ accountsOrder: order }),
            updateComponentSpacing: spacing => applySettingsPatch({ componentSpacing: spacing }),
            updateComponentPadding: padding => applySettingsPatch({ componentPadding: padding }),
            updateLastSeenVersion: version => applySettingsPatch({
                lastSeenVersion: selectNewestVersion(settingsRef.current.lastSeenVersion, version)
            }),
            updateDismissedBudgetSuggestions: keys => applySettingsPatch({
                dismissedBudgetSuggestions: keys
            }),
            updateDismissedScheduledSuggestions: keys => applySettingsPatch({
                dismissedScheduledSuggestions: keys
            }),
            updateSettings: patch => applySettingsPatch(patch, (
                Object.prototype.hasOwnProperty.call(patch, 'theme')
                || Object.prototype.hasOwnProperty.call(patch, 'primaryColor')
            ))
        }}>
            <div className={`transition-opacity duration-700 ${!isInitialLoadDone ? 'opacity-0' : 'opacity-100'}`}>
                {children}
            </div>
            {!isInitialLoadDone && (
                <div className={`fixed inset-0 w-full h-full flex flex-col items-center justify-center z-[9999] transition-all duration-500 ${isTransitioning ? 'opacity-0 scale-110 blur-sm' : 'opacity-100'} ${isSystemDark ? 'bg-black' : 'bg-white'} dark:bg-black`}>
                    <div className="flex flex-col items-center justify-center space-y-12">
                        <img src={LOGO_PATH} alt="Logo" className={`w-32 h-32 transition-transform duration-700 ${isTransitioning ? 'rotate-12 scale-110' : ''}`} />
                        <div className="w-10 h-10 border-4 border-indigo-500/10 border-t-indigo-500 rounded-full animate-spin" />
                    </div>
                </div>
            )}
        </SettingsContext.Provider>
    );
};

export const useSettings = () => {
    const context = useContext(SettingsContext);
    if (!context) throw new Error('useSettings must be used within SettingsProvider');
    return context;
};
