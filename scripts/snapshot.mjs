// Accept a complete dashboard snapshot before publishing its api directory.
// Usage: node scripts/snapshot.mjs <dashboard URL> <output directory>
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { flaggedAreas } from '../src/lib/areas.ts';
import { snapshotPath } from '../src/lib/snapshot.ts';

const dirs = d => readdirSync(d, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => e.name).sort();
const recorded = new Set(['context', 'signals', 'indicators', 'products', 'sources', 'news', 'telegram']);
const arrayRoutes = new Set(['news', 'conflicts', 'strikes', 'oil', 'markets', 'crypto', 'sources']);
const fields = {
  alerts: ['alerts'], drones: ['drones'], fires: ['events'], flights: ['flights'],
  ships: ['ships'], telegram: ['posts', 'channels'], 'regional-alerts': ['alerts'],
  polymarket: ['markets'], context: ['series', 'sources'], signals: ['sources'],
};
const object = x => x !== null && typeof x === 'object' && !Array.isArray(x);
const sha = text => createHash('sha256').update(text).digest('hex');

// Empty arrays are valid measurements; a missing object/field is not.
function validate(route, body) {
  if (arrayRoutes.has(route)) {
    if (!Array.isArray(body) || !body.every(object)) throw new Error('expected an array of records');
  } else if (route === 'indicators') {
    if (!Array.isArray(body) || !body.every(p => object(p) && Array.isArray(p.indicators))) {
      throw new Error('expected indicator boards');
    }
  } else if (route === 'products') {
    if (!object(body) || typeof body.text !== 'string' || !body.text.trim() || typeof body.t !== 'string') {
      throw new Error('expected a dated, nonempty product');
    }
  } else if (!fields[route]) {
    throw new Error(`no snapshot contract for ${route}`);
  } else if (!object(body) || !fields[route].every(k => Array.isArray(body[k]))) {
    throw new Error(`expected arrays: ${fields[route].join(', ')}`);
  }
}

/** Fetch and validate every region/route, then expose one complete api directory.
 * Existing accepted output is never overwritten; callers supply a fresh directory.
 * Failed staging and its manifest remain available for diagnosis.
 */
export async function snapshot(base, out, {
  regions = dirs('regions').filter(r => existsSync(join('regions', r, 'region.toml'))),
  routes = [...recorded], fetcher = fetch, strict = false,
} = {}) {
  if (!regions.length || !routes.length) throw new Error('snapshot requires regions and routes');
  out = resolve(out);
  if (existsSync(join(out, 'api'))) throw new Error(`snapshot output already exists: ${join(out, 'api')}`);
  mkdirSync(dirname(out), { recursive: true });
  const stage = mkdtempSync(join(dirname(out), `${basename(out)}.snapshot-`));
  const manifest = { schema: 1, accepted: false, fetched_at: new Date().toISOString(), required: [], routes: [] };
  const pending = routes.flatMap(route => regions.flatMap(conflict =>
    (route === 'products' ? ['intsum', 'sitrep'] : [null]).map(kind =>
      `/api/${route}?conflict=${conflict}${kind ? `&kind=${kind}` : ''}`)));
  const paths = new Set();
  for (let n = 0; n < pending.length; n++) {
    const url = pending[n], route = new URL(url, base).pathname.split('/')[2];
    const path = snapshotPath(url).replace(/^\//, '');
    if (paths.has(path)) throw new Error(`duplicate snapshot path: ${path}`);
    paths.add(path);
    manifest.required.push(path);
    const entry = { url, path, status: 'failed', fetched_at: new Date().toISOString() };
    manifest.routes.push(entry);
    try {
      const res = await fetcher(new URL(url, base), { signal: AbortSignal.timeout(60000) });
      entry.http_status = res.status;
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const text = await res.text(), body = JSON.parse(text);
      validate(route, body);
      entry.served_by = res.headers.get('x-served-by');
      entry.mode = res.headers.get('x-osw-mode') ?? 'unverified';
      entry.generation = res.headers.get('x-osw-generation');
      if (strict && recorded.has(route)) {
        if (entry.served_by !== 'engine' || entry.mode !== 'recorded') throw new Error('recorded route fell back from engine');
        if (!entry.generation || entry.generation === 'unknown') throw new Error('missing engine generation');
        if (manifest.generation && manifest.generation !== entry.generation) throw new Error('mixed engine generations');
        manifest.generation = entry.generation;
      }
      // A route's update time is not necessarily the time of its observations.
      entry.updated_at = body?.updated ?? body?.t ?? null;
      entry.observed_at = null;
      entry.status = body?.error || body?.status === 'UNAVAILABLE' ||
        (Array.isArray(body) && body.length > 0 && body.every(x => x.error)) ? 'unavailable' : 'available';
      entry.sha256 = sha(text);
      const file = join(stage, path);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, text);
      if (route === 'indicators') {
        for (const a of flaggedAreas(body)) {
          pending.push(`/api/products?conflict=${new URL(url, base).searchParams.get('conflict')}&kind=intsum&area=${encodeURIComponent(a)}`);
        }
      }
    } catch (e) {
      entry.status = 'failed';
      entry.error = e.message;
    }
  }
  const failed = manifest.routes.filter(r => r.status === 'failed');
  // Re-read the bytes which will actually be published, including dynamic area routes.
  manifest.accepted = !failed.length && manifest.required.length === paths.size &&
    manifest.routes.every(r => sha(readFileSync(join(stage, r.path))) === r.sha256);
  mkdirSync(join(stage, 'api'), { recursive: true });
  writeFileSync(join(stage, 'api', 'snapshot-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  if (!manifest.accepted) throw new Error(`${manifest.routes.length - failed.length} routes written; ${failed.length} failed: ${failed.map(r => `${r.url} (${r.error})`).join(' ')}; staged at ${stage}`);
  mkdirSync(out, { recursive: true });
  renameSync(join(stage, 'api'), join(out, 'api'));
  return manifest;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [base, out] = process.argv.slice(2);
  try {
    if (!base || !out) throw new Error('usage: node scripts/snapshot.mjs <dashboard URL> <output directory>');
    const manifest = await snapshot(base, out, { strict: process.env.OSW_SNAPSHOT_STRICT === '1' });
    console.log(`${manifest.routes.length} routes accepted; manifest: ${join(out, 'api', 'snapshot-manifest.json')}`);
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  }
}
