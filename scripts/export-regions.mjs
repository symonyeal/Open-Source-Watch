// Export what the Python engine reads from the TypeScript code, so the engine
// never hand-copies it (spec § Seam, P0-P2):
//   regions/<key>/conflict.json  each conflict's server config (+ map tables)
//   regions/global.json          market symbols and the Hebrew alert vocabulary
// Run: node scripts/export-regions.mjs [--check]
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { iranIsrael } from '../src/lib/conflicts/iran-israel.ts';
import { russiaUkraine } from '../src/lib/conflicts/russia-ukraine.ts';

const dump = (x) => JSON.stringify(
  x, (_, v) => (v instanceof RegExp ? { regex: v.source, flags: v.flags } : v), 2) + '\n';

const src = (p) => readFileSync(p, 'utf8');
const symbols = (p) => [...src(p).matchAll(/symbol: '([^']+)'/g)].map(m => m[1]);
// Object literal following `name ... = ` up to its closing brace, evaluated as JS.
const literal = (s, name) => {
  const i = s.indexOf('{', s.indexOf(name));
  return Function(`return ${s.slice(i, s.indexOf('\n};', i) + 2)}`)();
};

const files = {
  'regions/global.json': dump({
    markets: symbols('src/app/api/markets/route.ts'),
    oil: symbols('src/app/api/oil/route.ts'),
    coins: JSON.parse(src('src/app/api/crypto/route.ts').match(/const COINS = (\[[^\]]*\])/)[1].replace(/'/g, '"')),
    threatTranslations: literal(src('src/lib/hebrew.ts'), 'THREAT_TRANSLATIONS'),
  }),
};
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
