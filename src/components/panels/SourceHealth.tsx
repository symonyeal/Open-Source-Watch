'use client';

// Open-Source-Watch: collection health of the active region's sources
// (design § Errors: ok, stale, failed, unknown). Hidden without an engine.
import { useConflictFeed } from '@/lib/hooks';

interface Source {
  id: string;
  reliability: string;
  health: 'ok' | 'stale' | 'failed' | 'unknown';
  last_status: string | null;
}

const COLOR = { ok: 'var(--green)', stale: '#ffaa00', failed: 'var(--red)', unknown: 'var(--text-secondary)' };

export default function SourceHealth() {
  const { data } = useConflictFeed<Source[]>('/api/sources', 60000);
  if (!data?.length) return null;
  const n = (h: Source['health']) => data.filter(s => s.health === h).length;
  const bad = data.filter(s => s.health !== 'ok').map(s => `${s.id} [${s.reliability}] ${s.health}${s.last_status ? ` (${s.last_status})` : ''}`);
  return (
    <span title={bad.join('\n') || 'all sources ok'} className="flex items-center gap-2">
      COLLECTION:
      {(['ok', 'stale', 'failed', 'unknown'] as const).map(h => n(h) > 0 && (
        <span key={h} style={{ color: COLOR[h] }}>{n(h)} {h.toUpperCase()}</span>
      ))}
    </span>
  );
}
