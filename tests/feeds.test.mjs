import { test } from 'node:test';
import assert from 'node:assert/strict';
import { feedFailure, feedSuccess, startFeed, visibleFeed } from '../src/lib/feed.ts';

const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};

const noSchedule = () => () => {};

test('feed forwards an empty successful response so it clears prior data', async () => {
  const received = [];
  const feed = startFeed('/api/items', 1000, (value, time) => received.push([value, time]), () => {},
    async () => ({ ok: true, json: async () => [] }), noSchedule);
  await new Promise(resolve => setImmediate(resolve));
  feed.stop();
  assert.equal(received[0][0].length, 0);
  assert.ok(received[0][1] instanceof Date);
});

test('feed does not overlap a slow poll', async () => {
  const response = deferred();
  let calls = 0;
  const feed = startFeed('/api/items', 1000, () => {}, () => {}, () => {
    calls += 1;
    return response.promise;
  }, noSchedule);
  await feed.refresh();
  assert.equal(calls, 1);
  response.resolve({ ok: true, json: async () => ['done'] });
  await new Promise(resolve => setImmediate(resolve));
  await feed.refresh();
  assert.equal(calls, 2);
  feed.stop();
});

test('stopped URL lifecycle aborts and ignores its late response', async () => {
  const oldResponse = deferred();
  const oldValues = [];
  let oldSignal;
  const oldFeed = startFeed('/api/items?conflict=old', 1000, value => oldValues.push(value),
    () => {}, (_url, options) => {
      oldSignal = options.signal;
      return oldResponse.promise;
    }, noSchedule);
  oldFeed.stop();

  const currentValues = [];
  const currentFeed = startFeed('/api/items?conflict=new', 1000,
    value => currentValues.push(value), () => {},
    async () => ({ ok: true, json: async () => ['new'] }), noSchedule);
  oldResponse.resolve({ ok: true, json: async () => ['old'] });
  await new Promise(resolve => setImmediate(resolve));
  currentFeed.stop();

  assert.equal(oldSignal.aborted, true);
  assert.deepEqual(oldValues, []);
  assert.deepEqual(currentValues, [['new']]);
});

test('a URL switch hides the previous region data synchronously', () => {
  const previous = {
    url: '/api/items?conflict=old', data: ['old'], loading: false,
    error: null, lastUpdated: new Date(0),
  };
  assert.deepEqual(visibleFeed('/api/items?conflict=new', previous, null), {
    url: '/api/items?conflict=new', data: null, loading: true,
    error: null, lastUpdated: null,
  });
});

test('a failed first request for a new URL exposes its error and clears old data', () => {
  const previous = {
    url: '/api/items?conflict=old', data: ['old'], loading: false,
    error: null, lastUpdated: new Date(0),
  };
  assert.deepEqual(feedFailure(previous, '/api/items?conflict=new', 'HTTP 503'), {
    url: '/api/items?conflict=new', data: null, loading: false,
    error: 'HTTP 503', lastUpdated: null,
  });
});

test('success stamps the snapshot time and same-URL failure preserves displayed data', () => {
  const time = new Date(1234);
  const success = feedSuccess('/api/items', ['current'], time);
  assert.equal(success.lastUpdated, time);
  assert.deepEqual(feedFailure(success, '/api/items', 'HTTP 503'), {
    ...success, loading: false, error: 'HTTP 503',
  });
});
