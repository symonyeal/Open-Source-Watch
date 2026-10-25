import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { chartQuote, chartQuoteError, quoteError, quoteFromChart } from '../src/lib/quotes.ts';

const commodity = { type: 'crude_wti', name: 'WTI Crude Oil' };

function chart({
  price = 550,
  previousClose,
  chartPreviousClose,
  marketTime = 500,
  sessionStart = 200,
  timestamps = [100, 200],
  closes = [540, 545],
} = {}) {
  return {
    meta: {
      regularMarketPrice: price,
      previousClose,
      chartPreviousClose,
      regularMarketTime: marketTime,
      currentTradingPeriod: { regular: { start: sessionStart } },
    },
    timestamp: timestamps,
    indicators: { quote: [{ close: closes }] },
  };
}

test('quote prefers previousClose and preserves the source market timestamp', () => {
  const quote = quoteFromChart(commodity, chart({
    price: 550,
    previousClose: 540,
    chartPreviousClose: 504.02,
    marketTime: 1792526402,
  }));
  assert.equal(quote.price, 550);
  assert.equal(quote.change, 10);
  assert.equal(quote.changePercent, 1.85);
  assert.equal(quote.updated, new Date(1792526402 * 1000).toISOString());
  assert.equal(quote.unit, 'USD/bbl');
  assert.equal(quote.instrument, 'futures');
  assert.equal(quote.error, undefined);
});

test('same-session fallback excludes the current bar at regular session start', () => {
  const quote = quoteFromChart(commodity, chart({
    price: 550, marketTime: 250, sessionStart: 200,
    timestamps: [100, 200], closes: [540, 545],
  }));
  assert.equal(quote.change, 10);
  assert.equal(quote.changePercent, 1.85);
});

test('post-market fallback still excludes the just-completed current-session bar', () => {
  const quote = quoteFromChart(commodity, chart({
    price: 550, marketTime: 250, sessionStart: 300,
    timestamps: [100, 200], closes: [540, 545],
  }));
  assert.equal(quote.change, 10);
});

test('equity chart uses the previous daily close, not Yahoo chartPreviousClose', () => {
  const body = JSON.parse(readFileSync(new URL('./fixtures/yahoo_LMT.json', import.meta.url), 'utf8'));
  const quote = chartQuote(body.chart.result[0]);
  assert.equal(quote.price, 548.71);
  assert.equal(quote.change, 0.48);
  assert.equal(quote.changePercent, 0.09);
  assert.equal(quote.updated, new Date(1792526402 * 1000).toISOString());
  assert.equal(quote.error, undefined);
  assert.equal('instrument' in quote, false);
});

test('weekend fallback selects the latest completed bar before the next session', () => {
  const quote = quoteFromChart(commodity, chart({
    price: 550, marketTime: 400, sessionStart: 500,
    timestamps: [100, 200, 300], closes: [520, 530, 540],
  }));
  assert.equal(quote.change, 20);
});

test('Yahoo WTI chart fallback ignores chartPreviousClose and selects prior daily close', () => {
  const quote = quoteFromChart(commodity, {
    meta: {
      regularMarketPrice: 103.27,
      chartPreviousClose: 103.66,
      regularMarketTime: 1792789198,
      currentTradingPeriod: { regular: { start: 1792728000 } },
    },
    timestamp: [1792382400, 1792468800, 1792555200, 1792641600, 1792728000],
    indicators: { quote: [{ close: [103.66000366, 101.7900009, 97.1500015, 99.4700012, 103.2699966] }] },
  });
  assert.equal(quote.error, undefined);
  assert.equal(quote.change, 3.8);
  assert.equal(quote.changePercent, 3.82);
});

test('quotes label natural gas and refined products with their trading units', () => {
  const input = chart({ previousClose: 1 });
  assert.equal(quoteFromChart({ type: 'natural_gas', name: 'Gas' }, input).unit, 'USD/MMBtu');
  assert.equal(quoteFromChart({ type: 'heating_oil', name: 'Heating Oil' }, input).unit, 'USD/gal');
  assert.equal(quoteFromChart({ type: 'gasoline', name: 'Gasoline' }, input).unit, 'USD/gal');
});

test('finite zero and negative futures prices are valid; percent uses unrounded prices', () => {
  const negative = quoteFromChart(commodity, chart({ price: -0.5, previousClose: 0.25 }));
  assert.equal(negative.error, undefined);
  assert.equal(negative.price, -0.5);
  assert.equal(negative.change, -0.75);
  assert.equal(negative.changePercent, -300);

  const negativePrevious = quoteFromChart(commodity, chart({ price: -0.5, previousClose: -1 }));
  assert.equal(negativePrevious.error, undefined);
  assert.equal(negativePrevious.changePercent, -50);

  const zero = quoteFromChart(commodity, chart({ price: 0, previousClose: 1 }));
  assert.equal(zero.error, undefined);

  const smallMove = quoteFromChart({ type: 'natural_gas', name: 'Gas' }, chart({
    price: 2.001, previousClose: 2,
  }));
  assert.equal(smallMove.change, 0);
  assert.equal(smallMove.changePercent, 0.05);
});

test('missing, non-finite or unusable chart data is an explicit error without a false timestamp', () => {
  for (const input of [
    undefined,
    chart({ price: NaN, previousClose: 1 }),
    chart({ price: 1, previousClose: 0, timestamps: [null], closes: [null] }),
    chart({ price: 10, previousClose: Infinity, timestamps: [], closes: [] }),
    chart({ price: 10, timestamps: [null], closes: [null] }),
    { meta: { regularMarketPrice: 10, chartPreviousClose: 5, regularMarketTime: 100 },
      timestamp: [1], indicators: { quote: [{ close: [5] }] } },
  ]) {
    const quote = quoteFromChart(commodity, input);
    assert.equal(quote.error, true);
    assert.equal(quote.updated, null);
    assert.equal(quote.price, 0);
  }
  assert.equal(quoteError(commodity).updated, null);
  assert.deepEqual(chartQuoteError(), {
    price: 0, change: 0, changePercent: 0, updated: null, error: true,
  });
});
