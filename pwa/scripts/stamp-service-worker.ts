import { createHash } from 'node:crypto';
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join, relative } from 'node:path';

// Every changed build must change sw.js, including hotfixes without a desktop version bump.
export async function stampServiceWorker(directory: string, version: string) {
    const files: string[] = [];
    async function collect(folder: string) {
        for (const entry of await readdir(folder, { withFileTypes: true })) {
            const path = join(folder, entry.name);
            if (entry.isDirectory()) await collect(path);
            else if (entry.isFile()) files.push(path);
        }
    }
    await collect(directory);
    const hash = createHash('sha256');
    for (const file of files.sort()) {
        hash.update(relative(directory, file));
        const bytes = await readFile(file);
        hash.update(relative(directory, file) === 'sw.js'
            ? bytes.toString().replace(/^const CACHE_NAME = .+;$/m, 'const CACHE_NAME = "BUILD";')
                .replace(/^const BUILD_ASSETS = .+;$/m, 'const BUILD_ASSETS = [];')
            : bytes);
    }
    const build = hash.digest('hex').slice(0, 16);
    const path = join(directory, 'sw.js');
    const source = await readFile(path, 'utf8');
    if (!/^const CACHE_NAME = .+;$/m.test(source)) throw new Error('Service worker cache marker missing');
    const assets = files.map(file => relative(directory, file))
        .filter(file => file.startsWith('assets/') && /\.(js|css)$/.test(file))
        .sort().map(file => `/${file}`);
    if (!/^const BUILD_ASSETS = .+;$/m.test(source)) throw new Error('Service worker asset marker missing');
    await writeFile(path, source.replace(/^const CACHE_NAME = .+;$/m,
        `const CACHE_NAME = ${JSON.stringify(`dmxmoney-shell-${version}-${build}`)};`)
        .replace(/^const BUILD_ASSETS = .+;$/m, `const BUILD_ASSETS = ${JSON.stringify(assets)};`));
    console.log(`PWA cache: ${version}-${build}`);
}

if (import.meta.main) {
    const pkg = JSON.parse(await readFile('package.json', 'utf8')) as { version: string };
    await stampServiceWorker('dist', pkg.version);
}
