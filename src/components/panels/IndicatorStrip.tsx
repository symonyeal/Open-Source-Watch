'use client';

// Open-Source-Watch: indications and warnings. One chip per PIR, coloured by its
// worst indicator; the tooltip gives the question, each indicator's value against
// its thresholds, and the competing hypotheses ranked by inconsistent evidence.
// INTSUM / SITREP open the engine's written product. Hidden without an engine.
import { useState } from 'react';
import { useConflict } from '@/lib/conflicts/context';
import { useConflictFeed } from '@/lib/hooks';
import { apiUrl } from '@/lib/snapshot';

interface Indicator {
  id: string; name: string; status: string; count: number;
  metric?: string;
  window_h?: number; watch?: number; trigger?: number; z?: number | null;
}
interface Hypothesis { id: string; text: string; inconsistent: string[] }
interface Pir { pir: string; question: string; status: 'answered' | 'open' | 'gap'; indicators: Indicator[]; hypotheses: Hypothesis[] }

const COLOR: Record<string, string> = { triggered: 'var(--red)', watch: '#ffaa00', inactive: 'var(--green)', observed: 'var(--blue)', gap: 'var(--text-secondary)' };

const value = (i: Indicator) => i.window_h !== undefined
  ? `${i.count} in ${i.window_h}h (${i.metric === 'event_count' ? 'reporting count; no calibrated threshold' : `watch ${i.watch}, trigger ${i.trigger}`})`
  : `${i.count} observed AIS crossings, z ${i.z ?? 'n/a'}`;

export default function IndicatorStrip() {
  const { key } = useConflict();
  const { data } = useConflictFeed<Pir[]>('/api/indicators', 60000);
  const [product, setProduct] = useState<string | null>(null);
  if (!data?.length) return null;

  const open = async (kind: 'intsum' | 'sitrep') => {
    setProduct('Writing…');
    try {
      const res = await fetch(apiUrl(`/api/products?conflict=${key}&kind=${kind}`), { cache: 'no-store' });
      const json = await res.json();
      setProduct(json?.text ?? 'No product: the engine did not answer.');
    } catch {
      setProduct('No product: the engine did not answer.');
    }
  };

  return (
    <div className="flex items-center gap-3 px-4 py-1 text-[9px] border-t border-[var(--border-color)] overflow-x-auto">
      <span className="text-[var(--text-secondary)] tracking-[2px]">I&amp;W</span>
      {data.map(p => {
        const worst = p.indicators.some(i => i.status === 'triggered') ? 'triggered'
          : p.indicators.some(i => i.status === 'watch') ? 'watch' : p.status === 'gap' ? 'gap'
            : p.indicators.some(i => i.status === 'observed') ? 'observed' : 'inactive';
        const tip = [
          p.question,
          ...p.indicators.map(i => `${i.id} ${i.name}: ${value(i)} ${i.status}`),
          ...p.hypotheses.map((h, k) => `${k === 0 ? 'ACH best' : 'ACH'} ${h.id} ${h.text}: ${h.inconsistent.length} inconsistent`),
        ].join('\n');
        return (
          <span key={p.pir} title={tip} className="flex items-center gap-1 whitespace-nowrap">
            <span className="w-1.5 h-1.5 rounded-full" style={{ background: COLOR[worst] }} />
            <span style={{ color: COLOR[worst] }}>{p.pir}</span>
            <span className="text-[var(--text-secondary)]">{worst === 'gap' ? 'COLLECTION GAP' : worst.toUpperCase()}</span>
          </span>
        );
      })}
      <span className="ml-auto flex gap-2">
        {(['intsum', 'sitrep'] as const).map(k => (
          <button key={k} onClick={() => open(k)} className="px-2 border border-[var(--border-color)] rounded text-[var(--cyan)]">
            {k.toUpperCase()}
          </button>
        ))}
      </span>
      {product !== null && (
        <div className="fixed inset-0 z-[2000] bg-black/70 flex items-center justify-center" onClick={() => setProduct(null)}>
          <pre
            className="panel max-w-3xl max-h-[80vh] overflow-auto p-4 text-[11px] whitespace-pre-wrap text-[var(--text-primary)]"
            onClick={e => e.stopPropagation()}
          >
            {product}
          </pre>
        </div>
      )}
    </div>
  );
}
