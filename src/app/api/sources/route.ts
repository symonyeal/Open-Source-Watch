import { NextResponse } from 'next/server';
import { fromEngine } from '@/lib/engine';

export const dynamic = 'force-dynamic';

// Open-Source-Watch: collection health per source, from the engine's
// collection_run table. Without an engine there is nothing to report.
export async function GET(req: Request) {
  return (await fromEngine(req)) ?? NextResponse.json([]);
}
