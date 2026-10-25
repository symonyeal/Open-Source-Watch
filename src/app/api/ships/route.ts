import { NextResponse } from 'next/server';

export const dynamic = 'force-dynamic';

export async function GET() {
  return NextResponse.json({
    totalTracked: 0,
    ships: [],
    note: 'No dated ship-position source is configured; static estimates are withheld.',
  }, {
    headers: { 'Cache-Control': 'public, s-maxage=300, stale-while-revalidate=120' },
  });
}

