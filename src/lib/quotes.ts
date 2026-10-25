const UNIT: Record<string, string> = {
  crude_wti: 'USD/bbl',
  crude_brent: 'USD/bbl',
  natural_gas: 'USD/MMBtu',
  heating_oil: 'USD/gal',
  gasoline: 'USD/gal',
};

type Commodity = { type: string; name: string };
type ChartQuote = {
  price: number;
  change: number;
  changePercent: number;
  updated: string | null;
  error?: true;
};

/** Normalize a Yahoo chart result without assuming its instrument or unit. */
export function chartQuote(chart: unknown): ChartQuote {
  const result = isRecord(chart) ? chart : null;
  const meta = result && isRecord(result.meta) ? result.meta : null;
  const price = meta?.regularMarketPrice;
  const timestamp = meta?.regularMarketTime;
  const explicitPrevious = meta?.previousClose;
  const previous = finiteNonzero(explicitPrevious)
    ? explicitPrevious
    : previousDailyClose(result, meta, timestamp);

  if (typeof price !== 'number' || !Number.isFinite(price) ||
      typeof previous !== 'number' || !Number.isFinite(previous) || previous === 0 ||
      typeof timestamp !== 'number' || !Number.isFinite(timestamp) || timestamp <= 0) {
    return chartQuoteError();
  }

  const change = Math.round((price - previous) * 100) / 100;
  const changePercent = Math.round(((price - previous) / previous) * 10000) / 100;
  const updated = new Date(timestamp * 1000);
  if (!Number.isFinite(change) || !Number.isFinite(changePercent) ||
      !Number.isFinite(updated.getTime())) return chartQuoteError();
  return {
    price: Math.round(price * 100) / 100,
    change,
    changePercent,
    updated: updated.toISOString(),
  };
}

/** Add the oil panel's commodity label and unit to normalized chart values. */
export function quoteFromChart(commodity: Commodity, chart: unknown) {
  const quote = chartQuote(chart);
  return quote.error
    ? quoteError(commodity)
    : { ...commodity, ...quote, instrument: 'futures', currency: 'USD', unit: UNIT[commodity.type] };
}

export function chartQuoteError(): ChartQuote {
  return { price: 0, change: 0, changePercent: 0, updated: null, error: true };
}

export function quoteError(commodity: Commodity) {
  return {
    ...commodity,
    instrument: 'futures',
    ...chartQuoteError(),
    currency: 'USD',
    unit: UNIT[commodity.type],
  };
}

function previousDailyClose(
  chart: Record<string, unknown> | null,
  meta: Record<string, unknown> | null,
  marketTime: unknown,
) {
  const period = meta?.currentTradingPeriod;
  const regular = isRecord(period) ? period.regular : null;
  const start = isRecord(regular) ? regular.start : null;
  const timestamps = chart?.timestamp;
  const indicators = chart?.indicators;
  const quote = isRecord(indicators) && Array.isArray(indicators.quote)
    ? indicators.quote[0]
    : null;
  const closes = isRecord(quote) ? quote.close : null;
  if (typeof start !== 'number' || !Number.isFinite(start) ||
      typeof marketTime !== 'number' || !Number.isFinite(marketTime) ||
      !Array.isArray(timestamps) || !Array.isArray(closes)) return null;

  let cutoff = start;
  if (start > marketTime) {
    let currentTime = -Infinity;
    for (const time of timestamps) {
      if (typeof time === 'number' && Number.isFinite(time) &&
          time <= marketTime && time > currentTime) currentTime = time;
    }
    if (!Number.isFinite(currentTime)) return null;
    cutoff = currentTime;
  }

  let latestTime = -Infinity;
  let latestClose: number | null = null;
  for (let i = 0; i < Math.min(timestamps.length, closes.length); i += 1) {
    const time = timestamps[i];
    const close = closes[i];
    if (typeof time === 'number' && Number.isFinite(time) && time < cutoff &&
        typeof close === 'number' && Number.isFinite(close) && time > latestTime) {
      latestTime = time;
      latestClose = close;
    }
  }
  return latestClose;
}

function finiteNonzero(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value !== 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
