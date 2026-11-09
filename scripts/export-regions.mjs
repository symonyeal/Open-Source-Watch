// Export what the engine reads from the TypeScript code, so the engine
// never hand-copies it (spec § Seam, P0-P2):
//   regions/<key>/conflict.json  each conflict's server config (+ map tables)
// regions/global.json (market symbols, coins, Hebrew alert vocabulary) is the source
// of those lists: the engine and the routes read it, nothing is exported to it.
// Run: node scripts/export-regions.mjs [--check]
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { iranIsrael } from '../src/lib/conflicts/iran-israel.ts';
import { russiaUkraine } from '../src/lib/conflicts/russia-ukraine.ts';

const dump = (x) => JSON.stringify(
  x, (_, v) => (v instanceof RegExp ? { regex: v.source, flags: v.flags } : v), 2) + '\n';

const files = {};
for (const c of [iranIsrael, russiaUkraine]) {
  const { mapCenter, strikeLocations, alertCities } = c.client;
  files[`regions/${c.key}/conflict.json`] = dump(
    { key: c.key, label: c.label, ...c.server, client: { mapCenter, strikeLocations, alertCities } });
}

let stale = 0;
for (const [path, s] of Object.entries(files)) {
  if (process.argv.includes('--check')) {
    let old = '';
    try { old = readFileSync(path, 'utf8'); } catch { /* missing = stale */ }
    if (old.replace(/\r\n/g, '\n') !== s) { console.error(`stale: ${path}`); stale++; }
  } else {
    mkdirSync(path.slice(0, path.lastIndexOf('/')), { recursive: true });
    writeFileSync(path, s);
  }
}
process.exit(stale ? 1 : 0);
