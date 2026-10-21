import { NextResponse } from 'next/server';
import { fromEngine } from '@/lib/engine';

export const dynamic = 'force-dynamic';

// Open-Source-Watch: a fresh INTSUM or SITREP (?kind=sitrep) of the active region,
// written by the engine (engine/produce.py). Without an engine there is none.
export async function GET(req: Request) {
  return (await fromEngine(req)) ?? NextResponse.json(null);
}
