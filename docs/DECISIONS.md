# Decisions

Each entry: the decision, the alternatives considered, the evidence, the date.

## Taken

- **Navigation, connectivity and reference signals** (2026-10-29). Four repositories
  were assessed as additions: IRONSIGHT (already upstream; its 10 later commits are
  documentation and a Next patch this fork has), World Monitor (sources and page
  formats only; its instability index and convergence score use editorial weights
  and untested thresholds), God's Eye View (its GNSS-integrity method) and SpiderFoot
  (domain and IP reconnaissance, no overlap; last commit 2023). Taken: gpsjam.org
  daily GNSS cells and the engine's own ADS-B integrity snapshots, IODA outages,
  Taiwan MND daily reports and UCDP Candidate events. Method and limits in
  `SIGNALS.md`.
- **GNSS from published rules** (2026-10-29). Cells use gpsjam.org's formula and
  2/10 % bands. The per-aircraft rule (14 CFR 91.227 minima, via God's Eye View) is
  the only one available and is labelled unvalidated. Alternative refused: God's Eye
  View's 0.5° grid, 30-minute window and three-aircraft minimum, which are its own
  choices; H3 resolution 4 and UTC days match gpsjam.org, so the two can be compared.
  Cells are counted in Rust (h3o): Python has no H3 without a new dependency, and the
  kernel was already required.
- **Integrity snapshots share the aircraft URL** (2026-10-29). The radius query
  adsb.lol already answers carries NIC and NACp, so no request is added. Sources that
  share a URL are now each parsed by their own adapter; before, the group took the
  first source's parser, which was harmless only while one adapter used each URL.
- **Outages as observations** (2026-10-29). IODA events enter fusion as
  `internet_outage`, a new kind, at admin precision at the seat of government, so the
  three measurements of one outage link within τ. Country precision was refused: it
  links nothing, and one outage would count as up to three events.
- **MND as a series, not events** (2026-10-29). The daily report says the same thing
  on quiet and busy days; as `strategic_developments` observations it would add one
  event a day to TW-1a. It is a TW-1c indicator beside its 30-report median instead.
- **UCDP as reference only** (2026-10-29). Fusing UCDP events would let the engine
  match its own input. Matching uses UCDP's own precision classes and the fusion δ of
  25 km; the lag after the event date is reported at 0, 1 and 2 days, not chosen.
- **Adapter fetch** (2026-10-29). Where one URL is not one request (a list page and
  its reports; a dated window; a manifest and its days), an adapter supplies `fetch`.
  Its result is not cached for the recording proxy.

- **Published smoothing, descriptive South Asia counts** (2026-10-24). Shipping
  uses the World Bank notebook's centered seven-day mean, requiring full windows.
  Remove unsourced 7-versus-28-day comparisons and arbitrary observation-age
  cutoffs. South Asia has no validated alert thresholds, so its incident
  indicators display counts without watch/trigger tiers. The supplied satellite
  and iMoonLab repositories are assessed in `MARITIME-METHODS.md`; benchmark
  models are not represented as operational ship observations.

- **Recorded economic context** (2026-10-24). EIA-origin spot benchmarks via
  FRED, IMF PortWatch vessel activity, and World Bank energy/fuel-import
  indicators each retain their dates, units and collection health. All 17
  sources returned successful live polls. These series remain outside the
  classifier and PIR thresholds; tanker counts cannot establish crude cargo.
  See `DATA-CONTEXT.md` and `REVIEW-LOG.md`.
- **Explicit South Asia scope** (2026-10-24). Eight named ISO3 country codes
  replace dependence on a changing external regional aggregate. Nine selected
  ports give coastal context, without claiming complete port coverage.
- **Missing evidence is unassessed** (2026-10-24). A failed producing feed
  makes its indicator `no data`. ACH cannot count a collection gap as absent
  evidence. Stale AIS cannot answer a PIR; insufficient sampled days remains
  a gap even while the stream is healthy.
- **Region-scoped cadence and health** (2026-10-24). Reused RSS/Google source
  names previously allowed another region's poll to suppress a due request.
  Each run is keyed by region and source; health retains three cadences so
  daily baselines do not disappear after 24 hours.
- **Confidence follows indicator windows** (2026-10-24). A six-hour SITREP
  can cite a triggered 72-hour indicator. Supporting credibility now comes
  from that indicator's matching events. AIS confidence remains unassessed
  because the sampling coverage is not calibrated.

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
- **AIS crossing sample gap** (2026-10-23). Gate crossings require adjacent AIS
  positions no more than 15 minutes apart. aisstream is consumed as a live stream and
  reconnects after 30 seconds on connection failures; the 15-minute bound allows for
  several missed position reports while refusing to infer a route across a longer
  absence. A UTC day is sampled when a valid segment has an endpoint on it; this is not
  a full-day uptime claim. These policies are not validated against live coverage.
- **Undated naval positions withheld** (2026-10-23). The dashboard's static ship
  coordinates had no report dates and were stamped with request time. They are removed
  from the live view until a dated position source is available.
- **Registry from TOML, generated at build time** (2026-10-21). The spec's direction
  was an engine-served registry. But upstream's routes read the registry
  synchronously, so serving it would mean an async rewrite of every route. A generated,
  typed `generated.ts` keeps upstream code unchanged and lets `tsc` validate each
  region.
- **Classifier replaces keyword rules** (2026-10-21). On 200 labelled observations,
  blind to the rule label (`docs/labels/`), the measured population-weighted
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
- **Cron twice a day** (2026-10-24, the owner's decision). The `*/15` schedule ran
  26 times in 3.5 days, every 2–5 h, as GitHub delays scheduled jobs; all succeeded.
  It now runs at 06:00 and 18:00 UTC. Items that enter and leave a feed between runs
  (a feed lists its latest 25) are not collected by the cron; a local engine keeps
  the per-source cadences.
- **Seen set from every stored copy, 8 days** (2026-10-24). The set was read from the
  deduplicated view, which keeps an item's first fetch, so an item listed for over
  2 days was stored on every poll: on the `data` branch 43 % of raw reports and 51 %
  of observations were copies (Telegram items about 10 each). The set now holds the
  last time a poll listed each id, read from all stored rows and refreshed by every
  poll; 8 days covers the FIRMS 7-day file. A stateless runner stores an item still
  listed after 8 days again, at most once per 8 days.
- **Deterministic deduplication** (2026-10-24). Copies of an id often share their
  timestamp, and some differ (44 ids by kind, 172 by text, from re-derivation), so the
  view's choice varied between reads (fusion of one store gave 6,895 or 6,896 events).
  Ties are now broken by row content.
- **Compiled linkage kernel** (2026-10-24). The fusion linkage spent about 30 µs per
  observation in Python loop overhead (0.54 s on the cron store, 4.8 s at nine times
  its history). `kernels/` (Rust, PyO3) computes the same labels in 0.06 s and 0.58 s,
  checked against the NumPy code and a brute-force reference. Processing and storage costs
  were reduced in Python instead, where the cost was per call (batch classification,
  4.1 s to 0.6 s for 8,000 reports) or in I/O.

- **Store partitioned by write date** (2026-10-25). Observations were filed under their
  publication date, which spans years: 986 observation and 455 event files on the cron
  store, and DuckDB's scan of them was over 85 % of each request. Files now go under
  the UTC day they are written, one per region per write, and compaction merges past
  days. Same store, same rows (`EXCEPT ALL` both ways): `/api/indicators` 0.85 s to
  0.22 s, `/observations` 0.56 s to 0.14 s, `/events` 0.24 s to 0.08 s,
  `engine fuse` 5.0 s to 2.1 s. The Python in these requests took milliseconds, so a
  Rust rewrite of the engine would not have changed them.
- **One fusion implementation** (2026-10-25). The NumPy linkage fallback is removed;
  fusion requires `kernels/`, collection does not. Tests check the kernel against
  brute force.
- **Upstream scaffolding removed** (2026-10-25). IRONSIGHT's marketing site (`site/`),
  its link checker, promotional images, contribution and conduct guides, issue and PR
  templates, and the duplicated half of its README are gone; attribution, data sources,
  the legal notice and the licence stay. Dependabot version updates are off: each opened
  a branch, and the eight left from 2026-10-17 were stale. Security alerts still arrive.

- **Static snapshot on GitHub Pages** (2026-10-25, the owner's choice over a hosted
  server). After each collection the workflow serves the stored data through the engine
  and dashboard once, writes every route of every region to JSON
  (`scripts/snapshot.mjs`, 133 files, 1.3 MB), and deploys a static export that reads
  them (`.github/workflows/pages.yml`, after each successful collection and each push to
  main). Pages keeps the latest deployment only; nothing is committed. Live-only feeds
  (alerts, aircraft, drones) show what their sources returned at snapshot time.
- **Monthly roll-up** (2026-10-25). Compaction merges each past day, then each past
  month, into one file per region. A row is rewritten at most twice, so the `data`
  branch's history is bounded by three copies of its data, and the file count grows
  by month.
- **Phone layout** (2026-10-25). Below desktop width the panels stack in one scrolling
  column; upstream's 12-column grid left 390 px phones with unreadable slivers.
- **Basemap** (2026-10-25). CARTO's free Dark Matter tiles now return an
  "API key required" image; Esri's World Dark Gray Canvas needs no key and carries
  its attribution on the map.

## Open

- Engine recall against UCDP is not measured: the newest Candidate release (September
  2026) ends before collection began (2026-10-21). The October release should overlap.
- The per-aircraft GNSS rule and the agreement of 10-minute snapshots with gpsjam.org
  are unmeasured until a local engine has run for whole days.
- gpsjam.org, IODA, Taiwan MND and UCDP are graded F pending assessment.

- Thermal indicators II-3 and RU-2b still count events against fixed thresholds after
  the baseline (II-3a 42 in 12 h on 2026-10-21). Whether that reflects real strikes is
  not measured.
- AIS needs `AISSTREAM_KEY`; the message format is taken from aisstream.io's
  documentation, and no live message has been received. Transit PIRs read as
  collection gaps until a key is set and 7 days of coverage exist.
- The 15-minute AIS crossing bound has not been calibrated against live tracks.
- Live-only feeds (alerts, aircraft, drone tracks) have history only while a local
  engine runs.
- A local engine and the cron both write the `data` branch's tables when `./data` is
  its worktree. Raw reports and observations deduplicate by id on read;
  `collection_run` rows do not, but they never collide, since each run writes its own
  rows.

## Tested and refused
