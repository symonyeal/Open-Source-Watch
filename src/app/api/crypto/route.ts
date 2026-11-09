import { NextResponse } from 'next/server';

import { fetchWithTimeout } from '@/lib/fetcher';
import GLOBAL from '../../../../regions/global.json';

export const dynamic = 'force-dynamic';

// CoinGecko free API — no API key required
const COIN_META: Record<string, { name: string; symbol: string }> = Object.fromEntries(
  GLOBAL.coins.map((x) => [x.id, { name: x.name, symbol: x.symbol }]),
);
const COINS = GLOBAL.coins.map((x) => x.id);

export async function GET() {
  try {
    const ids = COINS.join(',');
    const url = `https://api.coingecko.com/api/v3/simple/price?ids=${ids}&vs_currencies=usd&include_24hr_change=true`;

    const res = await fetchWithTimeout(url, {
      timeout: 8000,
      headers: { 'User-Agent': 'IronSight/1.0', Accept: 'application/json' },
    });

    if (!res.ok) throw new Error('CoinGecko API failed');
    const data = await res.json();

    const prices = COINS.map(id => {
      const coin = data[id];
      const meta = COIN_META[id];
      if (!coin) return { name: meta.name, symbol: meta.symbol, price: 0, changePercent: 0, error: true };

      return {
        name: meta.name,
        symbol: meta.symbol,
        price: coin.usd,
        changePercent: Math.round((coin.usd_24h_change || 0) * 100) / 100,
      };
    });

    return NextResponse.json(prices, {
      headers: { 'Cache-Control': 'public, s-maxage=300, stale-while-revalidate=120' },
    });
  } catch {
    return NextResponse.json({ error: 'Failed to fetch crypto prices' }, { status: 500 });
  }
}
