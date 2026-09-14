// Fix for Safari 13 MediaQueryList.addEventListener
// Safari 13 supports matchMedia but returns a MediaQueryList that inherits from EventTarget only in newer versions.
// React 18+ relies on addEventListener being present on the result of matchMedia.

if (typeof window !== 'undefined' && window.matchMedia) {
    try {
        const mediaQueryList = window.matchMedia('(min-width: 1px)');
        // Check if addEventListener is missing on the prototype or instance
        if (!mediaQueryList.addEventListener) {
            const proto = Object.getPrototypeOf(mediaQueryList);

            // Patch the prototype if possible, otherwise patch the instance (though prototype is better)
            if (proto) {
                // Map to store wrappers to enable removeEventListener
                const wrappers = new Map<EventListener, (mql: MediaQueryList) => void>();

                proto.addEventListener = function (event: string, listener: EventListener) {
                    if (event === 'change') {
                        // Create a wrapper to emulate MediaQueryListEvent
                        const wrapper = (mql: MediaQueryList) => {
                            // Create a fake event object that mimics MediaQueryListEvent
                            const fakeEvent = {
                                matches: mql.matches,
                                media: mql.media,
                                target: mql,
                                type: 'change',
                                timeStamp: Date.now(),
                                bubbles: false,
                                cancelable: false,
                                // Add other Event properties as needed
                            } as unknown as MediaQueryListEvent; // Force type check pass
                            
                            // Call the listener with the fake event
                            if (typeof listener === 'function') {
                                listener.call(this, fakeEvent as any);
                            } else if (listener && typeof (listener as any).handleEvent === 'function') {
                                (listener as any).handleEvent(fakeEvent);
                            }
                        };
                        
                        wrappers.set(listener, wrapper);
                        this.addListener(wrapper);
                    }
                };

                proto.removeEventListener = function (event: string, listener: EventListener) {
                    if (event === 'change') {
                        const wrapper = wrappers.get(listener);
                        if (wrapper) {
                            this.removeListener(wrapper);
                            wrappers.delete(listener);
                        } else {
                            // Fallback: try removing the listener directly if it wasn't wrapped (unlikely but safe)
                            this.removeListener(listener as any);
                        }
                    }
                };
            }
        }
    } catch (e) {
        console.error('Failed to apply MediaQueryList polyfill', e);
    }
}
