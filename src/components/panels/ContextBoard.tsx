'use client';

import { useConflictFeed } from '@/lib/hooks';

type ContextPoint = {
  date: string;
  value: number | null;
  tanker?: number | null;
  country?: string;
  name?: string;
};

type ContextMean = {
  mean: number;
  n: number;
  center_date: string;
  window_start: string;
  window_end: string;
};

type ContextSeries = {
  code: string;
  name: string;
  kind: 'fred' | 'wdi' | 'port' | 'chokepoint' | string;
  unit: string;
  source: string;
  source_url: string;
  fetched_at: string;
  health: string;
  latest?: ContextPoint | null;
  age_days?: number | null;
  change_pct?: number | null;
  mean7?: ContextMean | null;
  tanker_mean7?: ContextMean | null;
  countries?: Record<string, ContextPoint>;
};

type ContextSource = {
  id: string;
  url: string;
  region: string;
  reliability: string;
  cadence_s: number;
  health: string;
};

type ContextResponse = {
  region: string;
  countries: string[];
  series: ContextSeries[];
  spread: { date: string; value: number; unit: string } | null;
  sources: ContextSource[];
  updated: string;
};

const COUNTRY_NAMES: Record<string, string> = {
  BGD: 'Bangladesh',
  IND: 'India',
  PAK: 'Pakistan',
  LKA: 'Sri Lanka',
  MMR: 'Myanmar',
  NPL: 'Nepal',
  BTN: 'Bhutan',
  MDV: 'Maldives',
};

const WDI_SERIES = [
  { code: 'EG.IMP.CONS.ZS', label: 'Net energy imports (% of energy use)' },
  { code: 'TM.VAL.FUEL.ZS.UN', label: 'Fuel imports (% of merchandise imports)' },
];

const PANEL_GRID = 'grid grid-cols-1 gap-4 xl:grid-cols-2';
const SECTION = 'rounded border border-[var(--border-color)] bg-[var(--bg-secondary)] p-4';
const SECTION_TITLE = 'mb-3 text-sm font-bold tracking-wide text-[var(--text-primary)]';
const LABEL = 'text-xs text-[var(--text-secondary)]';

export default function ContextBoard() {
  const { data, loading, error } = useConflictFeed<ContextResponse>('/api/context', 300000);
  const series = data?.series ?? [];
  const brent = series.find(item => item.kind === 'fred' && item.code === 'DCOILBRENTEU');
  const wti = series.find(item => item.kind === 'fred' && item.code === 'DCOILWTICO');
  const portWatch = series.filter(item => item.kind === 'port' || item.kind === 'chokepoint');
  const netEnergy = series.find(item => item.kind === 'wdi' && item.code === WDI_SERIES[0].code);
  const fuelImports = series.find(item => item.kind === 'wdi' && item.code === WDI_SERIES[1].code);
  const healthSources = data?.sources ?? [];

  return (
    <section className="panel flex h-full min-h-0 flex-col" aria-labelledby="context-board-title">
      <header className="panel-header">
        <span className="status-dot" style={{ background: 'var(--blue)' }} aria-hidden="true" />
        <h2 id="context-board-title" className="text-inherit">ENERGY, SHIPPING & COUNTRY CONTEXT</h2>
        <span className="ml-auto text-xs font-normal normal-case tracking-normal text-[var(--text-secondary)]">
          Refreshes every 5 min
        </span>
      </header>

      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
        {error && (
          <div className="rounded border border-[var(--amber)]/50 bg-[rgba(255,170,0,0.08)] px-4 py-3 text-sm text-[var(--amber)]" role="status">
            {data
              ? 'The context feed could not refresh. Showing the last stored snapshot where available.'
              : error.includes('503')
                ? 'Context data are unavailable because the engine is not connected for this request.'
                : 'The context feed could not be reached. Retry on the next refresh.'}
          </div>
        )}

        {!data && loading ? (
          <div className="space-y-3" aria-label="Loading regional context">
            <div className="loading-shimmer h-32 rounded" />
            <div className="loading-shimmer h-48 rounded" />
          </div>
        ) : !data ? (
          <div className="rounded border border-[var(--border-color)] bg-[var(--bg-secondary)] px-4 py-6 text-sm text-[var(--text-secondary)]">
            No regional context snapshot is available yet. Source health will appear after the engine records its first collection.
          </div>
        ) : (
          <>
            <div className={PANEL_GRID}>
              <div className="space-y-4">
              <section className={SECTION} aria-labelledby="context-energy-title">
                <h3 id="context-energy-title" className={SECTION_TITLE}>EIA spot prices</h3>
                <p className={`${LABEL} mb-4`}>
                  Crude benchmarks · U.S. EIA data via FRED · USD per barrel
                </p>
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <PriceCard label="Brent spot" item={brent} />
                  <PriceCard label="WTI spot" item={wti} />
                </div>
                <div className="mt-4 border-t border-[var(--border-color)] pt-3">
                  <div className="flex flex-wrap items-baseline justify-between gap-2">
                    <span className={LABEL}>Brent − WTI spread, same observation date</span>
                    <span className="text-base font-bold text-[var(--text-primary)]">
                      {data.spread ? `${formatNumber(data.spread.value)} ${data.spread.unit}` : '—'}
                    </span>
                  </div>
                  <p className="mt-1 text-xs text-[var(--text-secondary)]">
                    {data.spread ? `Both benchmarks dated ${formatDate(data.spread.date)}.` : 'Unavailable until both spot series share an observation date.'}
                  </p>
                </div>
              </section>

              <section className={SECTION} aria-labelledby="context-country-title">
                <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
                  <h3 id="context-country-title" className={SECTION_TITLE}>Country energy import context</h3>
                  <a className="text-xs text-[var(--cyan)] underline underline-offset-2" href="https://data.worldbank.org/" target="_blank" rel="noreferrer">
                    World Bank Indicators
                  </a>
                </div>
                <p className={`${LABEL} mb-3`}>
                  Latest available WDI observation; each cell shows its own year. Negative net energy imports indicate net exports.
                </p>
                <div className="overflow-x-auto">
                  <table className="w-full min-w-[620px] border-collapse text-left text-sm">
                    <thead>
                      <tr className="border-b border-[var(--border-color)] text-xs text-[var(--text-secondary)]">
                        <th scope="col" className="px-2 py-2 font-medium">Country</th>
                        {WDI_SERIES.map(metric => (
                          <th scope="col" key={metric.code} className="px-2 py-2 text-right font-medium">{metric.label}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {(data.countries ?? []).map(code => (
                        <tr key={code} className="border-b border-[rgba(30,58,95,0.4)] last:border-0">
                          <th scope="row" className="whitespace-nowrap px-2 py-2.5 font-medium text-[var(--text-primary)]">
                            {COUNTRY_NAMES[code] ?? code}
                          </th>
                          <CountryValue point={netEnergy?.countries?.[code]} netEnergy />
                          <CountryValue point={fuelImports?.countries?.[code]} />
                        </tr>
                      ))}
                      {(data.countries ?? []).length === 0 && (
                        <tr>
                          <td colSpan={3} className="px-2 py-4 text-[var(--text-secondary)]">Country coverage is not available.</td>
                        </tr>
                      )}
                    </tbody>
                  </table>
                </div>
              </section>
              </div>
              <section className={SECTION} aria-labelledby="context-shipping-title">
                <h3 id="context-shipping-title" className={SECTION_TITLE}>Shipping activity</h3>
                <p className={`${LABEL} mb-3`}>
                  IMF PortWatch daily vessel calls and transits. Centered seven-day smoothing follows the World Bank chokepoints methodology, requiring full windows here.
                </p>
                {portWatch.length === 0 ? (
                  <p className="rounded border border-[var(--border-color)] px-3 py-4 text-sm text-[var(--text-secondary)]">
                    No PortWatch observations are available yet.
                  </p>
                ) : (
                  <div className="max-h-[65vh] space-y-3 overflow-y-auto pr-1">
                    {portWatch.map(item => <ShippingCard key={`${item.kind}:${item.code}`} item={item} />)}
                  </div>
                )}
                <p className="mt-3 text-xs leading-relaxed text-[var(--text-secondary)]">
                  Tanker vessel counts are not crude-oil volume, cargo capacity, or confirmed deliveries.{' '}
                  <a className="text-[var(--cyan)] underline underline-offset-2" href="https://portwatch.imf.org/" target="_blank" rel="noreferrer">
                    UN Global Platform · IMF PortWatch
                  </a>
                </p>
              </section>


            </div>

            <section className={SECTION} aria-labelledby="context-health-title">
              <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
                <h3 id="context-health-title" className={SECTION_TITLE}>Collection health</h3>
                <a href="https://github.com/symonyeal/Open-Source-Watch/blob/main/docs/MARITIME-METHODS.md"
                  target="_blank" rel="noreferrer" className="text-xs text-[var(--cyan)] underline">
                  Maritime methods and satellite datasets
                </a>
                <span className={LABEL}>Feed status is separate from measurement date.</span>
              </div>
              {healthSources.length ? (
                <ul className="grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-3">
                  {healthSources.map(source => (
                    <li key={`${source.region}:${source.id}`} className="flex min-w-0 items-center justify-between gap-3 rounded border border-[rgba(30,58,95,0.55)] px-3 py-2 text-xs">
                      <span className="min-w-0 truncate text-[var(--text-primary)]" title={source.id}>{source.id.replace(/^context:/, '')}</span>
                      <HealthBadge health={source.health} />
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-[var(--text-secondary)]">No context source health records yet.</p>
              )}
            </section>
          </>
        )}
      </div>
    </section>
  );
}

function PriceCard({ label, item }: { label: string; item?: ContextSeries }) {
  return (
    <div className="rounded border border-[rgba(30,58,95,0.55)] px-3 py-3">
      <div className={LABEL}>{label}</div>
      <div className="mt-1 text-xl font-bold tabular-nums text-[var(--text-primary)]">
        {item?.latest?.value == null ? '—' : `${formatNumber(item.latest.value)} USD/bbl`}
      </div>
      <div className="mt-1 text-xs text-[var(--text-secondary)]">
        {item?.latest ? observationLabel(item.latest.date, item.age_days) : 'Observation unavailable'}
      </div>
      {item?.source_url && (
        <a className="mt-2 inline-block text-xs text-[var(--cyan)] underline underline-offset-2" href={item.source_url} target="_blank" rel="noreferrer">
          Source details
        </a>
      )}
    </div>
  );
}

function ShippingCard({ item }: { item: ContextSeries }) {
  const latest = item.latest;
  const isPort = item.kind === 'port';
  const mean = item.mean7;
  const tankerMean = item.tanker_mean7;

  return (
    <article className="rounded border border-[rgba(30,58,95,0.55)] px-3 py-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h4 className="text-sm font-semibold text-[var(--text-primary)]">{item.name}</h4>
        <span className="text-xs text-[var(--text-secondary)]">{isPort ? 'Port calls/day' : 'Vessel transits/day'}</span>
      </div>
      <div className="mt-2 flex flex-wrap items-baseline justify-between gap-2">
        <span className="text-xl font-bold tabular-nums text-[var(--text-primary)]">
          <span className="mr-1 text-xs font-normal text-[var(--text-secondary)]">Total</span>
          {latest?.value == null ? '—' : formatNumber(latest.value)}
          <span className="ml-3 text-sm font-medium text-[var(--text-secondary)]">
            Tanker {latest?.tanker == null ? '—' : formatNumber(latest.tanker)}
          </span>
        </span>
        <span className="text-xs text-[var(--text-secondary)]">
          {latest ? observationLabel(latest.date, item.age_days) : 'Observation unavailable'}
        </span>
      </div>
      <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
        <MeanReadout label="Total · centered 7-day mean" value={mean} />
        <MeanReadout label="Tanker · centered 7-day mean" value={tankerMean} />
      </div>
    </article>
  );
}

function MeanReadout({ label, value }: { label: string; value?: ContextMean | null }) {
  return (
    <div className="rounded bg-[var(--bg-primary)] px-2.5 py-2">
      <div className="text-xs text-[var(--text-secondary)]">{label}</div>
      {value ? (
        <div className="mt-1 text-xs tabular-nums text-[var(--text-primary)]">
          {formatNumber(value.mean)} · centered {formatDate(value.center_date)}
          <div className="mt-1 text-[var(--text-secondary)]">
            {value.window_start} to {value.window_end} · {value.n} days
          </div>
        </div>
      ) : (
        <div className="mt-1 text-xs text-[var(--text-secondary)]">Seven complete days unavailable</div>
      )}
    </div>
  );
}

function CountryValue({ point, netEnergy = false }: { point?: ContextPoint; netEnergy?: boolean }) {
  return (
    <td className="whitespace-nowrap px-2 py-2.5 text-right tabular-nums text-[var(--text-primary)]">
      {point?.value == null ? '—' : (
        <>
          {formatNumber(point.value)}% <span className="text-xs text-[var(--text-secondary)]">({point.date})</span>
          {netEnergy && point.value < 0 && <span className="ml-1 text-xs text-[var(--cyan)]">net exporter</span>}
        </>
      )}
    </td>
  );
}

function HealthBadge({ health }: { health: string }) {
  const good = health === 'ok';
  const color = good ? 'var(--green)' : health === 'stale' || health === 'blocked'
    ? 'var(--amber)' : health === 'unknown' ? 'var(--text-secondary)' : 'var(--red)';
  const background = good ? 'rgba(0,255,136,0.08)' : health === 'stale' || health === 'blocked'
    ? 'rgba(255,170,0,0.1)' : health === 'unknown' ? 'rgba(148,163,184,0.08)' : 'rgba(255,51,102,0.1)';
  return (
    <span className="shrink-0 rounded px-2 py-1 text-xs font-semibold capitalize" style={{ color, backgroundColor: background }}>
      {health || 'unknown'}
    </span>
  );
}

function observationLabel(date: string, ageDays?: number | null) {
  const age = ageDays == null ? '' : ` · ${ageDays === 0 ? 'today' : `${ageDays}d old`}`;
  return `${formatDate(date)}${age}`;
}

function formatDate(value: string) {
  const date = new Date(`${value.slice(0, 10)}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('en', { dateStyle: 'medium', timeZone: 'UTC' }).format(date);
}

function formatNumber(value: number) {
  return new Intl.NumberFormat('en', { maximumFractionDigits: 2 }).format(value);
}
