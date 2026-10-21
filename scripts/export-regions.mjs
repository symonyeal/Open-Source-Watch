// Export each conflict's server config to regions/<key>/conflict.json for the
// Python engine. The TypeScript configs stay the single source of the feed
// lists (spec: P0-P2); the engine never hand-copies them.
// Run: node scripts/export-regions.mjs [--check]
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { iranIsrael } from '../src/lib/conflicts/iran-israel.ts';
import { russiaUkraine } from '../src/lib/conflicts/russia-ukraine.ts';

const json = (c) => JSON.stringify(
  { key: c.key, label: c.label, ...c.server, client: { mapCenter: c.client.mapCenter, strikeLocations: c.client.strikeLocations } },
  (_, v) => (v instanceof RegExp ? { regex: v.source, flags: v.flags } : v),
  2,
) + '\n';

let stale = 0;
for (const c of [iranIsrael, russiaUkraine]) {
  const path = `regions/${c.key}/conflict.json`;
  const s = json(c);
  if (process.argv.includes('--check')) {
    let old = '';
    try { old = readFileSync(path, 'utf8'); } catch { /* missing = stale */ }
    if (old !== s) { console.error(`stale: ${path}`); stale++; }
  } else {
    mkdirSync(`regions/${c.key}`, { recursive: true });
    writeFileSync(path, s);
  }
}
process.exit(stale ? 1 : 0);
