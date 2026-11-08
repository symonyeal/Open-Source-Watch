import { NextResponse } from 'next/server';
import { fromEngine } from '@/lib/engine';

export const dynamic = 'force-dynamic';

export async function GET(req: Request) {
  return (await fromEngine(req)) ?? NextResponse.json(
    { error: 'Energy and trade context requires the engine (ENGINE_URL).' },
    { status: 503 },
  );
}
