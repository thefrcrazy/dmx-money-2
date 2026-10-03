export interface VirtualRange { start: number; end: number }

/** First row whose trailing edge is beyond a position, using sorted row offsets. */
function rowAt(offsets: readonly number[], position: number): number {
    let low = 0;
    let high = offsets.length - 1;
    while (low < high) {
        const middle = (low + high) >>> 1;
        if (offsets[middle + 1] <= position) low = middle + 1;
        else high = middle;
    }
    return low;
}

/** Half-open window; work stays logarithmic even for large journals. */
export function virtualRange(offsets: readonly number[], top: number, height: number, overscan = 400): VirtualRange {
    const count = Math.max(0, offsets.length - 1);
    if (!count || height <= 0) return { start: 0, end: 0 };
    const start = Math.min(count, rowAt(offsets, Math.max(0, top - overscan)));
    const end = Math.min(count, rowAt(offsets, Math.max(0, top + height + overscan)) + 1);
    return { start, end: Math.max(start, end) };
}
