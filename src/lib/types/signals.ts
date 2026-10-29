// Open-Source-Watch: shape of the engine's /api/signals (engine/signals.py).

export interface GnssCell {
  h3: string; lat: number; lon: number; near: string | null; good: number; bad: number; pct: number;
  band: 'medium' | 'high'; boundary: [number, number][];
}

export interface GnssDay {
  date: string; source: 'gpsjam.org' | 'adsb.lol'; suspect: boolean;
  cells: number; low: number; medium: number; high: number;
}

export interface MndReport {
  id: string; day: string; published: string; start: string; end: string; text: string;
  aircraft: number | null; ships: number | null; official: number | null;
  balloons: number | null; adiz: number | null; median_line: number | null;
}

export interface Outage {
  country: string; country_name: string; datasource: string; method: string | null;
  start: number; start_utc: string; duration: number; hours: number; score: number | null;
  url: string; fetched_at: string;
}

export interface RecallRow {
  lag_days: number; eligible: number; recalled: number; recall: number | null;
  engine_events: number; engine_in_reach: number;
}

export interface Tally { events: number; deaths_best: number }

export interface SignalSource {
  id: string; region: string; url: string; reliability: string; cadence_s: number;
  health: string; last_status: string | null; last_run: string | null;
}

export interface SignalsResponse {
  region: string;
  gnss: {
    days: GnssDay[];
    map: { date: string | null; source: string; cells: GnssCell[] };
    agreement: { both: number; gpsjam: number; engine: number; neither: number };
  };
  official: {
    reports: MndReport[]; latest: MndReport | null; n_prior: number; unit: string; source: string;
    median: Record<'aircraft' | 'ships' | 'official' | 'balloons' | 'adiz' | 'median_line', number | null>;
  } | null;
  outages: { countries: Record<string, string>; events: Outage[] };
  reference: {
    versions: string[]; events: number; first: string; last: string; eligible_share: number;
    by_month: Record<string, Tally>; by_country: Record<string, Tally>;
    by_type: Record<string, number>;
    recall: { engine_period: [string | null, string | null]; ucdp_period: [string, string]; grid: RecallRow[] };
  } | null;
  sources: SignalSource[];
  updated: string;
}
