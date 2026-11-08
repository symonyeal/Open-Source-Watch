'use client';

import { useState } from 'react';
import dynamic from 'next/dynamic';
import AlertsPanel from '@/components/panels/AlertsPanel';
import ConflictFeed from '@/components/panels/ConflictFeed';
import FlightsPanel from '@/components/panels/FlightsPanel';
import NavalPanel from '@/components/panels/NavalPanel';
import RegionalAlertsPanel from '@/components/panels/RegionalAlertsPanel';
import SatellitePanel from '@/components/panels/SatellitePanel';
import StrikesPanel from '@/components/panels/StrikesPanel';
import NewsFeed from '@/components/panels/NewsFeed';
import TelegramPanel from '@/components/panels/TelegramPanel';
import SourceHealth from '@/components/panels/SourceHealth';
import IndicatorStrip from '@/components/panels/IndicatorStrip';
import ConflictToggle from '@/components/ConflictToggle';
import ContextBoard from '@/components/panels/ContextBoard';
import SignalsBoard from '@/components/panels/SignalsBoard';
import { SNAPSHOT } from '@/lib/snapshot';
import { useConflict } from '@/lib/conflicts/context';

const ConflictMap = dynamic(() => import('@/components/map/ConflictMap'), {
  ssr: false,
  loading: () => <div className="panel h-full loading-shimmer" aria-label="Loading theater map" />,
});

const views = { monitor: 'Intelligence monitor', context: 'Energy & trade', signals: 'Disruption signals', reports: 'Supporting reports' };
type View = keyof typeof views;

export default function Dashboard() {
  const [view, setView] = useState<View>('monitor');
  const { key: conflictKey } = useConflict();
  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <a href="#evidence" className="sr-only focus:not-sr-only focus:p-3">Skip to evidence</a>
      <header className="border-b border-[var(--border-color)] bg-[var(--bg-secondary)] shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-4 px-4 py-3">
          <div>
            <h1 className="text-sm font-bold tracking-wide text-[var(--cyan)]">Open-Source-Watch</h1>
            <p className="mt-1 text-xs text-[var(--text-secondary)]">
              {SNAPSHOT ? 'Snapshot ' + SNAPSHOT.slice(0, 16).replace('T', ' ') + ' UTC · Not live' : 'Intelligence monitor · Live feed polling'}
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
      <main id="evidence" className={`flex-1 min-h-0 overflow-auto ${view === 'monitor' ? 'p-1' : 'p-3'}`} aria-label={views[view]}>
        {view === 'monitor' ? (
          <div className="grid grid-cols-1 auto-rows-[22rem] gap-1 lg:h-full lg:min-h-[46rem] lg:grid-cols-12 lg:auto-rows-auto lg:[grid-template-rows:2fr_1fr_1fr]">
            <div className="min-h-0 lg:col-span-6"><ConflictMap key={conflictKey} className="h-full" /></div>
            <div className="min-h-0 lg:col-span-3"><NewsFeed /></div>
            <div className="min-h-0 lg:col-span-3"><AlertsPanel /></div>
            <div className="min-h-0 lg:col-span-4"><TelegramPanel /></div>
            <div className="min-h-0 lg:col-span-4"><StrikesPanel /></div>
            <div className="min-h-0 lg:col-span-4"><ConflictFeed /></div>
            <div className="min-h-0 lg:col-span-3"><FlightsPanel /></div>
            <div className="min-h-0 lg:col-span-3"><NavalPanel /></div>
            <div className="min-h-0 lg:col-span-3"><RegionalAlertsPanel /></div>
            <div className="min-h-0 lg:col-span-3"><SatellitePanel /></div>
          </div>
        ) : view === 'context' ? <ContextBoard /> : view === 'signals' ? <SignalsBoard /> : (
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
        <span>{view === 'context' ? 'Measurements retain their observation dates. Vessel counts are not cargo volume.' : SNAPSHOT ? 'Snapshot data · Measurements retain their observation dates.' : 'Public-source feeds · Collection failures appear in source health.'}</span>
        <SourceHealth />
        <a href="https://github.com/symonyeal/Open-Source-Watch/blob/main/docs/DATA-CONTEXT.md" className="text-[var(--cyan)] underline">Sources & methods</a>
      </footer>
    </div>
  );
}
