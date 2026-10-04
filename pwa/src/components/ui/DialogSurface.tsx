import React, { useLayoutEffect, useRef } from 'react';
import { createPortal } from 'react-dom';

let locks = 0;
let previousBodyOverflow = '';
let previousDocumentOverflow = '';

function lockScroll() {
    if (locks++ === 0) {
        previousBodyOverflow = document.body.style.overflow;
        previousDocumentOverflow = document.documentElement.style.overflow;
        document.body.style.overflow = 'hidden';
        document.documentElement.style.overflow = 'hidden';
    }
    return () => {
        if (--locks === 0) {
            document.body.style.overflow = previousBodyOverflow;
            document.documentElement.style.overflow = previousDocumentOverflow;
        }
    };
}

/** The browser provides focus containment, background inertness and focus restoration. */
export default function DialogSurface({ children, className, labelledBy, label, onClose, busy = false }: {
    children: React.ReactNode;
    className: string;
    labelledBy?: string;
    label?: string;
    onClose: () => void;
    busy?: boolean;
}) {
    const ref = useRef<HTMLDialogElement>(null);
    useLayoutEffect(() => {
        const dialog = ref.current;
        if (!dialog) return;
        dialog.showModal();
        const unlock = lockScroll();
        return () => {
            dialog.close();
            unlock();
        };
    }, []);
    useLayoutEffect(() => {
        ref.current?.setAttribute('closedby', busy ? 'none' : 'closerequest');
    }, [busy]);
    return createPortal(
        <dialog ref={ref} aria-modal="true" aria-labelledby={labelledBy} aria-label={labelledBy ? undefined : label}
            className={`app-dialog-surface ${className}`} tabIndex={-1}
            onKeyDown={event => {
                const dialog = ref.current;
                if (busy && event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); return; }
                if (!dialog || event.key !== 'Tab' || event.altKey || event.ctrlKey || event.metaKey
                    || (event.target as Element).closest('dialog') !== dialog) return;
                const controls = [...dialog.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [tabindex]')]
                    .filter(node => node.tabIndex >= 0 && !node.matches(':disabled') && !node.closest('[inert]') && node.getClientRects().length > 0
                        && getComputedStyle(node).visibility !== 'hidden');
                const first = controls[0];
                const last = controls[controls.length - 1];
                if (!first) { event.preventDefault(); dialog.focus(); }
                else if (event.shiftKey && (document.activeElement === first || document.activeElement === dialog)) {
                    event.preventDefault(); last.focus();
                } else if (!event.shiftKey && document.activeElement === last) {
                    event.preventDefault(); first.focus();
                }
            }} onClose={() => {
                const dialog = ref.current;
                if (!dialog || dialog.open) return;
                if (busy) dialog.showModal();
                else onClose();
            }} onCancel={event => {
                event.preventDefault();
                if (!busy) onClose();
            }}>
            {children}
        </dialog>, document.body,
    );
}

/** Popovers must remain inside their modal's inert boundary. */
export function popoverContainer(anchor: Element | null): Element {
    return anchor?.closest('dialog') ?? document.body;
}
