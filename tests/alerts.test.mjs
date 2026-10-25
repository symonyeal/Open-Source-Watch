import { test } from 'node:test';
import assert from 'node:assert/strict';
import { advanceAlerts, readAlertProvider } from '../src/lib/alertState.ts';

const alert = {
  id: 'alert-1', time: '2026-10-24T10:00:00Z', type: 'ROCKET', threat: 'Rocket alert',
  threatOriginal: 'rocket', locations: ['Haifa'], locationsOriginal: ['Haifa'],
  source: 'Home Front Command', active: true,
};

const response = (ok, body) => ({ ok, json: async () => body });

test('provider HTTP, JSON and schema failures are unknown; a valid empty list is clear input', async () => {
  const failedHttp = await readAlertProvider(async () => response(false, []), body => body);
  const failedJson = await readAlertProvider(async () => ({ ok: true, json: async () => {
    throw new Error('invalid JSON');
  } }), body => body);
  const failedShape = await readAlertProvider(async () => response(true, { alerts: [] }),
    body => Array.isArray(body) ? body : null);
  const empty = await readAlertProvider(async () => response(true, []), body => body);

  assert.deepEqual(failedHttp, { status: 'unknown', alerts: [] });
  assert.deepEqual(failedJson, { status: 'unknown', alerts: [] });
  assert.deepEqual(failedShape, { status: 'unknown', alerts: [] });
  assert.deepEqual(empty, { status: 'ok', alerts: [] });
});

test('unsupported provider is unavailable and never clear', () => {
  const state = advanceAlerts([], 'unavailable', [], 1000);
  assert.deepEqual(state, { status: 'UNAVAILABLE', activeCount: 0, alerts: [], sticky: [] });
});

test('an alert stays active beyond 90 seconds while the provider keeps observing it', () => {
  let state = advanceAlerts([], 'ok', [alert], 0);
  for (const time of [60_000, 120_000, 180_000]) {
    state = advanceAlerts(state.sticky, 'ok', [alert], time);
    assert.equal(state.status, 'ACTIVE');
    assert.equal(state.activeCount, 1);
    assert.equal(state.alerts.length, 1);
    assert.equal(state.sticky[0].lastSeen, time);
  }
});

test('valid empty polls keep an ended alert briefly, then expire it; reactivation starts a new lifetime', () => {
  const active = advanceAlerts([], 'ok', [alert], 10_000);
  const cleared = advanceAlerts(active.sticky, 'ok', [], 20_000);
  assert.equal(cleared.status, 'CLEAR');
  assert.equal(cleared.activeCount, 0);
  assert.equal(cleared.alerts[0].active, false);

  const expired = advanceAlerts(cleared.sticky, 'ok', [], 110_001);
  assert.equal(expired.status, 'CLEAR');
  assert.deepEqual(expired.alerts, []);

  const reactivated = advanceAlerts(expired.sticky, 'ok', [alert], 120_000);
  assert.equal(reactivated.status, 'ACTIVE');
  assert.equal(reactivated.sticky[0].lastSeen, 120_000);
});

test('provider failure reports unknown and preserves last-known alerts until their expiry', () => {
  const active = advanceAlerts([], 'ok', [alert], 10_000);
  const unknown = advanceAlerts(active.sticky, 'unknown', [], 20_000);
  assert.equal(unknown.status, 'UNKNOWN');
  assert.equal(unknown.activeCount, 0);
  assert.equal(unknown.alerts.length, 1);
  assert.equal(unknown.alerts[0].active, true);

  const expired = advanceAlerts(unknown.sticky, 'unknown', [], 100_001);
  assert.equal(expired.status, 'UNKNOWN');
  assert.deepEqual(expired.alerts, []);
});
