const parseVersion = (value: string): number[] | null => {
    const normalized = value.trim().replace(/^v/i, '').split(/[+-]/, 1)[0];
    if (!normalized) return null;

    const parts = normalized.split('.');
    if (parts.some(part => !/^\d+$/.test(part))) return null;
    return parts.map(Number);
};

export const compareVersions = (left: string, right: string) => {
    const leftParts = parseVersion(left);
    const rightParts = parseVersion(right);

    if (!leftParts && !rightParts) return 0;
    if (!leftParts) return -1;
    if (!rightParts) return 1;

    const length = Math.max(leftParts.length, rightParts.length);
    for (let index = 0; index < length; index += 1) {
        const difference = (leftParts[index] || 0) - (rightParts[index] || 0);
        if (difference !== 0) return difference;
    }
    const prerelease = (value: string) => value.trim().split('+', 1)[0].split('-').slice(1).join('-');
    const leftPre = prerelease(left);
    const rightPre = prerelease(right);
    if (!leftPre || !rightPre) return leftPre ? -1 : rightPre ? 1 : 0;
    const leftIds = leftPre.split('.');
    const rightIds = rightPre.split('.');
    for (let index = 0; index < Math.max(leftIds.length, rightIds.length); index += 1) {
        const a = leftIds[index];
        const b = rightIds[index];
        if (a === undefined) return -1;
        if (b === undefined) return 1;
        if (a === b) continue;
        const aNumeric = /^\d+$/.test(a);
        const bNumeric = /^\d+$/.test(b);
        if (aNumeric && bNumeric) return BigInt(a) < BigInt(b) ? -1 : 1;
        if (aNumeric !== bNumeric) return aNumeric ? -1 : 1;
        return a < b ? -1 : 1;
    }
    return 0;
};

export const selectNewestVersion = (...versions: Array<string | null | undefined>) => {
    return versions.reduce<string | undefined>((selected, candidate) => {
        if (!candidate) return selected;
        if (!selected || compareVersions(candidate, selected) > 0) return candidate;
        return selected;
    }, undefined);
};
