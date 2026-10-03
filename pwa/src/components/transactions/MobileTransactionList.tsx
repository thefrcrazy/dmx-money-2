import { memo, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { useVirtualWindow } from '../../hooks/useVirtualWindow';
import { formatDate } from '../../utils/format';

type Operation = { id: string; date: string };
type Entry<T> = { key: string; date: string; transaction?: T; first?: boolean; last?: boolean; position?: number };
interface Props<T extends Operation> {
    data: T[];
    resetKey: string;
    renderRow: (transaction: T, startsDay: boolean, endsDay: boolean) => ReactNode;
}

const EntryContent = memo(function EntryContent<T extends Operation>({ entry, renderRow }: {
    entry: Entry<T>; renderRow: Props<T>['renderRow'];
}) {
    return entry.transaction
        ? renderRow(entry.transaction, !!entry.first, !!entry.last)
        : <h3 className="flex h-full items-end px-4 pb-2 text-[13px] font-medium text-[var(--color-text-secondary)] first-letter:uppercase">
            {formatDate(entry.date, 'EEEE d MMM')}
        </h3>;
}) as <T extends Operation>(props: {entry: Entry<T>; renderRow: Props<T>['renderRow']}) => ReactNode;

/** Date headers and operations share one bounded window, even for a huge single day. */
export default function MobileTransactionList<T extends Operation>({ data, resetKey, renderRow }: Props<T>) {
    const { entries, offsets, keys } = useMemo(() => {
        const entries: Entry<T>[] = [];
        const offsets = [0];
        data.forEach((transaction, index) => {
            const first = index === 0 || data[index - 1].date !== transaction.date;
            const last = index === data.length - 1 || data[index + 1].date !== transaction.date;
            if (first) {
                entries.push({key: `date-${transaction.date}`, date: transaction.date});
                offsets.push(offsets[offsets.length - 1] + 44);
            }
            entries.push({key: `tx-${transaction.id}`, date: transaction.date, transaction, first, last, position: index + 1});
            offsets.push(offsets[offsets.length - 1] + 72);
        });
        return {entries, offsets, keys: entries.map(entry => entry.key)};
    }, [data]);
    const { containerRef, range } = useVirtualWindow(offsets, keys, resetKey);
    const [focusedKey, setFocusedKey] = useState<string | null>(null);
    const pendingFocus = useRef<{key: string; backwards: boolean} | null>(null);
    const visible = entries.slice(range.start, range.end).map((entry, offset) => ({entry, index: range.start + offset}));
    // Keep a keyboard-focused row mounted when the user scrolls away from it.
    const focusedIndex = useMemo(() => focusedKey ? entries.findIndex(entry => entry.key === focusedKey) : -1, [entries, focusedKey]);
    if (focusedIndex >= 0 && (focusedIndex < range.start || focusedIndex >= range.end)) {
        visible.push({entry: entries[focusedIndex], index: focusedIndex});
        visible.sort((a, b) => a.index - b.index);
    }

    useLayoutEffect(() => {
        const pending = pendingFocus.current;
        if (!pending) return;
        const row = containerRef.current?.querySelector(`[data-virtual-entry="${CSS.escape(pending.key)}"]`);
        const controls = row?.querySelectorAll<HTMLElement>('button, input, [tabindex="0"]');
        if (controls?.length) {
            pendingFocus.current = null;
            controls[pending.backwards ? controls.length - 1 : 0].focus();
        }
    }, [focusedKey, containerRef]);

    const moveFocus = (event: KeyboardEvent<HTMLDivElement>, index: number) => {
        if (event.key !== 'Tab') return;
        const controls = event.currentTarget.querySelectorAll('button, input, [tabindex="0"]');
        const edge = controls[event.shiftKey ? 0 : controls.length - 1];
        if (event.target !== edge) return;
        const step = event.shiftKey ? -1 : 1;
        let next = index + step;
        while (next >= 0 && next < entries.length && !entries[next].transaction) next += step;
        if (next < 0 || next >= entries.length) return;
        if (next >= range.start && next < range.end && index >= range.start && index < range.end) return;
        event.preventDefault();
        pendingFocus.current = {key: entries[next].key, backwards: event.shiftKey};
        setFocusedKey(entries[next].key);
    };

    return <div ref={containerRef} role="list" aria-label="Opérations" className="relative" data-virtual-journal="mobile" style={{height: offsets[offsets.length - 1]}}>
        {visible.map(({entry, index}) => {
            return <div key={entry.key} data-virtual-entry={entry.key}
                role={entry.transaction ? 'listitem' : undefined}
                aria-posinset={entry.position} aria-setsize={entry.transaction ? data.length : undefined}
                onKeyDownCapture={event => moveFocus(event, index)}
                onFocusCapture={() => setFocusedKey(entry.key)}
                onBlurCapture={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocusedKey(null); }}
                style={{position:'absolute', top:offsets[index], left:0, right:0, height:offsets[index + 1] - offsets[index]}}>
                <EntryContent entry={entry} renderRow={renderRow} />
            </div>;
        })}
    </div>;
}
