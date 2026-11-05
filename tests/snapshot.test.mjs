import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { snapshot } from '../scripts/snapshot.mjs';

const work = resolve('../agent-kit/.migration/work/osw-snapshot-tests');
mkdirSync(work, { recursive: true });
writeFileSync(join(work, 'README.md'), '# Snapshot acceptance checks\n\nRetained output from tests/snapshot.test.mjs. Each run uses a separate directory.\n');
const output = name => join(mkdtempSync(join(work, `${name}-`)), 'public');
const response = body => new Response(JSON.stringify(body), {
  headers: { 'x-served-by': 'engine', 'x-osw-generation': 'fixture-1', 'x-osw-mode': 'recorded' },
});
const options = { regions: ['test'], strict: true };

test('missing provenance and mixed generations reject the snapshot', async () => {
  for (const generation of [null, 'unknown', 'fixture-2']) {
    let n = 0;
    await assert.rejects(snapshot('http://fixture.invalid', output('generation'), {
      ...options, routes: ['news', 'sources'], fetcher: async () => {
        const r = response([]);
        if (++n === 2) {
          if (generation === null) r.headers.delete('x-osw-generation');
          else r.headers.set('x-osw-generation', generation);
        }
        return r;
      },
    }), /missing engine generation|mixed engine generations/);
  }
});

test('all-503 extraction exits nonzero and never exposes partial output', () => {
  const out = output('all-503');
  const script = `globalThis.fetch=async()=>new Response('',{status:503}); process.argv=['node','scripts/snapshot.mjs', 'http://audit.invalid', ${JSON.stringify(out)}]; await import('./scripts/snapshot.mjs');`;
  const run = spawnSync(process.execPath, ['--input-type=module', '-e', script], { encoding: 'utf8' });
  assert.equal(run.status, 1, run.stderr);
  assert.match(run.stderr, /0 routes written; 56 failed/);
  assert.equal(existsSync(join(out, 'api')), false);
});

test('one required failure rejects otherwise valid extraction', async () => {
  const out = output('partial');
  await assert.rejects(snapshot('http://fixture.invalid', out, {
    ...options, routes: ['news', 'sources'],
    fetcher: async url => url.pathname.endsWith('/sources') ? new Response('', { status: 503 }) : response([]),
  }), /1 routes written; 1 failed/);
  assert.equal(existsSync(join(out, 'api')), false);
});

test('HTML, null products, missing fields and silent engine fallback fail acceptance', async () => {
  for (const [route, body] of [['news', '<html>error</html>'], ['products', null], ['signals', {}]]) {
    await assert.rejects(snapshot('http://fixture.invalid', output('invalid'), {
      ...options, routes: [route], fetcher: async () => response(body),
    }), /failed/);
  }
  await assert.rejects(snapshot('http://fixture.invalid', output('fallback'), {
    ...options, routes: ['sources'], fetcher: async () => new Response('[]'),
  }), /recorded route fell back/);
});

test('accepted snapshot reconciles dynamic products, bytes, provenance and explicit unavailability', async () => {
  const out = output('accepted');
  const board = [{ indicators: [{ status: 'triggered', metric: 'unusual', areas: [{ area: 'Kharkiv Oblast', count: 4 }] }] }];
  const m = await snapshot('http://fixture.invalid', out, {
    ...options, routes: ['indicators', 'products', 'alerts', 'news'],
    codeRevision: 'fixture-code', dataRevision: 'fixture-data',
    fetcher: async url => response({
      '/api/indicators': board,
      '/api/products': { text: 'Fixture report', t: '2026-11-05T00:00:00Z' },
      '/api/alerts': { alerts: [], status: 'UNAVAILABLE' },
      '/api/news': [],
    }[url.pathname]),
  });
  assert.equal(m.accepted, true);
  assert.equal(m.code_revision, 'fixture-code');
  assert.equal(m.data_revision, 'fixture-data');
  assert.equal(m.routes.length, 6);
  assert.equal(m.required.length, new Set(m.required).size);
  assert.ok(m.routes.some(r => r.url.includes('area=Kharkiv%20Oblast')));
  assert.equal(m.routes.find(r => r.url.startsWith('/api/alerts')).status, 'unavailable');
  assert.deepEqual(JSON.parse(readFileSync(join(out, 'api/snapshot-manifest.json'))), m);
  for (const r of m.routes) {
    assert.equal(createHash('sha256').update(readFileSync(join(out, r.path))).digest('hex'), r.sha256);
    assert.equal(r.mode, 'recorded');
    assert.equal(r.generation, 'fixture-1');
    assert.equal(r.observed_at, null);
  }
  await assert.rejects(snapshot('http://fixture.invalid', out, {
    ...options, routes: ['news'], fetcher: async () => response([]),
  }), /already exists/);
});
