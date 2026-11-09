import { NextResponse } from 'next/server';

import { fetchWithTimeout } from '@/lib/fetcher';
import { quoteError, quoteFromChart } from '@/lib/quotes';
import GLOBAL from '../../../../regions/global.json';

export const dynamic = 'force-dynamic';

const COMMODITIES = GLOBAL.oil;

export async function GET() {
  try {
    const prices = await Promise.all(
      COMMODITIES.map(async (c) => {
        try {
          const url = `https://query1.finance.yahoo.com/v8/finance/chart/${c.symbol}?interval=1d&range=5d`;
          const res = await fetchWithTimeout(url, {
            timeout: 8000,
            headers: { 'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' },
          });
          if (!res.ok) throw new Error('Failed');
          const data = await res.json();
          const chart = data?.chart?.result?.[0];
          return quoteFromChart(c, chart);
        } catch {
          return quoteError(c);
        }
      })
    );

    return NextResponse.json(prices, {
      headers: { 'Cache-Control': 'public, s-maxage=300, stale-while-revalidate=120' },
    });
  } catch {
    return NextResponse.json({ error: 'Failed to fetch oil prices' }, { status: 500 });
  }
}
