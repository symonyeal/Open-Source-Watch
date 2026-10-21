// Open-Source-Watch seam (docs/specs/2026-10-20-open-source-watch-design.md § Seam).
// With ENGINE_URL set, an API route forwards its request to the Python engine,
// which serves the same JSON shape from its store. Unset, or if the engine does
// not answer, the route runs upstream's own fetch: the app always works.
import { NextResponse } from 'next/server';
import { fetchWithTimeout } from './fetcher';

export async function fromEngine(req: Request): Promise<NextResponse | null> {
  const base = process.env.ENGINE_URL;
  if (!base) return null;
  const { pathname, search } = new URL(req.url);
  try {
    const res = await fetchWithTimeout(`${base}${pathname}${search}`, { timeout: 8000 });
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
