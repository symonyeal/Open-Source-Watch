# Decisions

Each entry: the decision, the alternatives considered, the evidence, the date.

## Taken

- **Fork IRONSIGHT; Python engine beside it** (2026-10-20). Alternatives: a Python core
  with a new static front end; a Next.js front end with a separate Python service.
  The user chose to fork, then build the engine. Upstream stays at the root so
  `git merge upstream/main` works.
- **Intelligence-cycle structure, Admiralty grading, ACLED taxonomy** (2026-10-20). The
  user asked for standard intelligence-tool design. See the design document.
- **No language models** (2026-10-20, user). Processing is rule-based; translation uses
  the same free Google endpoint as upstream.
- **Feeds exported, not mirrored** (2026-10-21). The spec first had hand-written TOML
  feed lists tested for agreement with the TypeScript configs. Instead
  `scripts/export-regions.mjs` writes `regions/<id>/conflict.json`, and a test fails
  when it is stale. This leaves one copy of each list.
- **`report` kind** (2026-10-21). 318 of 465 observations in the first live collection
  matched no kind rule. Labelling them with an ACLED type would be false, so they are
  `report`, which fusion ignores.
- **Engine port 8790** (2026-10-21). 8765 was already bound by two processes on this
  machine.
- **pytz dependency** (2026-10-21). DuckDB needs it to return `TIMESTAMPTZ` values to
  Python; without it every timestamp query raised `InvalidInputException`.
- **Recording proxy as the seam** (2026-10-21). The P0 seam forwarded whole routes to
  engine views with upstream's JSON shape. That would have needed Python copies of
  upstream's shaping tables: aircraft classification, the Hebrew alert vocabulary,
  strike categories. Instead `fetchWithTimeout` asks the engine for each upstream URL;
  the engine serves the response it stored, and upstream's code shapes it. Evidence:
  with `ENGINE_URL` set, all 23 routes answered; 106 of 106 proxied requests were
  served by the engine (103 with 200; 3 with 502 for feeds that fail at their origin);
  0 unscheduled URLs.
- **Telegram channel previews** (2026-10-21). `t.me/s/<channel>` lists the latest posts
  in one request, against about 20 embed probes per channel per poll upstream. 70 of 73
  channels serve it; Alertisrael, TasnimNewsEN and aljazeeraglobal do not, and record
  `empty`. Consequence: the engine-served Telegram panel shows the stored 280-character
  excerpt, not the full post.
- **ADS-B storage subset** (2026-10-21). Stored: military-feed aircraft inside a
  region's box, and radius-feed aircraft flagged military or interesting (dbFlags 1|2).
  Upstream also displays aircraft that it classifies as military by callsign or type
  alone; those are shown but not stored, since storing the whole radius feed means
  every civil aircraft within 2,500 km every 30 s.
- **FIRMS low confidence** (2026-10-21). VIIRS detections with confidence `l` are stored
  as raw reports but are not observations of a thermal anomaly.
- **GeoNames gazetteer** (2026-10-21). `regions/geonames.tsv`: 4,289 names of
  cities15000 entries inside some region's box. Matching is by 1–3-word phrase
  lookup. GeoNames names must be capitalised, and names under 5 letters need a
  population of at least 500,000, because title-case headlines ("Oil Zone Hit by
  Fire") otherwise geocode to Hit, Iraq. The curated `strikeLocations` table still
  takes precedence and matches case-insensitively.
- **Compaction** (2026-10-21). The scheduler writes one file per table per tick. Every
  hour it merges each past day's directory into one file, under a lock that queries
  share.

- **Google News independence by outlet** (2026-10-21). A Google News item's owner is
  the outlet in its " - Outlet" suffix (`payload.outlet`), not the aggregator.
- **Country-level observations do not link** (2026-10-21). In the first live fusion,
  1,036 events in total, observations located only to "Ukraine" merged unrelated
  reports, because they share one coordinate. A country is not an administrative unit
  for fusion, so these observations now form single-observation events.
- **Contradiction deferred to P3** (2026-10-21). The narrowed rule (no A/B siren within
  30 minutes) fired once on live data, and falsely: reports published at 06:07 and
  07:21 UTC described an alert at 02:20 UTC. Text observations carry publication time,
  not event time. Credibility 5 stays defined and tested; no rule assigns it.
- **Events and entities are recomputed, not appended** (2026-10-21). `fuse` replaces
  both tables atomically from the store every 10 minutes (2.0 s on 4,589
  observations). The cron does not commit them, since a recomputed table would churn
  the `data` branch; `engine fuse` rebuilds them locally.
- **Indicator status on demand** (2026-10-21). `/api/indicators` evaluates each
  indicator over the event table when asked. Status history is the P3 indicator board.

- **Raw report ids include the region** (2026-10-21). The id was a hash of source and
  item only, so a source shared by two regions (for example `rss:Al Jazeera`, same URL
  in both upstream theaters) gave one id, and deduplication kept one region's copy.
  Ids written before this change (P0 to P2, and the first `data` branch commits) use
  the old hash; the `data` branch was restarted.
- **Thermal baseline** (2026-10-21). A detection is a thermal anomaly only if its 0.1°
  cell was active on fewer than 3 of the 7 preceding days. In the first live run with
  the 7-day file, 1,206 of 1,435 Iran/Israel detections in 24 h were at persistent
  sources, and II-3a fell from 228 to 42 events. The 7-day file is processed before
  the 24-hour file, since processing them in parallel judged recent detections with no
  history.
- **Registry from TOML, generated at build time** (2026-10-21). The spec's direction
  was an engine-served registry. But upstream's routes read the registry
  synchronously, so serving it would mean an async rewrite of every route. A generated,
  typed `generated.ts` keeps upstream code unchanged and lets `tsc` validate each
  region.
- **Classifier replaces keyword rules** (2026-10-21). On 200 observations labelled by
  the agent, blind to the rule label (`docs/labels/`), the measured population-weighted
  accuracy was rules 0.543 against TF-IDF + logistic regression 0.68 (5 seeds,
  0.67–0.69) under 3-fold cross-validation; event-vs-report precision was 0.63 against
  0.79. C = 4 was chosen from {1, 4, 16} on the same folds, a mild selection bias. Rare
  kinds remain poor for both methods (macro-F1 about 0.4). The labels are one labeller's
  judgement.
- **Contradiction restored with stated event times** (2026-10-21). The P2 false
  positive (Kyiv reports describing a 02:20 UTC alert) is now a regression test: no
  warning-source runs covered 02:20, so there is no contradiction.
- **Collection gap by producing adapters** (2026-10-21). The P2 rule called a kind
  that had not occurred a gap (Taiwan, no maritime incidents). A gap is now: no healthy
  source of an adapter able to produce the kind.
- **ACH sees every indicator of the region** (2026-10-21). Hypotheses cite evidence
  across PIRs (RS-2 cites RS-1a).
- **Cron polls only due sources** (2026-10-21). `collect --due` skips sources whose
  last successful run is younger than 90 % of their cadence; otherwise the 34 MB FIRMS
  7-day file would be fetched every 15 minutes.

## Open

- Thermal indicators II-3 and RU-2b still count events against fixed thresholds after
  the baseline (II-3a 42 in 12 h on 2026-10-21). Whether that reflects real strikes is
  not measured.
- AIS needs `AISSTREAM_KEY`; the message format is taken from aisstream.io's
  documentation, and no live message has been received. Transit PIRs read as
  collection gaps until a key is set and 7 days of coverage exist.
- Live-only feeds (alerts, aircraft, drone tracks) have history only while a local
  engine runs.
- A local engine and the cron both write the `data` branch's tables when `./data` is
  its worktree. Raw reports and observations deduplicate by id on read;
  `collection_run` rows do not, but they never collide, since each run writes its own
  rows.

## Tested and refused
