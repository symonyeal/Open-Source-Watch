'use client';

import dynamic from 'next/dynamic';
import MetricsBar from '@/components/panels/MetricsBar';
import ThreatClock from '@/components/panels/ThreatClock';
import NewsFeed from '@/components/panels/NewsFeed';
import OilPanel from '@/components/panels/OilPanel';
import MarketsPanel from '@/components/panels/MarketsPanel';
import ConflictFeed from '@/components/panels/ConflictFeed';
import TelegramPanel from '@/components/panels/TelegramPanel';
import FlightsPanel from '@/components/panels/FlightsPanel';
import StrikesPanel from '@/components/panels/StrikesPanel';
import AlertsPanel from '@/components/panels/AlertsPanel';
import SatellitePanel from '@/components/panels/SatellitePanel';
import NavalPanel from '@/components/panels/NavalPanel';
import RegionalAlertsPanel from '@/components/panels/RegionalAlertsPanel';
import CryptoPanel from '@/components/panels/CryptoPanel';
import PolymarketPanel from '@/components/panels/PolymarketPanel';
import SourceHealth from '@/components/panels/SourceHealth';
import IndicatorStrip from '@/components/panels/IndicatorStrip';
import ConflictToggle from '@/components/ConflictToggle';
import ContextBoard from '@/components/panels/ContextBoard';
import { useConflict } from '@/lib/conflicts/context';
import { SNAPSHOT } from '@/lib/snapshot';
import { useState, useEffect } from 'react';

const ConflictMap = dynamic(() => import('@/components/map/ConflictMap'), {
  ssr: false,
  loading: () => <div className="panel h-full loading-shimmer" />,
});

export default function Dashboard() {
  const { key: conflictKey, config } = useConflict();
  const [uptime, setUptime] = useState(0);
  const [view, setView] = useState<'monitor' | 'context'>('monitor');

  useEffect(() => {
    const start = Date.now();
    const id = setInterval(() => {
      setUptime(Math.floor((Date.now() - start) / 1000));
    }, 1000);
    return () => clearInterval(id);
  }, []);

  const formatUptime = (s: number) => {
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${sec.toString().padStart(2, '0')}`;
  };

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      {/* Top bar */}
      <header className="border-b border-[var(--border-color)] bg-[var(--bg-secondary)] shrink-0">
        <div className="flex flex-wrap items-center justify-between gap-2 px-4 py-1.5">
          <div className="flex shrink-0 items-center gap-3">
            <div className="relative w-8 h-8">
              <div className="absolute inset-0 rounded-full border border-[var(--cyan)] opacity-30" />
              <div className="absolute inset-1 rounded-full border border-[var(--cyan)] opacity-20" />
              <div className="absolute inset-2 rounded-full border border-[var(--cyan)] opacity-10" />
              <div
                className="absolute bottom-1/2 left-1/2 -ml-px w-0.5 h-4 bg-[var(--cyan)] origin-bottom radar-sweep"
              />
              <div className="absolute top-1/2 left-1/2 w-1.5 h-1.5 bg-[var(--cyan)] rounded-full -translate-x-1/2 -translate-y-1/2" />
            </div>
            <div>
              <h1 className="text-sm font-bold tracking-[3px] text-[var(--cyan)]">OPEN-SOURCE-WATCH</h1>
              <p className="text-[8px] text-[var(--text-secondary)] tracking-[2px]">
                {SNAPSHOT ? `SNAPSHOT ${SNAPSHOT.slice(0, 16).replace('T', ' ')} UTC // NOT LIVE` : 'OSINT COMMAND CENTER // UNCLASSIFIED'}
              </p>
            </div>
          </div>
          <div className="hidden lg:block min-w-0 flex-1"><MetricsBar /></div>
          <div className="flex flex-wrap items-center gap-3 text-[9px] text-[var(--text-secondary)]">
            <ConflictToggle />
            <nav aria-label="Dashboard view" className="flex gap-2">
              {(['monitor', 'context'] as const).map(v => (
                <button key={v} onClick={() => setView(v)} aria-pressed={view === v}
                  className={`whitespace-nowrap px-2 py-1 border rounded ${view === v ? 'text-[var(--cyan)] border-[var(--cyan)]' : 'border-[var(--border-color)]'}`}>
                  {v === 'monitor' ? 'MONITOR' : 'ENERGY & TRADE'}
                </button>
              ))}
            </nav>
            <span className="hidden xl:inline">SESSION {formatUptime(uptime)}</span>
            <span className="hidden md:flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-[var(--cyan)]" />
              POLLING
            </span>
          </div>
        </div>
        <div className="border-t border-[var(--border-color)]">
          <ThreatClock />
        </div>
        <IndicatorStrip />
      </header>

      {/* Main grid - 3 rows */}
      {view === 'context' ? <main className="flex-1 min-h-0 overflow-auto p-3"><ContextBoard /></main> : (
      <main className="flex-1 min-h-0 grid grid-cols-1 auto-rows-[22rem] gap-1 p-1 overflow-y-auto
        lg:grid-cols-12 lg:auto-rows-auto lg:[grid-template-rows:2fr_1.5fr_1.5fr] lg:overflow-hidden"
      >
        {/* === ROW 1: Map center, News left, Alerts + Markets right === */}
        <div className="lg:col-span-3 min-h-0">
          <NewsFeed />
        </div>
        <div className="lg:col-span-4 min-h-0">
          <ConflictMap key={conflictKey} className="h-full" />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <AlertsPanel />
        </div>
        <div className="lg:col-span-3 min-h-0 flex flex-col gap-1">
          <div className="flex-1 min-h-0">
            <TelegramPanel />
          </div>
        </div>

        {/* === ROW 2: Markets, Strikes, Polymarket, Conflicts, Flights === */}
        <div className="lg:col-span-3 min-h-0">
          <MarketsPanel />
        </div>
        <div className="lg:col-span-3 min-h-0">
          <StrikesPanel />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <PolymarketPanel />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <ConflictFeed />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <FlightsPanel />
        </div>

        {/* === ROW 3: Regional Alerts, Naval, Crypto, Energy, Satellite === */}
        <div className="lg:col-span-3 min-h-0">
          <RegionalAlertsPanel />
        </div>
        <div className="lg:col-span-3 min-h-0">
          <NavalPanel />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <CryptoPanel />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <OilPanel />
        </div>
        <div className="lg:col-span-2 min-h-0">
          <SatellitePanel />
        </div>
      </main>
      )}

      {/* Bottom status bar */}
      <footer className="hidden md:flex border-t border-[var(--border-color)] bg-[var(--bg-secondary)] px-4 py-1 items-center justify-between text-[9px] text-[var(--text-secondary)] shrink-0">
        <span>PUBLIC SOURCES: NEWS | TELEGRAM | YAHOO | EIA / FRED | IMF PORTWATCH | WORLD BANK | NASA FIRMS | ADSB.LOL</span>
        <div className="flex items-center gap-4">
          <SourceHealth />
          <span>ALERTS: 15s | NEWS: 90s | MARKETS: 10m</span>
          <span>ALL DATA: PUBLIC / OSINT</span>
          <span>{config.label} · PUBLIC DATA</span>
        </div>
      </footer>
    </div>
  );
}
