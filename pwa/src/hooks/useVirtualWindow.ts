import { useLayoutEffect, useRef, useState } from 'react';
import { virtualRange, type VirtualRange } from '../utils/virtualWindow';

/** Window inside the page's existing scroll area; scrolling never rerenders the journal parent. */
export function useVirtualWindow(offsets: readonly number[], keys: readonly string[], resetKey: string) {
    const containerRef = useRef<HTMLDivElement>(null);
    const anchorRef = useRef<{ key: string; inset: number } | null>(null);
    const previousResetKey = useRef(resetKey);
    const [range, setRange] = useState<VirtualRange>({ start: 0, end: 0 });

    useLayoutEffect(() => {
        const list = containerRef.current;
        if (!list) return;
        const scroller = list.closest('main');
        const target = scroller ?? window;
        let frame = 0;
        if (previousResetKey.current !== resetKey) {
            previousResetKey.current = resetKey;
            anchorRef.current = null;
            if (scroller) scroller.scrollTop = 0;
            else window.scrollTo(0, 0);
        }
        const relativeTop = () => (scroller?.getBoundingClientRect().top ?? 0) - list.getBoundingClientRect().top;
        const anchor = anchorRef.current;
        if (anchor && list.offsetParent !== null) {
            const index = keys.indexOf(anchor.key);
            if (index >= 0) {
                const delta = offsets[index] - anchor.inset - relativeTop();
                if (scroller) scroller.scrollTop += delta;
                else window.scrollBy(0, delta);
            }
        }
        const measure = () => {
            frame = 0;
            const top = relativeTop();
            const height = list.offsetParent === null ? 0 : (scroller?.clientHeight ?? window.innerHeight);
            const next = virtualRange(offsets, top, height);
            const first = virtualRange(offsets, top, height, 0).start;
            anchorRef.current = top > 0 && height > 0 && first < keys.length
                ? { key: keys[first], inset: offsets[first] - top } : null;
            setRange(previous => previous.start === next.start && previous.end === next.end ? previous : next);
        };
        const schedule = () => { if (!frame) frame = requestAnimationFrame(measure); };
        measure();
        target.addEventListener('scroll', schedule, { passive: true });
        window.addEventListener('resize', schedule);
        const observer = new ResizeObserver(schedule);
        observer.observe(list);
        if (scroller) observer.observe(scroller);
        const journal = list.parentElement?.parentElement;
        if (journal && journal !== scroller) observer.observe(journal);
        return () => {
            if (frame) cancelAnimationFrame(frame);
            observer.disconnect();
            target.removeEventListener('scroll', schedule);
            window.removeEventListener('resize', schedule);
        };
    }, [offsets, keys, resetKey]);
    return { containerRef, range };
}
