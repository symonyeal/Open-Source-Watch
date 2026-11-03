// Static snapshot (GitHub Pages). A snapshot build sets NEXT_PUBLIC_SNAPSHOT to its
// UTC time; each route the dashboard polls, /api/<route>?conflict=<k>[&kind=<kind>][&area=<a>],
// is then read from the file <base>/api/<route>/<k>[-<kind>][-<a>].json (characters
// other than letters, digits, '.', '_' and '-' as '_') written by
// scripts/snapshot.mjs. Unset, the URL is unchanged.
export const SNAPSHOT = process.env.NEXT_PUBLIC_SNAPSHOT ?? '';
const BASE = process.env.NEXT_PUBLIC_BASE_PATH ?? '';

export function snapshotPath(url: string): string {
  const u = new URL(url, 'http://local');
  const k = [u.searchParams.get('conflict'), u.searchParams.get('kind'), u.searchParams.get('area')].filter(Boolean);
  return `${u.pathname}/${k.join('-').replace(/[^A-Za-z0-9._-]+/g, '_')}.json`;
}

export function apiUrl(url: string): string {
  return SNAPSHOT ? BASE + snapshotPath(url) : url;
}
