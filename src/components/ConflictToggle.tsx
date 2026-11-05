'use client';

import { useConflict } from '@/lib/conflicts/context';
import { CONFLICT_KEYS, CONFLICTS } from '@/lib/conflicts';

// Theater selector. Open-Source-Watch: a drop-down, since regions grow beyond the
// two upstream theaters (regions/*/region.toml).
export default function ConflictToggle() {
  const { key, setConflict } = useConflict();

  return (
    <div className="flex items-center gap-1">
      <label htmlFor="region" className="text-xs text-[var(--text-secondary)]">Region</label>
      <select
        id="region"
        value={key}
        onChange={e => setConflict(e.target.value)}
        className="min-h-11 text-sm px-2 py-1 rounded border border-[var(--border-color)] bg-[var(--bg-secondary)] text-[var(--cyan)]"
        title="Switch the dashboard to another region"
      >
        {CONFLICT_KEYS.map(k => (
          <option key={k} value={k}>{CONFLICTS[k].label}</option>
        ))}
      </select>
    </div>
  );
}
