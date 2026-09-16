import { expect, test } from 'bun:test';
import { compareVersions, selectNewestVersion } from './version';

test('stable release supersedes RC and keeps seen version monotonic', () => {
    expect(compareVersions('2.0.3', '2.0.3-rc.2')).toBeGreaterThan(0);
    expect(compareVersions('2.0.3-rc.2', '2.0.3')).toBeLessThan(0);
    expect(compareVersions('2.0.3-rc.10', '2.0.3-rc.2')).toBeGreaterThan(0);
    expect(compareVersions('v2.0.3+build.1', '2.0.3')).toBe(0);
    expect(selectNewestVersion('2.0.3-rc.2', '2.0.3')).toBe('2.0.3');
});
