'use client';

// Open-Source-Watch: GNSS integrity cells on the theater map. The latest gpsjam.org
// day's H3 cells where 2-10 % (medium) or over 10 % (high) of aircraft reported
// low navigation accuracy (engine/gnss.py). Kept out of ConflictMap so the
// upstream component changes by one hook call and one toggle.
import { useEffect, useMemo, useRef } from 'react';
import type * as Leaflet from 'leaflet';
import { useConflictFeed } from '@/lib/hooks';
import { esc } from '@/lib/escapeHtml';
import type { SignalsResponse } from '@/lib/types/signals';

export const GNSS_COLOR = { medium: '#ffaa00', high: '#ff3366' } as const;

export function useGnssLayer(map: React.RefObject<Leaflet.Map | null>, mounted: boolean, show: boolean) {
  const { data } = useConflictFeed<SignalsResponse>('/api/signals', 300000);
  const layer = useRef<Leaflet.LayerGroup | null>(null);
  const cells = useMemo(() => data?.gnss?.map.cells ?? [], [data]);
  const day = data?.gnss?.map.date ?? null;

  useEffect(() => {
    if (!mounted || !map.current) return;
    let cancelled = false;
    import('leaflet').then(L => {
      if (cancelled || !map.current) return;
      layer.current ??= L.layerGroup();
      layer.current.clearLayers();
      for (const c of cells) {
        L.polygon(c.boundary, {
          color: GNSS_COLOR[c.band], weight: 1, opacity: 0.7, fillColor: GNSS_COLOR[c.band], fillOpacity: 0.18,
        }).bindTooltip(
          `GNSS ${c.band}${c.near ? ` near ${esc(c.near)}` : ''}: ${c.pct}% of ${c.good + c.bad} aircraft` +
          ` (${c.bad} low accuracy, first discounted)` +
          `<br>gpsjam.org, ${day} UTC. Jamming is one possible cause, not a finding.`,
          { className: 'city-label', sticky: true },
        ).addTo(layer.current);
      }
      if (show) layer.current.addTo(map.current);
      else layer.current.remove();
    });
    return () => { cancelled = true; };
  }, [mounted, map, cells, day, show]);

  return cells.length;
}
