import { useState, useCallback, useEffect } from 'react';

type UpdaterState = {
    isChecking: boolean;
    updateAvailable: boolean;
};

const sharedState: UpdaterState = {
    isChecking: false,
    updateAvailable: false,
};

const listeners = new Set<(state: UpdaterState) => void>();
let pollingInterval: ReturnType<typeof setInterval> | null = null;

const hasTauriRuntime = () => (
    typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
);

const setSharedState = (patch: Partial<UpdaterState>) => {
    Object.assign(sharedState, patch);
    const nextState = { ...sharedState };
    listeners.forEach(listener => listener(nextState));
};

export const useUpdater = () => {
    const [state, setState] = useState<UpdaterState>(sharedState);

    const checkUpdate = useCallback(async (silent = false) => {
        // Prevent running in browser mode without Tauri
        if (!hasTauriRuntime() || sharedState.isChecking) return;

        setSharedState({ isChecking: true });
        try {
            const [{ check }, { ask, message }, { relaunch }] = await Promise.all([
                import('@tauri-apps/plugin-updater'),
                import('@tauri-apps/plugin-dialog'),
                import('@tauri-apps/plugin-process')
            ]);
            const updateResult = await check();
            
            if (updateResult) {
                console.log(`Update available: ${updateResult.version}`);
                setSharedState({ updateAvailable: true });
                
                // If not silent (manual check), ask to update immediately
                if (!silent) {
                    const yes = await ask(
                        `La version ${updateResult.version} est disponible.\n\nNotes : ${updateResult.body || 'Aucune note fournie.'}`,
                        {
                            title: 'Mise à jour disponible',
                            kind: 'info',
                            okLabel: 'Installer',
                            cancelLabel: 'Annuler',
                        }
                    );

                    if (yes) {
                        await updateResult.downloadAndInstall(undefined, { timeout: 600000 });
                        await relaunch();
                    }
                }
            } else {
                setSharedState({ updateAvailable: false });
                if (!silent) {
                    await message('Vous utilisez déjà la dernière version.', { title: 'Aucune mise à jour', kind: 'info' });
                }
            }
        } catch (error) {
            const errorMsg = String(error);
            console.error('Update check failed:', error);
            
            // Only show error if not silent
            if (!silent) {
                const { message } = await import('@tauri-apps/plugin-dialog');
                if (errorMsg.includes('fetch a valid release JSON')) {
                    await message(
                        'Une mise à jour est probablement en cours de préparation sur le serveur.\n\nVeuillez réessayer dans quelques minutes.',
                        { title: 'Mise à jour en cours', kind: 'info' }
                    );
                } else {
                    await message(`Impossible de vérifier ou installer la mise à jour.\n\n${errorMsg}`, { title: 'Erreur de mise à jour', kind: 'error' });
                }
            }
        } finally {
            setSharedState({ isChecking: false });
        }
    }, []);

    // Poll for updates every 15 minutes
    useEffect(() => {
        listeners.add(setState);
        setState({ ...sharedState });

        if (!pollingInterval && hasTauriRuntime()) {
            // Check at startup (silent)
            checkUpdate(true);

            pollingInterval = setInterval(() => {
                checkUpdate(true);
            }, 15 * 60 * 1000); // 15 minutes
        }

        return () => {
            listeners.delete(setState);
            if (listeners.size === 0 && pollingInterval) {
                clearInterval(pollingInterval);
                pollingInterval = null;
            }
        };
    }, [checkUpdate]);

    return { checkUpdate, isChecking: state.isChecking, updateAvailable: state.updateAvailable };
};
