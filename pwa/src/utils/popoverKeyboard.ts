interface EscapeEvent {
    key: string;
    nativeEvent?: { isComposing?: boolean };
    preventDefault(): void;
    stopPropagation(): void;
}

/** Consume Escape before the containing native dialog receives a close request. */
export function dismissPopoverOnEscape(event: EscapeEvent, open: boolean, close: () => void, trigger: HTMLElement | null) {
    if (!open || event.key !== 'Escape' || event.nativeEvent?.isComposing) return;
    event.preventDefault();
    event.stopPropagation();
    close();
    trigger?.focus();
}
