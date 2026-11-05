'use client';

import { useState } from 'react';
import NewsFeed from '@/components/panels/NewsFeed';
import TelegramPanel from '@/components/panels/TelegramPanel';
import SourceHealth from '@/components/panels/SourceHealth';
import IndicatorStrip from '@/components/panels/IndicatorStrip';
import ConflictToggle from '@/components/ConflictToggle';
import ContextBoard from '@/components/panels/ContextBoard';
import SignalsBoard from '@/components/panels/SignalsBoard';
import { SNAPSHOT } from '@/lib/snapshot';

const views = { context: 'Energy & trade', signals: 'Disruption signals', reports: 'Supporting reports' };
type View = keyof typeof views;

export default function Dashboard() {
  const [view, setView] = useState<View>('context');
  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <a href="#evidence" className="sr-only focus:not-sr-only focus:p-3">Skip to evidence</a>
      <header className="border-b border-[var(--border-color)] bg-[var(--bg-secondary)] shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-4 px-4 py-3">
          <div>
            <h1 className="text-sm font-bold tracking-wide text-[var(--cyan)]">Open-Source-Watch</h1>
            <p className="mt-1 text-xs text-[var(--text-secondary)]">
              {SNAPSHOT ? 'Snapshot ' + SNAPSHOT.slice(0, 16).replace('T', ' ') + ' UTC' : 'Public-source evidence'}
            </p>
          </div>
          <ConflictToggle />
          <nav aria-label="Evidence view" className="flex flex-wrap gap-2">
            {(Object.keys(views) as View[]).map(v => (
              <button key={v} onClick={() => setView(v)} aria-pressed={view === v}
                className={`min-h-11 px-3 py-2 border rounded text-xs focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--cyan)] ${view === v ? 'text-[var(--cyan)] border-[var(--cyan)]' : 'text-[var(--text-secondary)] border-[var(--border-color)] hover:text-[var(--text-primary)]'}`}>
                {views[v]}
              </button>
            ))}
          </nav>
        </div>
        <IndicatorStrip />
      </header>
      <main id="evidence" className="flex-1 min-h-0 overflow-auto p-3" aria-label={views[view]}>
        {view === 'context' ? <ContextBoard /> : view === 'signals' ? <SignalsBoard /> : (
          <div className="flex h-full min-h-0 flex-col gap-3">
            <p className="text-sm text-[var(--text-secondary)]">Reports provide leads and competing accounts. Repeated coverage is not independent confirmation; reported incidents remain sensitive to translation and classification errors.</p>
            <div className="grid flex-1 min-h-0 grid-cols-1 auto-rows-[30rem] gap-3 lg:grid-cols-2 lg:auto-rows-auto">
              <NewsFeed />
              <TelegramPanel />
            </div>
          </div>
        )}
      </main>
      <footer className="flex flex-wrap gap-3 border-t border-[var(--border-color)] bg-[var(--bg-secondary)] px-4 py-2 text-xs text-[var(--text-secondary)] shrink-0">
        <span>Measurements retain their observation dates. Vessel counts are not cargo volume.</span>
        <SourceHealth />
        <a href="https://github.com/symonyeal/Open-Source-Watch/blob/main/docs/DATA-CONTEXT.md" className="text-[var(--cyan)] underline">Sources & methods</a>
      </footer>
    </div>
  );
}
