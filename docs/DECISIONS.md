# Decisions

Each entry: the decision, the alternatives considered, the evidence, the date.

## Taken

- **Restore the intelligence monitor** (2026-11-06). The owner identified the
  energy-first screen as a regression. `beb08f3` removed the map and sensor panels
  from `page.tsx` and their routes from snapshot publication. Restore the theater
  map as the first panel and the monitor as the default, with alerts, reports,
  aircraft, vessels and satellite detections alongside it. Keep energy/trade and
  disruption analysis as secondary views. Preserve generation validation for
  recorded engine views; upstream-shaped map/sensor responses are validated and
  hashed without claiming a shared engine generation. Pages remains a snapshot;
  local operation polls live feeds. This supersedes the default-view choice below.
  The owner clarified that improvements must accumulate across iterations. Retain
  the generation/lease/recovery work, complete shipping comparisons, provider
  revisions, source provenance, GNSS/outage/MND analysis, report drill-down and
  accessible navigation. The repair adds the missing monitor to that foundation;
  it does not revert the review commit. The publication regression checks both
  the existing recorded evidence families and the restored map/sensor families.

- **Fewer views, deeper evidence** (2026-11-05). Default to energy/trade, with
  disruption signals and supporting reports as separate views. Retain EIA/FRED,
  IMF PortWatch and World Bank WDI because their measures fit those questions;
  an additional price proxy or unvalidated satellite detector adds no defensible
  inference. Complete weekly windows, a rolling four-week reference and release
  revisions expose what changed and what was known. No predictive alert is implied.
- **One dataset commit** (2026-11-05). Immutable files plus a generation manifest
  replace destructive table swaps. OS writer leases, pinned readers and retained
  inputs provide failure isolation and recovery. This supersedes earlier process
  query-lock and bounded physical-file-count claims. Retention grows until a
  reviewed archive policy exists. Pages rebuilds offline with current resources
  and accepts only coherent, recorded, validated snapshots. See
  [integrity and recovery](ENGINE-INTEGRITY.md).

- **Incident linkage across a day** (2026-11-03). In a region with a `[gazetteer]`, two
  news or Telegram reports of one kind are one incident when they name the same oblast,
  their word sets overlap by at least θ = 0.6 (overlap coefficient
  |W ∩ W'| / min(|W|, |W'|), English function words removed), and the merged incident
  spans at most τ_I = 24 h (`engine/src/fuse.rs`, `incidents`). Fusion's τ = 2 h split
  one night's strike into several events as its reports ran on for a day; those
  duplicates were the main error of the unusual indicators (Validation 2026-11-02).
  - θ was chosen on the odd half of the 64 judged flags (0.6: 11 of 72 duplicates
    merged, 0 of 24 real) and held on the even half (29 of 76, 0 of 27).
  - Alternatives considered: one event per place and window (raions, then gazetteer
    points; refused 2026-11-02, it merged real separate strikes); Jaccard similarity
    (merged 4 to 14 of 72 duplicates at 0.3 to 0.4); single linkage without a span
    bound (refused: one event ran 457 h with 4,995 reports, a daily story chaining);
    linking alert and drone-track messages (refused: templated texts are identical).
  - Scoped to gazetteer regions because it was judged only on Russia/Ukraine; the other
    six regions' events are unchanged.
- **Freshness of cron-collected sources** (2026-11-03). A source's last good run is fresh
  within 3 × max(cadence, period of its runner), the cron's period being 12 h
  (`cycle::health`). Before, a source the cron collected an hour earlier was stale
  against its 5-minute local cadence, so every text PIR of every published INTSUM read
  "collection gap", and the backtest's health-as-of mode read stale throughout.
- **Read-only snapshot server** (2026-11-03). `osw serve --no-collect` stores nothing: the
  recording proxy forwards the upstream panels' requests live but neither records them
  nor marks them seen (`cycle::READ_ONLY`). Before, the Pages job's proxy polls wrote
  runs and reports into the store mid-snapshot, so the board read a later state than the
  INTSUM written just before it (RU-2a triggered on the board, no data in the INTSUM).
  The switch is set by the command, not by `api::serve`, because the tests serve the API
  without the scheduler.
- **Warning-sensor indicators** (2026-11-03). An indicator of `air_raid_alert` or
  `airspace_activity` is no data unless its warning sensor (alerts, drone tracks) is
  healthy (`analyze::sensor`). Text sources can report alerts, so the earlier rule (no
  healthy producer of the kind) left RU-1a "inactive, 0 in 3 h" on the published board,
  which only a local engine can collect.
- **Grades of the signal sources** (2026-11-03). gpsjam.org, IODA, Taiwan MND and UCDP
  Candidate: B, usually reliable. IODA is an academic measurement platform with a
  published method; gpsjam.org's published formula was reproduced here cell for cell
  (47,482 of 47,482), on ADS-B data like adsb.lol's B; Taiwan MND is a party's official
  source, graded as the Ukrainian General Staff and the US DoD are; UCDP Candidate is
  human-coded and provisional, revised in the final GED. Only IODA outages become
  observations, so their single-source credibility moves from 6 to 3.
- **Translation failures are retried** (2026-11-03). The free endpoint refused 12 % of
  stored texts, on some days 45 % (Validation), and a refused text stayed untranslated,
  so the classifier and the gazetteer could not read it. `google` retries once and stops
  asking for 10 minutes after 20 failures in a row; `osw retranslate` re-derives the
  reports whose text is not English and rewrites the observation table only if one now
  translates. The Collect workflow runs it weekly, because the rewrite is 16 MB.
  - Alternatives considered: `osw rebuild` on every run (a 16 MB rewrite twice a day);
    another provider (none keyless and official; open).
- **Translations are excerpts too** (2026-11-03). A 280-character original translated to
  up to 533 characters in the store; translations are now cut to `EXCERPT`.
- **Operating-system TLS roots** (2026-11-03). ureq's default roots (WebPKI), handed to
  native-tls as the only trusted roots, failed every request on Windows ("unable to
  find any user-specified roots in the final cert chain"): no local engine on this
  machine could collect. The agent uses native-tls's own roots
  (`RootCerts::PlatformVerifier`).
- **Collect off the hour** (2026-11-03). The schedule moved from 06:00/18:00 to 06:17/18:17
  UTC: GitHub delays scheduled runs most at the start of the hour, and these started up
  to 9 h late.

- **Area INTSUM** (2026-11-02). A region whose events carry `admin1` gets an "Areas"
  table in its INTSUM (events, strikes, fighting, energy targets, killed, injured and
  the indicators that flag each oblast), and each oblast has an INTSUM of its own: the
  same report restricted to the oblast's events and to the indicators counted by
  oblast (`produce::of_area`). The dashboard opens it from the oblasts the header strip
  flags (`src/lib/areas.ts`); the static snapshot writes only those. One report
  function serves both, and no route or table was added.
  - Alternatives considered: a stored product per oblast and cycle (about 30 more
    rows per run for Russia/Ukraine, mostly unread) and a separate drill-down view.
- **Unusual counts stay event counts** (2026-11-02). Counting an oblast's distinct
  raions or gazetteer points per window, instead of events, was tried against the 64
  judged flags (Validation): neither raised precision without losing most of the 13
  supported flags. The indicators keep counting events, at about 20 % precision on the
  history, until incident linking across a day is designed and measured.

- **Unusual activity by oblast, and the Russia/Ukraine PIRs** (2026-11-02). For each
  oblast, the indicator compares the events of one kind (and target class) in the last
  24 hours with the 28 days before. The oblast is unusual if it has at least 3 such
  events and a Poisson count at its trailing rate reaches that many with probability
  below 1 % (`engine/src/analyze.rs`, metric `unusual`).
  - The trailing rate is the oblast's events per report of the kind's producers,
    scaled to the last day's reports. Collection grew from about 10 Russia/Ukraine
    reports a day to 2,000–3,000 in October 2026. With a rate per day, an oblast in
    the steady fighting was unusual on most days: RU-5a was triggered at 52 of 81
    six-hourly steps, against 39 with the scaled rate.
  - Fewer than 7 covered days is no data.
  - Alternatives considered: a z-score (undefined on the many all-zero oblasts) and a
    fixed multiple of the mean (no stated error rate).
  - PIRs:
    - RU-2 is now strikes on energy targets by oblast. RU-2a counts per oblast in 24 h,
      watch 1, trigger 3; RU-2c is the unusual test. The thermal indicator stays as
      RU-2b.
    - RU-5 is new: unusual strikes and explosions (RU-5a) and battles (RU-5b) by
      oblast.
    - These count credibility up to 4, because most single-source text events grade 4
      (82 % of explosions) and grading is out of scope.
  - Backtest over the stored history is in Validation. The history at full volume
    (from 2026-10-20) is shorter than the 28-day window, so the rule's error rate is
    not yet known. Re-measure after 2026-11-17.
- **Incident detail and the statement filter** (2026-11-02). In a region with a
  `[gazetteer]` (Russia/Ukraine), each classified text observation carries the ACLED
  sub-event type, weapon classes (drone, missile, glide bomb, artillery), missile type,
  target class (energy, military, infrastructure, civilian; a site's kind decides
  first), killed and injured counts and the actor pair (`engine/src/process/what.rs`,
  keyword rules, no language model). An event takes its members' largest counts, every
  weapon named and otherwise its first member's value. A report filed as battles,
  explosions or violence against civilians that states no incident becomes `report`,
  which fusion ignores ("Zelensky: 2 ways to halt strikes"). A report that names a
  weapon is kept: without that clause the filter dropped 19 of 175 real incidents on
  one held-out week ("Russia attacks Ukraine with 433 drones"); with it, 4 of 163 on a
  fresh sample, for 28 of 50 statements removed instead of more. Strategic
  developments, alerts and airspace reports are never dropped. Measured per field on
  the labelled samples in `docs/labels/detail-*.jsonl` (Validation); other regions are
  unchanged until each is measured.
- **Sites** (2026-11-02). A town followed by a site word ("Kstovo refinery",
  "Engels airbase"), or preceded by one across "in", "near", "at" or "outside"
  ("refinery in Ryazan"), is a `site` location named "<town> <kind>" in Russia/Ukraine;
  observations and events carry it (`site`). It sits at the nearest OpenStreetMap
  site of that kind within 30 km (100 km for a nuclear plant) from `regions/sites.tsv`,
  or else at the town's point; a site's English name in full also matches. A bare town
  is never a site. A Russian adjective before a site word names its town
  ("Novokuibyshevsky refinery"). On the stored reports 2.7 % of classified observations
  became sites, 90 of 100 judged right (Validation); other regions unchanged. The file
  is built by the Sites workflow from the Overpass API (monthly and on demand), not
  downloaded locally: the repository stays small and every machine gets it by
  `git pull`. Refused: Geofabrik extracts read with pyosmium (341 MB of downloads per
  refresh); GeoNames sites (no refinery or oil depot in UA or RU).
- **Villages, oblasts and raions** (2026-11-02). Russia/Ukraine lists UA and RU under
  `[gazetteer]`, so `osw gazetteer` adds every populated place of both inside the box,
  their ADM1 and ADM2 units, and both countries' cities whole (Omsk, St Petersburg).
  Every place carries its oblast and raion, and so do observations and events
  (`admin1`, `admin2`); sensor points take the units of the nearest place within
  30 km. On the 5,018 classified stored observations of the region, an oblast is now
  known for 62 % (none before) and the unlocated share fell from 32.2 % to 20.7 %
  (Validation). Rules, each from a reviewed miss: a name before a unit word is the
  unit ("Kharkiv region" was placed on the city 351 times); the first place named
  wins among equals, not the longest; a village needs a locative word before it
  ("Yana Peshkova" is a village); a name shared by places resolves only by an oblast
  named, or a city, or a tenfold known population (GeoNames writes 0 for unknown);
  a town followed by a speech verb ("Kyiv says") or "group" is not a place, by
  "direction" or "axis" it is its oblast; a curated `not_places` list masks people
  ("Volodymyr Zelensky": Volodymyr is a city). Refused: all Latin alternate names of
  villages (7.3 MB more for UA alone, names of people); GeoNames sites (no refinery or
  oil depot in UA or RU; OpenStreetMap is next). The products' "near X" now names the
  chosen place: before, it was the first name matched, a country for 421 of 2,557
  located events. Other regions keep their rules; their stored locations are unchanged
  except 21 Red Sea and 4 Black Sea gains (Taiz; Ukrainian alternate names).
- **One Rust engine** (2026-11-01). The Python engine (`engine/*.py`) and the PyO3
  kernel (`kernels/`) are replaced by one crate, `engine/` (library and binary `osw`),
  with the same stages, store layout, API shapes and value formats. The Parquet store is
  read with arrow-rs instead of DuckDB, deduplicated by the same rule (order column,
  then row content). On a copy of the `data` branch (7e2e781e, 138,811 raw reports)
  rebuilt by both at one fixed time, every derived table and every regional view agrees
  (Validation); the rebuild takes 7.3 s against 17.8 s. The Python code and its pytest
  suite are archived outside the repository; the behaviour they guarded is tested in
  `engine/tests`. Dropped with it: the keyword-rule fallback of the classifier and its
  comparison command (the trained model is always present; the 0.68 against 0.54
  result below stays the record).
- **Classifier solved to optimality** (2026-11-01). scikit-learn's `LogisticRegression`
  stops L-BFGS at its default `tol = 1e-4`, short of the optimum: refitted at
  `tol = 1e-8` or `1e-12` it changes the kind of 23 of 99,825 stored observations, and
  agrees there with the Rust solver, which stops at ‖∇‖∞ ≤ 1e-10 (106 iterations,
  0.3 s). The engine's kinds are the optimum's. Near the optimum the loss changes by
  less than its rounding error, so the line search also accepts Hager and Zhang's
  approximate Wolfe conditions; with Armijo alone it stalled at ‖∇‖∞ ≈ 3.5e-7 after
  20,000 iterations (about 300 s).
- **Raw reports read through their adapter** (2026-10-29). A payload's JSON was
  written by its collector and decoded again, by SQL on `raw_report` or `json.loads`,
  in seven other modules (GNSS, signals, context, fusion, the API and both
  processors), and each source's rule for its revised reports (the longest IODA
  listing, the latest UCDP version, the highest MND report number of a day, the latest
  context snapshot) was written where it was read. The adapter now owns both sides:
  `engine.collect.stored` reads and decodes raw reports by adapter, region and date;
  the adapter's reader applies its rule (`mnd.reports`, `ioda.events`, `ucdp.events`,
  `context.releases`, `rss.outlets`); processors receive decoded payloads. Refused:
  moving each processor into its collector module, which would fold processing into
  collection against the cycle's stages. Derived tables and views are unchanged on both
  stores checked (Validation).
- **Days and versions skipped per region** (2026-10-29). gpsjam.org days and UCDP
  versions were skipped once any region stored them, so `engine collect --region R`,
  or a new region, left the other regions without them. A gpsjam.org day is now
  fetched until every region stores it. The UCDP file is read every week (1.6 MB) and
  a region that stores its version takes nothing from it: a region whose box holds no
  event of a version stores nothing that would mark the version read.
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
- Translation uses Google's unofficial endpoint, which refuses requests at times (it
  served this machine its block page on 2026-11-03). No keyless official alternative is
  in use; `osw retranslate` repairs what it refused once it answers.
- `regions/sites.tsv` has not been written: the public Overpass instances timed out or
  refused on 2026-11-02 and 11-03. The script now keeps every answered tile and
  resumes.

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
