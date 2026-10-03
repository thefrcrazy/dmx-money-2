import { expect, test } from 'bun:test';
import { RelayRequestQueue } from './relayRequestQueue';

test('six startup reads share two slots and canceled queued work never starts', async () => {
    const queue = new RelayRequestQueue(2);
    const first = await queue.acquire();
    const second = await queue.acquire();
    const controller = new AbortController();
    const canceled = queue.acquire(controller.signal);
    let started = 0;
    const third = queue.acquire().then(release => { started++; return release; });
    const fourth = queue.acquire().then(release => { started++; return release; });
    await Promise.resolve();
    expect(started).toBe(0);
    controller.abort();
    await expect(canceled).rejects.toThrow('annulée');
    first(); first();
    const releaseThird = await third;
    expect(started).toBe(1);
    second();
    const releaseFourth = await fourth;
    expect(started).toBe(2);
    releaseThird(); releaseFourth();
    const last = await queue.acquire(); last();
});
