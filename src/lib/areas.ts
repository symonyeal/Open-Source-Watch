// Areas (oblasts) the indicator board flags, each of which has an area INTSUM
// (engine/src/produce.rs): those a triggered unusual indicator lists, and those whose
// own count reaches a per-area indicator's watch level. Read by the indicator strip
// and by scripts/snapshot.mjs, which writes their products.
interface Area { area: string; count: number }
interface Indicator { status: string; metric?: string; watch?: number; areas?: Area[] }

export function flaggedAreas(board: { indicators: Indicator[] }[]): string[] {
  const A = board.flatMap(p => p.indicators)
    .filter(i => i.status === 'triggered' || i.status === 'watch')
    .flatMap(i => (i.areas ?? []).filter(a => i.metric === 'unusual' || a.count >= (i.watch ?? Infinity)))
    .map(a => a.area);
  return [...new Set(A)].sort();
}
