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

## Open

- Source independence for Google News items should use the outlet (" - Outlet"
  suffix, stored as `payload.outlet`), not the aggregator. Needed by fusion (P2).

## Tested and refused
