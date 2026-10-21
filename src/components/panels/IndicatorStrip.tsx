'use client';

// Open-Source-Watch: indications and warnings. One chip per PIR, coloured by its
// status; the tooltip gives the question and each indicator's count against its
// watch / trigger thresholds (design § Data model). Hidden without an engine.
import { useConflictFeed } from '@/lib/hooks';

interface Indicator { id: string; name: string; count: number; window_h: number; watch: number; trigger: number; status: string }
interface Pir { pir: string; question: string; status: 'answered' | 'open' | 'gap'; indicators: Indicator[] }

const COLOR: Record<string, string> = { triggered: 'var(--red)', watch: '#ffaa00', inactive: 'var(--green)', gap: 'var(--text-secondary)' };

export default function IndicatorStrip() {
  const { data } = useConflictFeed<Pir[]>('/api/indicators', 60000);
  if (!data?.length) return null;
  return (
    <div className="flex items-center gap-3 px-4 py-1 text-[9px] border-t border-[var(--border-color)] overflow-x-auto">
      <span className="text-[var(--text-secondary)] tracking-[2px]">I&amp;W</span>
      {data.map(p => {
        const worst = p.indicators.some(i => i.status === 'triggered') ? 'triggered'
          : p.indicators.some(i => i.status === 'watch') ? 'watch' : p.status === 'gap' ? 'gap' : 'inactive';
        const tip = [p.question, ...p.indicators.map(i => `${i.id} ${i.name}: ${i.count} in ${i.window_h}h (watch ${i.watch}, trigger ${i.trigger}) ${i.status}`)].join('\n');
        return (
          <span key={p.pir} title={tip} className="flex items-center gap-1 whitespace-nowrap">
            <span className="w-1.5 h-1.5 rounded-full" style={{ background: COLOR[worst] }} />
            <span style={{ color: COLOR[worst] }}>{p.pir}</span>
            <span className="text-[var(--text-secondary)]">{worst === 'gap' ? 'COLLECTION GAP' : worst.toUpperCase()}</span>
          </span>
        );
      })}
    </div>
  );
}
