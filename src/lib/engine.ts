// Open-Source-Watch seam for routes the recording proxy cannot serve
// (docs/specs/2026-10-20-open-source-watch-design.md § Seam). With ENGINE_URL
// set, the route returns the engine's view of the same JSON shape, built from its
// store. Unset, or if the engine does not answer, the route runs upstream's code.
import { NextResponse } from 'next/server';

export async function fromEngine(req: Request): Promise<NextResponse | null> {
  const base = process.env.ENGINE_URL;
  if (!base) return null;
  const { pathname, search } = new URL(req.url);
  try {
    const res = await fetch(`${base}${pathname}${search}`, {
      cache: 'no-store',
      signal: AbortSignal.timeout(8000),
    });
    if (!res.ok) return null;
    return new NextResponse(await res.text(), {
      headers: {
        'Content-Type': 'application/json',
        'Cache-Control': 'no-cache, no-store, must-revalidate',
        'X-Served-By': 'engine',
      },
    });
  } catch {
    return null;
  }
}
