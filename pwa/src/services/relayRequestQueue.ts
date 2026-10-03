/** Bound concurrent encrypted replies as well as outgoing work. Queued requests honor cancellation. */
export class RelayRequestQueue {
    private active = 0;
    private waiting: Array<{ start: () => void }> = [];
    constructor(private readonly capacity: number) {}

    private claim(): () => void {
        this.active++;
        let released = false;
        return () => {
            if (released) return;
            released = true;
            this.active--;
            this.waiting.shift()?.start();
        };
    }

    tryAcquire(signal?: AbortSignal | null): (() => void) | null {
        if (signal?.aborted) throw new DOMException('Requête annulée.', 'AbortError');
        return this.active < this.capacity ? this.claim() : null;
    }

    acquire(signal?: AbortSignal | null): Promise<() => void> {
        if (signal?.aborted) return Promise.reject(new DOMException('Requête annulée.', 'AbortError'));
        return new Promise((resolve, reject) => {
            const entry = { start: () => {
                signal?.removeEventListener('abort', abort);
                resolve(this.claim());
            } };
            const abort = () => {
                const index = this.waiting.indexOf(entry);
                if (index < 0) return;
                this.waiting.splice(index, 1);
                signal?.removeEventListener('abort', abort);
                reject(new DOMException('Requête annulée.', 'AbortError'));
            };
            if (this.active < this.capacity) entry.start();
            else { this.waiting.push(entry); signal?.addEventListener('abort', abort, { once: true }); }
        });
    }
}
