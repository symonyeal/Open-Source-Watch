'use client';

import { useConflict } from '@/lib/conflicts/context';
import { CONFLICT_KEYS, CONFLICTS } from '@/lib/conflicts';

// Theater selector. Open-Source-Watch: a drop-down, since regions grow beyond the
// two upstream theaters (regions/*/region.toml).
export default function ConflictToggle() {
  const { key, setConflict } = useConflict();

  return (
    <div className="flex items-center gap-1">
      <span className="text-[8px] text-[var(--text-secondary)] tracking-[2px] hidden md:inline">
        THEATER
      </span>
      <select
        value={key}
        onChange={e => setConflict(e.target.value)}
        className="text-[9px] font-bold tracking-[1px] px-2 py-1 rounded border border-[var(--border-color)] bg-[var(--bg-secondary)] text-[var(--cyan)]"
        title="Switch the dashboard to another region"
      >
        {CONFLICT_KEYS.map(k => (
          <option key={k} value={k}>{CONFLICTS[k].label}</option>
        ))}
      </select>
    </div>
  );
}
