import { describe, expect, test } from 'bun:test';
import { virtualRange } from './virtualWindow';

describe('journal render window', () => {
    const offsets = Array.from({ length: 30001 }, (_, index) => index * 72);
    test('bounded at the beginning, middle and end of a 30,000-operation list', () => {
        for (const top of [0, 72000, 2160000 - 844]) {
            const range = virtualRange(offsets, top, 844);
            expect(range.start).toBeGreaterThanOrEqual(0);
            expect(range.end).toBeLessThanOrEqual(30000);
            expect(range.end - range.start).toBeLessThanOrEqual(24);
            expect(offsets[range.start]).toBeLessThanOrEqual(top);
            expect(offsets[range.end]).toBeGreaterThanOrEqual(top + 844);
        }
    });
    test('variable date headers and row boundaries', () => {
        expect(virtualRange([0, 44, 116, 188, 232, 304], 116, 72, 0)).toEqual({start:2,end:4});
        expect(virtualRange([0, 44, 116], -200, 100, 0)).toEqual({start:0,end:1});
    });
    test('empty, hidden and exhausted lists', () => {
        expect(virtualRange([0], 0, 844)).toEqual({start:0,end:0});
        expect(virtualRange(offsets, 0, 0)).toEqual({start:0,end:0});
        expect(virtualRange([0, 72], 1000, 844, 0)).toEqual({start:1,end:1});
    });
});
