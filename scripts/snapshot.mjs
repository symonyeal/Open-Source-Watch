// Write every dashboard route of every region, as served by a running dashboard,
// to the files a snapshot build reads (src/lib/snapshot.ts).
// Usage: node scripts/snapshot.mjs <dashboard URL> <output directory>
import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { snapshotPath } from '../src/lib/snapshot.ts';

const [base, out] = process.argv.slice(2);
const dirs = d => readdirSync(d, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name);
const regions = dirs('regions').filter(r => existsSync(join('regions', r, 'region.toml')));
const routes = dirs('src/app/api');
const KINDS = { products: ['intsum', 'sitrep'] };

let ok = 0, failed = [];
for (const route of routes) {
  for (const conflict of regions) {
    for (const kind of KINDS[route] ?? [null]) {
      const url = `/api/${route}?conflict=${conflict}${kind ? `&kind=${kind}` : ''}`;
      const res = await fetch(base + url, { signal: AbortSignal.timeout(60000) }).catch(e => e);
      if (!(res instanceof Response) || !res.ok) { failed.push(url); continue; }
      const file = join(out, snapshotPath(url));
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, await res.text());
      ok++;
    }
  }
}
console.log(`${ok} routes written; ${failed.length} failed${failed.length ? ': ' + failed.join(' ') : ''}`);
