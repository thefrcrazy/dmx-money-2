import { useCallback, useSyncExternalStore } from 'react';

/** Mount only the matching layout and react to live viewport/orientation changes. */
export function useMediaQuery(query: string) {
    const subscribe = useCallback((notify: () => void) => {
        const media = window.matchMedia(query);
        media.addEventListener('change', notify);
        return () => media.removeEventListener('change', notify);
    }, [query]);
    const snapshot = useCallback(() => window.matchMedia(query).matches, [query]);
    return useSyncExternalStore(subscribe, snapshot, () => false);
}
