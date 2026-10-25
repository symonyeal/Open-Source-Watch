import { NextResponse } from 'next/server';

import { fetchWithTimeout } from '@/lib/fetcher';
import { chartQuote, chartQuoteError } from '@/lib/quotes';

export const dynamic = 'force-dynamic';

const SYMBOLS = [
  { symbol: 'LMT', name: 'Lockheed Martin' },
  { symbol: 'RTX', name: 'Raytheon' },
  { symbol: 'NOC', name: 'Northrop Grumman' },
  { symbol: 'BA', name: 'Boeing' },
  { symbol: 'GD', name: 'General Dynamics' },
  { symbol: 'LHX', name: 'L3Harris' },
  { symbol: '^GSPC', name: 'S&P 500' },
  { symbol: '^DJI', name: 'Dow Jones' },
  { symbol: '^VIX', name: 'VIX (Fear Index)' },
  { symbol: 'GC=F', name: 'Gold' },
  { symbol: 'DX-Y.NYB', name: 'US Dollar Index' },
];

async function fetchYahoo(sym: string) {
  const url = `https://query1.finance.yahoo.com/v8/finance/chart/${encodeURIComponent(sym)}?interval=1d&range=5d`;
  const res = await fetchWithTimeout(url, {
    timeout: 8000,
    headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' },
  });
  if (!res.ok) throw new Error('Failed');
  const data = await res.json();
  return data?.chart?.result?.[0];
}

export async function GET() {
  try {
    const markets = await Promise.all(
      SYMBOLS.map(async (s) => {
        try {
          const quote = chartQuote(await fetchYahoo(s.symbol));
          return {
            symbol: s.symbol,
            name: s.name,
            ...quote,
          };
        } catch {
          return {
            symbol: s.symbol,
            name: s.name,
            ...chartQuoteError(),
          };
        }
      })
    );

    return NextResponse.json(markets, {
      headers: { 'Cache-Control': 'public, s-maxage=300, stale-while-revalidate=120' },
    });
  } catch {
    return NextResponse.json([], { status: 500 });
  }
}
