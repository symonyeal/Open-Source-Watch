'use client';

// Open-Source-Watch: GNSS integrity (gpsjam.org, own adsb.lol snapshots), internet
// outages (IODA), Taiwan MND daily reports and UCDP Candidate reference events of
// the selected region, from the engine's /api/signals. Descriptive only.
import { useConflictFeed } from '@/lib/hooks';
import type { GnssDay, MndReport, SignalSource, SignalsResponse } from '@/lib/types/signals';

const SECTION = 'rounded border border-[var(--border-color)] bg-[var(--bg-secondary)] p-4';
const TITLE = 'mb-1 text-sm font-bold tracking-wide text-[var(--text-primary)]';
const NOTE = 'mb-3 text-xs leading-relaxed text-[var(--text-secondary)]';
const TABLE = 'w-full border-collapse text-left text-xs tabular-nums';
const TH = 'px-2 py-1.5 font-medium text-[var(--text-secondary)]';
const TD = 'px-2 py-1.5 text-[var(--text-primary)]';
const ROW = 'border-b border-[rgba(30,58,95,0.4)] last:border-0';
const LINK = 'text-[var(--cyan)] underline underline-offset-2';
const MND_FIELDS = [
  ['aircraft', 'PLA aircraft sorties'], ['adiz', 'Entered ADIZ'], ['median_line', 'Crossed median line'],
  ['ships', 'PLAN ships'], ['official', 'Official ships'], ['balloons', 'Balloons'],
] as const;

export default function SignalsBoard() {
  const { data, loading, error } = useConflictFeed<SignalsResponse>('/api/signals', 300000);

  return (
    <section className="panel flex h-full min-h-0 flex-col" aria-labelledby="signals-title">
      <header className="panel-header">
        <span className="status-dot" style={{ background: 'var(--amber)' }} aria-hidden="true" />
        <h2 id="signals-title" className="text-inherit">NAVIGATION, CONNECTIVITY & REFERENCE SIGNALS</h2>
        <span className="ml-auto text-xs font-normal normal-case tracking-normal text-[var(--text-secondary)]">
          Refreshes every 5 min
        </span>
      </header>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
        {error && !data && (
          <p className="rounded border border-[var(--amber)]/50 px-4 py-3 text-sm text-[var(--amber)]" role="status">
            {error.includes('503') ? 'Signals need the engine, which is not connected for this request.' : 'The signals feed could not be reached.'}
          </p>
        )}
        {!data ? (loading ? <div className="loading-shimmer h-48 rounded" /> : null) : (
          <>
            <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
              <Gnss data={data} />
              <div className="space-y-4">
                {data.official && <Official o={data.official} />}
                <Outages data={data} />
              </div>
            </div>
            <Reference data={data} />
            <Health sources={data.sources} />
          </>
        )}
      </div>
    </section>
  );
}

function Gnss({ data }: { data: SignalsResponse }) {
  const g = data.gnss;
  const days = [...g.days].sort((a, b) => b.date.localeCompare(a.date) || a.source.localeCompare(b.source));
  const top = [...g.map.cells].sort((a, b) => b.pct - a.pct).slice(0, 10);
  const a = g.agreement;
  return (
    <section className={SECTION} aria-labelledby="gnss-title">
      <h3 id="gnss-title" className={TITLE}>GNSS integrity</h3>
      <p className={NOTE}>
        Aircraft per H3 cell and UTC day reporting low navigation accuracy, by{' '}
        <a className={LINK} href="https://gpsjam.org/faq" target="_blank" rel="noreferrer">gpsjam.org&apos;s rule</a>:
        100&nbsp;(bad&nbsp;−&nbsp;1)&nbsp;/&nbsp;all; medium 2–10&nbsp;%, high over 10&nbsp;%. Jamming or spoofing is one
        cause, avionics faults another. adsb.lol rows are this engine&apos;s own 10-minute snapshots (local engine only).
      </p>
      {days.length === 0 ? <p className="text-sm text-[var(--text-secondary)]">No GNSS days stored yet.</p> : (
        <div className="overflow-x-auto">
          <table className={TABLE}>
            <thead><tr className={ROW}>
              <th className={TH}>Day (UTC)</th><th className={TH}>Source</th>
              <th className={`${TH} text-right`}>Cells</th>
              <th className={`${TH} text-right`}><Swatch c="#ffaa00" />Medium</th>
              <th className={`${TH} text-right`}><Swatch c="#ff3366" />High</th>
            </tr></thead>
            <tbody>{days.map((d: GnssDay) => (
              <tr key={d.date + d.source} className={ROW}>
                <td className={`${TD} whitespace-nowrap`}>{d.date}{d.suspect && <span className="ml-1 text-[var(--amber)]" title="gpsjam.org marks this day incomplete">incomplete</span>}</td>
                <td className={TD}>{d.source}</td>
                <td className={`${TD} text-right`}>{d.cells}</td>
                <td className={`${TD} text-right`}>{d.medium}</td>
                <td className={`${TD} text-right`}>{d.high}</td>
              </tr>
            ))}</tbody>
          </table>
        </div>
      )}
      {top.length > 0 && (
        <>
          <h4 className="mt-4 mb-1 text-xs font-semibold text-[var(--text-primary)]">
            Highest cells, {g.map.date} (shown on the map as GNSS)
          </h4>
          <table className={TABLE}>
            <thead><tr className={ROW}>
              <th className={TH}>Nearest city</th><th className={TH}>Centre</th><th className={`${TH} text-right`}>Aircraft</th>
              <th className={`${TH} text-right`}>Low accuracy</th><th className={`${TH} text-right`}>Share</th>
            </tr></thead>
            <tbody>{top.map(c => (
              <tr key={c.h3} className={ROW}>
                <td className={TD}>{c.near ?? '—'}</td>
                <td className={`${TD} whitespace-nowrap`}>{c.lat.toFixed(2)}, {c.lon.toFixed(2)}</td>
                <td className={`${TD} text-right`}>{c.good + c.bad}</td>
                <td className={`${TD} text-right`}>{c.bad}</td>
                <td className={`${TD} text-right`}><Swatch c={c.band === 'high' ? '#ff3366' : '#ffaa00'} />{c.pct}%</td>
              </tr>
            ))}</tbody>
          </table>
        </>
      )}
      {a.both + a.gpsjam + a.engine + a.neither > 0 && (
        <p className="mt-3 text-xs text-[var(--text-secondary)]">
          Cell-days seen by both sources: {a.both} anomaly in both, {a.gpsjam} in gpsjam.org only,
          {' '}{a.engine} in the engine only, {a.neither} in neither. Agreement, not accuracy.
        </p>
      )}
    </section>
  );
}

function Official({ o }: { o: NonNullable<SignalsResponse['official']> }) {
  const latest = o.latest;
  const recent = [...o.reports].reverse().slice(0, 14);
  return (
    <section className={SECTION} aria-labelledby="mnd-title">
      <h3 id="mnd-title" className={TITLE}>PLA activity reported by Taiwan&apos;s Ministry of National Defense</h3>
      <p className={NOTE}>
        Daily counts {o.unit}, as the ministry states them: a party&apos;s own claims, not independent observation.
        Median over the {o.n_prior} reports before the latest.
      </p>
      {!latest ? <p className="text-sm text-[var(--text-secondary)]">No reports stored yet.</p> : (
        <>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {MND_FIELDS.map(([k, label]) => (
              <div key={k} className="rounded border border-[rgba(30,58,95,0.55)] px-3 py-2">
                <div className="text-xs text-[var(--text-secondary)]">{label}</div>
                <div className="text-lg font-bold tabular-nums text-[var(--text-primary)]">{latest[k] ?? '—'}</div>
                <div className="text-[10px] text-[var(--text-secondary)]">median {o.median[k] ?? '—'}</div>
              </div>
            ))}
          </div>
          <p className="mt-2 text-xs text-[var(--text-secondary)]">
            Report of {latest.day} (window {latest.start.slice(0, 16).replace('T', ' ')} to {latest.end.slice(0, 16).replace('T', ' ')} UTC).{' '}
            <a className={LINK} href={`https://www.mnd.gov.tw/en/News/PLAAct/${latest.id}`} target="_blank" rel="noreferrer">Source</a>
          </p>
          <Sorties reports={o.reports} />
          <div className="mt-3 max-h-64 overflow-y-auto">
            <table className={TABLE}>
              <thead><tr className={ROW}>
                <th className={TH}>Day</th>
                {MND_FIELDS.map(([k, label]) => <th key={k} className={`${TH} text-right`}>{label}</th>)}
              </tr></thead>
              <tbody>{recent.map((p: MndReport) => (
                <tr key={p.id} className={ROW}>
                  <td className={`${TD} whitespace-nowrap`}><a className={LINK} href={`https://www.mnd.gov.tw/en/News/PLAAct/${p.id}`} target="_blank" rel="noreferrer">{p.day}</a></td>
                  {MND_FIELDS.map(([k]) => <td key={k} className={`${TD} text-right`}>{p[k] ?? '—'}</td>)}
                </tr>
              ))}</tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}

// One series, one hue: aircraft sorties per report, each bar's value in its tooltip.
function Sorties({ reports }: { reports: MndReport[] }) {
  const R = reports.filter(p => p.aircraft !== null);
  if (R.length < 2) return null;
  const max = Math.max(...R.map(p => p.aircraft as number), 1);
  const W = 600, H = 90, w = W / R.length;
  return (
    <figure className="mt-3">
      <figcaption className="mb-1 text-xs text-[var(--text-secondary)]">
        PLA aircraft sorties per report, {R[0].day} to {R[R.length - 1].day}; maximum {max}
      </figcaption>
      <svg viewBox={`0 0 ${W} ${H + 1}`} className="h-24 w-full" preserveAspectRatio="none" role="img"
        aria-label={`Daily PLA aircraft sorties, maximum ${max}`}>
        <line x1={0} x2={W} y1={H + 0.5} y2={H + 0.5} stroke="var(--border-color)" />
        {R.map((p, k) => {
          const h = Math.max(1, (H * (p.aircraft as number)) / max);
          return (
            <rect key={p.id} x={k * w + 1} y={H - h} width={Math.max(1, w - 2)} height={h} rx={1}
              fill="var(--cyan)" opacity={0.85}>
              <title>{`${p.day}: ${p.aircraft} sorties`}</title>
            </rect>
          );
        })}
      </svg>
    </figure>
  );
}

function Outages({ data }: { data: SignalsResponse }) {
  const { countries, events } = data.outages;
  return (
    <section className={SECTION} aria-labelledby="outage-title">
      <h3 id="outage-title" className={TITLE}>Internet outages</h3>
      <p className={NOTE}>
        Outages detected by <a className={LINK} href="https://ioda.inetintel.cc.gatech.edu/" target="_blank" rel="noreferrer">IODA</a>{' '}
        (Georgia Tech) in the last 7 days, per measurement: BGP routing, active probing (ping-slash24), network telescope
        (merit-nt). Countries: {Object.values(countries).join(', ') || 'none configured'}.
      </p>
      {events.length === 0 ? <p className="text-sm text-[var(--text-secondary)]">None detected in the last 7 days.</p> : (
        <table className={TABLE}>
          <thead><tr className={ROW}>
            <th className={TH}>Country</th><th className={TH}>Signal</th><th className={TH}>Start (UTC)</th>
            <th className={`${TH} text-right`}>Hours listed</th><th className={`${TH} text-right`}>Score</th>
          </tr></thead>
          <tbody>{events.map(e => (
            <tr key={`${e.country}:${e.datasource}:${e.start}`} className={ROW}>
              <td className={TD}>{e.country_name}</td>
              <td className={TD}><a className={LINK} href={e.url} target="_blank" rel="noreferrer">{e.datasource}</a></td>
              <td className={`${TD} whitespace-nowrap`}>{e.start_utc.slice(0, 16).replace('T', ' ')}</td>
              <td className={`${TD} text-right`}>{e.hours}</td>
              <td className={`${TD} text-right`}>{e.score === null ? '—' : Math.round(e.score)}</td>
            </tr>
          ))}</tbody>
        </table>
      )}
    </section>
  );
}

function Reference({ data }: { data: SignalsResponse }) {
  const ref = data.reference;
  return (
    <section className={SECTION} aria-labelledby="ucdp-title">
      <h3 id="ucdp-title" className={TITLE}>Reference events: UCDP Candidate</h3>
      <p className={NOTE}>
        Organized violence with at least one death, coded by the{' '}
        <a className={LINK} href="https://ucdp.uu.se/downloads/" target="_blank" rel="noreferrer">Uppsala Conflict Data Program</a>{' '}
        in monthly preliminary releases, for events inside this region&apos;s map box. Used to measure how many the engine
        finds, never as engine reports.
      </p>
      {!ref ? <p className="text-sm text-[var(--text-secondary)]">No UCDP events stored for this region.</p> : (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
          <div>
            <p className="text-xs text-[var(--text-secondary)]">
              Release {ref.versions.join(', ')} · {ref.events} events, {ref.first} to {ref.last} ·
              {' '}{Math.round(100 * ref.eligible_share)}% with an exact date and a location within 25 km
            </p>
            <table className={`${TABLE} mt-2`}>
              <thead><tr className={ROW}><th className={TH}>Type</th><th className={`${TH} text-right`}>Events</th></tr></thead>
              <tbody>{Object.entries(ref.by_type).map(([k, n]) => (
                <tr key={k} className={ROW}><td className={TD}>{k}</td><td className={`${TD} text-right`}>{n}</td></tr>
              ))}</tbody>
            </table>
          </div>
          <div className="max-h-56 overflow-y-auto">
            <table className={TABLE}>
              <thead><tr className={ROW}>
                <th className={TH}>Country</th><th className={`${TH} text-right`}>Events</th><th className={`${TH} text-right`}>Deaths (best)</th>
              </tr></thead>
              <tbody>{Object.entries(ref.by_country).sort((a, b) => b[1].events - a[1].events).map(([k, v]) => (
                <tr key={k} className={ROW}>
                  <td className={TD}>{k}</td><td className={`${TD} text-right`}>{v.events}</td>
                  <td className={`${TD} text-right`}>{v.deaths_best}</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
          <Recall r={ref.recall} />
        </div>
      )}
    </section>
  );
}

function Recall({ r }: { r: NonNullable<SignalsResponse['reference']>['recall'] }) {
  const scored = r.grid.some(x => x.eligible > 0);
  return (
    <div>
      <h4 className="mb-1 text-xs font-semibold text-[var(--text-primary)]">Engine recall of UCDP events</h4>
      {!scored ? (
        <p className="text-xs leading-relaxed text-[var(--text-secondary)]">
          Not measured: no UCDP event with an exact date and location falls inside the engine&apos;s collection period
          ({(r.engine_period[0] ?? '—').slice(0, 10)} to {(r.engine_period[1] ?? '—').slice(0, 10)}; UCDP release covers
          {' '}{r.ucdp_period[0]} to {r.ucdp_period[1]}). The next monthly release will overlap.
        </p>
      ) : (
        <table className={TABLE}>
          <thead><tr className={ROW}>
            <th className={TH}>Lag</th><th className={`${TH} text-right`}>Eligible</th>
            <th className={`${TH} text-right`}>Found</th><th className={`${TH} text-right`}>Recall</th>
          </tr></thead>
          <tbody>{r.grid.map(x => (
            <tr key={x.lag_days} className={ROW}>
              <td className={TD}>{x.lag_days} d</td><td className={`${TD} text-right`}>{x.eligible}</td>
              <td className={`${TD} text-right`}>{x.recalled}</td>
              <td className={`${TD} text-right`}>{x.recall === null ? '—' : `${Math.round(100 * x.recall)}%`}</td>
            </tr>
          ))}</tbody>
        </table>
      )}
    </div>
  );
}

function Health({ sources }: { sources: SignalSource[] }) {
  return (
    <section className={SECTION} aria-labelledby="signals-health-title">
      <h3 id="signals-health-title" className={TITLE}>Collection health</h3>
      <ul className="mt-2 grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-4">
        {sources.map(s => (
          <li key={`${s.region}:${s.id}`} className="flex min-w-0 items-center justify-between gap-2 rounded border border-[rgba(30,58,95,0.55)] px-3 py-1.5 text-xs">
            <span className="min-w-0 truncate text-[var(--text-primary)]" title={`${s.id}, grade ${s.reliability}`}>{s.id.split('#')[0]}{s.id.includes('#') ? ` ${s.id.split('#')[1]}` : ''}</span>
            <span className={s.health === 'ok' ? 'text-[var(--green)]' : s.health === 'unknown' ? 'text-[var(--text-secondary)]' : 'text-[var(--amber)]'}>{s.health}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Swatch({ c }: { c: string }) {
  return <span className="mr-1 inline-block h-2 w-2 rounded-sm align-middle" style={{ background: c }} aria-hidden="true" />;
}
