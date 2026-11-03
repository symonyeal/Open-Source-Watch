// Write every dashboard route of every region, as served by a running dashboard,
// to the files a snapshot build reads (src/lib/snapshot.ts).
// Usage: node scripts/snapshot.mjs <dashboard URL> <output directory>
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { flaggedAreas } from '../src/lib/areas.ts';
import { snapshotPath } from '../src/lib/snapshot.ts';

const [base, out] = process.argv.slice(2);
const dirs = d => readdirSync(d, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name);
const regions = dirs('regions').filter(r => existsSync(join('regions', r, 'region.toml')));
const routes = dirs('src/app/api');
const KINDS = { products: ['intsum', 'sitrep'] };

let ok = 0, failed = [];
const save = async url => {
  const res = await fetch(base + url, { signal: AbortSignal.timeout(60000) }).catch(e => e);
  if (!(res instanceof Response) || !res.ok) { failed.push(url); return null; }
  const text = await res.text();
  const file = join(out, snapshotPath(url));
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, text);
  ok++;
  return text;
};
for (const route of routes) {
  for (const conflict of regions) {
    for (const kind of KINDS[route] ?? [null]) {
      const text = await save(`/api/${route}?conflict=${conflict}${kind ? `&kind=${kind}` : ''}`);
      // The INTSUM of each area the board flags (src/lib/areas.ts).
      if (route === 'indicators' && text) {
        for (const a of flaggedAreas(JSON.parse(text) ?? [])) {
          await save(`/api/products?conflict=${conflict}&kind=intsum&area=${encodeURIComponent(a)}`);
        }
      }
    }
  }
}
console.log(`${ok} routes written; ${failed.length} failed${failed.length ? ': ' + failed.join(' ') : ''}`);
