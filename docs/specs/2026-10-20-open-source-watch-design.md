# Open-Source-Watch: design

Date: 2026-10-20. Status: approved 2026-10-20; amended 2026-10-21 (see docs/DECISIONS.md).

## Purpose

Open-Source-Watch is an open-source intelligence platform. Version one is a live,
multi-region monitor. Every observation it shows is stored, graded and traceable to its
source, so the same store supports analysis later. Coverage begins with IRONSIGHT's two
theaters, Middle East and Eastern Europe. It then grows chokepoint by chokepoint until
it covers sections of the world.

The platform follows the intelligence cycle: direction, collection, processing,
exploitation and fusion, analysis, production, dissemination, feedback. It uses
standard grading conventions: Admiralty reliability and credibility grades, and ICD 203
estimative language.

## Decisions

| decision | choice |
|---|---|
| purpose | live monitor first; storage designed in for later analysis |
| base | GitHub fork of `NoblerWorks-HQ/IRONSIGHT` (MIT), renamed `symonyeal/Open-Source-Watch` |
| front end | IRONSIGHT's Next.js app in its upstream layout at the repo root, minimally edited |
| back end | system Python 3.14 for everything outside the browser; Node only builds and serves the front end |
| language models | none; processing is rule-based and statistical |
| runtime | local engine for live feeds; GitHub Actions cron for slow feeds, committing to a `data` branch |
| expansion | by chokepoint: Red Sea/Bab el-Mandeb, Hormuz, Taiwan Strait/South China Sea, Black Sea |
| store | Parquet partitioned by `region/date`, queried with DuckDB |

## Repository layout

```
src/ public/ tests/ ...    IRONSIGHT, upstream layout
engine/
  collect/                 one adapter per source
  process/                 translation, geolocation, classification, deduplication
  fuse/                    events, entities, tracks
  analyze/                 indicators, models, ACH (P3)
  produce/                 INTSUM, SITREP, flash alerts (P3)
  store.py                 Parquet writer, DuckDB reader
  api.py                   Starlette HTTP and SSE server
  cli.py                   collect, rebuild, serve
regions/<id>.toml          region registry: geography, sources, PIRs, indicators
.github/workflows/         CI gate; collection cron
docs/                      house layout; specs
```

IRONSIGHT's files stay in place, so `git merge upstream/main` keeps working.

`upstream` is a second remote that points at IRONSIGHT.

## Intelligence cycle to modules

| stage | module | output |
|---|---|---|
| planning and direction | `regions/*.toml` | PIRs → indicators → collection tasks (sources, cadence) |
| collection | `engine/collect` | immutable raw reports with provenance |
| processing | `engine/process` | observations: English text, location, time, kind |
| exploitation and fusion | `engine/fuse` | events, entities, tracks |
| analysis | `engine/analyze` | indicator status, models, assessments |
| production | `engine/produce` | INTSUM, SITREP, flash alerts |
| dissemination | front end and API | common operational picture (COP), panels, alerts |
| feedback | `pir` table | answered or gap → re-tasking |

## Seam between front end and engine

(Amended 2026-10-21.) The seam is a **recording proxy**. Upstream routes fetch through
one helper, `fetchWithTimeout` in `src/lib/fetcher.ts`.
- If `ENGINE_URL` is set, that helper requests `GET /fetch?url=<upstream URL>` from the
  engine. The engine polls every upstream URL on its cadence, stores what it parses,
  and answers with the latest response.
- The route then runs upstream's own shaping code on it, so the dashboard shows exactly
  what the engine stored, and no upstream logic is copied into Python.
- If `ENGINE_URL` is unset, or the engine answers 502 or not at all, the helper fetches
  directly, and the fork runs as plain IRONSIGHT.

Telegram is the exception: upstream probes embed pages one post at a time, which the
engine replaces with the channel preview page. Its route therefore returns the
engine's own view through `fromEngine` (`src/lib/engine.ts`), as does the new
`/api/sources`.

Engine API:

- `GET /fetch?url=`: the recording proxy; 502 for a URL on no scheduled host
- `GET /api/telegram`: upstream's `TelegramData` from the store
- `GET /regions`: region registry
- `GET /events?region=&since=&kind=`
- `GET /observations?region=&since=&source=`
- `GET /tracks?region=&since=&type=`
- `GET /sources?region=`: source health from `collection_run`
- `GET /stream?region=`: SSE stream of new observations, events and alerts

Region source of truth:
- **P0–P2:** `src/lib/conflicts/*.ts` is the single source of feeds, channels and
  keywords. `node scripts/export-regions.mjs` writes it to `regions/<id>/conflict.json`,
  and a test fails when that export is stale. `regions/<id>/region.toml` holds only what
  the engine owns: bounding box, grades, owners, PIRs and indicators. (Amended
  2026-10-21: the export replaces hand-copied lists checked for agreement.)
- **P3 onward:** the conflict registry loads from `GET /regions`, and a new region is
  one TOML file.

## Data model

Every row links to its parent, so any claim on the map traces back to raw reports.

```
Source ─< RawReport ─< Observation >─ Event >─< Entity
                                        │
                     Indicator >─ Assessment >─ Product
```

| table | fields |
|---|---|
| `source` | id, name, discipline, owner, url, reliability ∈ {A..F}, cadence_s, regions, lang |
| `collection_run` | source_id, started_at, status ∈ {ok, empty, http_error, parse_error, blocked}, n_rows, latency_ms, runner ∈ {local, cron} |
| `raw_report` | id = sha256(source_id, native_id), source_id, fetched_at, published_at, url, payload |
| `observation` | id, raw_id, t, lat, lon, geo_precision ∈ {point, city, admin, country, none}, region, kind, text_en, lang, credibility ∈ {1..6}, names |
| `event` | id, t_start, t_end, lat, lon, region, kind, observation_ids, n_independent, credibility |
| `entity` | id, type ∈ {actor, unit, vessel, aircraft, site}, names, identifiers (ICAO hex, MMSI, IMO), attributes |
| `track_point` | entity_id, t, lat, lon, heading, speed, source_id |
| `pir` | id, region, question, status ∈ {open, answered, gap}, updated_at |
| `indicator` | id, pir_id, region, rule, status ∈ {inactive, watch, triggered}, updated_at |
| `assessment`, `product` | defined in P3 |

`payload` stores the metadata and an excerpt of at most 280 characters for news and
Telegram, and the full record for sensor, track, alert and market feeds. The repository
is public, so full article text is never stored.

Disciplines are source tags:
- `osint`: news, Telegram
- `geoint`: FIRMS
- `tracking`: ADS-B, AIS, drone tracks
- `warning`: siren and oblast alerts
- `market`: prices, Polymarket

### Event taxonomy

The taxonomy is ACLED's event types and sub-event types:
- battles
- explosions/remote violence
- violence against civilians
- protests
- riots
- strategic developments

It is extended with four kinds: `air_raid_alert`, `maritime_incident`,
`airspace_activity` and `thermal_anomaly`. A processed item that no rule classifies has
kind `report`, and fusion ignores it (amended 2026-10-21).

### Credibility

Credibility is computed in `fuse` from corroboration. Adapters never assign it. Let n
be the number of independent sources graded A or B that support an event. Sources are
independent when their `owner` fields differ.

| condition, first match wins | credibility |
|---|---|
| contradicted by an A/B source | 5 improbable |
| n ≥ 3 | 1 confirmed |
| n = 2 | 2 probably true |
| n = 1 | 3 possibly true |
| supported only by sources graded C, D or E | 4 doubtful |
| otherwise (F or ungraded only) | 6 cannot be judged |

An unfused observation carries the credibility of a one-observation event.

**"Contradicted":** an A/B warning or sensor source covering the same place and window
reports no activity of that kind. No rule assigns it before P3 (see § Fusion,
amendment).

### Fusion

Two observations are linked when all three conditions hold:
- they have the same `kind`;
- |Δt| ≤ 2 h;
- they are within 25 km, or in the same administrative unit when either has
  `geo_precision` coarser than `city`.

Events are the connected components of this graph (single linkage), so the result
doesn't depend on input order. Thresholds live in config.

(Amended 2026-10-21.) A country-level observation links to nothing and forms a
single-observation event. An unlocated observation forms no event. In the first live
fusion, treating every observation located only to "Ukraine" as one administrative
unit merged unrelated reports.

**Contradiction is deferred to P3 (amended 2026-10-21).** Text observations carry
their publication time, not the time of what they describe. In the first live fusion,
the one narrowed rule tried (no A/B siren within 30 minutes) fired once, and falsely:
Kyiv reports published at 06:07 and 07:21 UTC described an alert at 02:20 UTC. The
absence of a VIIRS detection is not evidence either, because the satellite passes about
twice a day. Credibility 5 stays defined; no rule assigns it until P3 extracts event
times.

## Processing (no language models)

- **Translation:** Google Translate's unofficial endpoint, as in IRONSIGHT. It sits
  behind a `translate(text, lang) -> str` interface so that it can be replaced.
- **Geolocation:** match against the GeoNames gazetteer, restricted to the region's
  bounding box. The precision is set by the matched feature class.
- **Classification:** keyword rules per kind and language in the region TOML. From P3, a
  scikit-learn classifier trained on stored, labelled history.
- **Deduplication:** exact match on `raw_report.id`, plus a near-duplicate text hash
  within a source.

## Sources for P1

These are IRONSIGHT's sources, as adapters. Grades are defaults that can be overridden
per region TOML.

| source | discipline | cadence | runner | default grade |
|---|---|---|---|---|
| Google News RSS, outlet RSS | osint | 5 min | local, cron | wire B; national C; belligerent state media D |
| Telegram public channels (embed scrape) | osint | 2 min | local; cron best-effort | official A–B; aggregators E |
| NASA FIRMS | geoint | 10 min | local, cron | A |
| adsb.lol | tracking | 30 s | local | B |
| Tzeva Adom, alerts.com.ua | warning | 15 s | local | A |
| neptun.in.ua | tracking | 30 s | local | C |
| Yahoo Finance, CoinGecko, Polymarket | market | 10 min | local, cron | B |

The `local` runner collects only while the engine runs on the laptop. Gaps in its
history are recorded in `collection_run`, never read as inactivity.

## Errors

- Each poll writes one `collection_run` row, and an adapter failure is confined to its
  own source.
- **Source health:** a source is ok if its last run succeeded, stale if its last success
  is older than 3 × cadence, and failed if its last three runs failed.
- A PIR whose every source is stale or failed is shown as a **collection gap**.
- **Idempotence:** raw report ids are content hashes, and processing and fusion are pure
  functions of the raw layer. `engine rebuild` regenerates every derived table, so the
  raw layer is the only irreplaceable state.

## Testing and gate

- **Adapters:** recorded-fixture tests (a saved response → expected raw reports). No
  network access in tests.
- **Fusion:** property tests. The result is invariant to input permutation, and the
  output is the connected components of the link graph.
- **Credibility:** property tests. Credibility is monotone in n, and an A/B
  contradiction always yields 5.
- **Config:** every `regions/*.toml` validates against the schema. In P0–P2 each one
  agrees with its TypeScript conflict config.
- **Front end:** upstream tests and lint are kept unchanged.
- **Gate:** `py -3.14 -m pytest`, `py -3.14 -m ruff check .`, `npm run lint` and
  `npm run build`, run locally and in CI.

## Phases

| phase | scope | done when |
|---|---|---|
| P0 | fork, clone, rename; house layout; `engine/` skeleton; full schema; region TOMLs for both theaters; Google News adapter → observations → store; news route proxied | with `ENGINE_URL` set, the news panel shows engine-served items that persist across restarts; gate green |
| P1 | remaining P1 sources as adapters; translation, geolocation, classification; every route proxied; source-health panel | each panel matches upstream behaviour with `ENGINE_URL` set; every adapter has a fixture test |
| P2 | collection cron onto `data`; fusion into events and entities; credibility; PIRs and indicators for both theaters | the cron runs at least 24 h without manual repair; events carry credibility; the indicator panel shows live status |
| P3 (direction) | registry served by the engine; indicator board; INTSUM and SITREP templates; ACH worksheet; classifier | set by its own spec |
| P4 (direction) | AIS adapter (aisstream.io, free key in `.env` and an Actions secret); chokepoint regions; transit and thermal anomaly models; Supply-Chain-Cuts link | set by its own spec |

## Out of scope

- Language-model processing.
- Paid or keyed sources before P4.
- User accounts.
- Hosting the live engine.
- Storing full article text.

## Glossary

| term | meaning |
|---|---|
| PIR | priority intelligence requirement: a question the platform must answer for a region |
| indicator | an observable whose status bears on a PIR |
| raw report | one unmodified item as fetched from a source |
| observation | a processed raw report: English text, location, time, kind |
| event | a cluster of observations judged to describe one occurrence |
| reliability | Admiralty grade A–F of a source |
| credibility | Admiralty grade 1–6 of an observation or event, derived from corroboration |
| COP | common operational picture: the map and panels |
| INTSUM | intelligence summary: a periodic written product per region |
| collection gap | a PIR whose sources are all stale or failed |

## Attribution

The README credits IRONSIGHT (MIT, Nobler Works) and every data provider, and keeps
upstream's research-use disclaimer.

GitHub topics: osint, intelligence, geospatial, conflict-monitoring, python, nextjs.
