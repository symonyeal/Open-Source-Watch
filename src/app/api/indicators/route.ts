import { NextResponse } from 'next/server';
import { fromEngine } from '@/lib/engine';

export const dynamic = 'force-dynamic';

// Open-Source-Watch: PIR and indicator status of the active region (engine
// analyze.board). Without an engine there is nothing to report.
export async function GET(req: Request) {
  return (await fromEngine(req)) ?? NextResponse.json([]);
}
