# Validation

What must be checked, how, and what has been. Checks are invariants and recorded real
responses, never snapshots of live output.

## 2026-10-24 engine debugging audit

Measured on a copy of the `data` branch at its 2026-10-25 01:39 UTC commit (49,782
distinct raw reports, 42,502 observations).

| Finding | Check | Result |
| --- | --- | --- |
| Items listed for over 2 days were stored on every poll | stored rows against distinct ids; `test_an_item_still_listed_is_stored_once` | raw 86,639 rows for 49,782 ids, observations 87,308 for 42,502; repeat gaps 49–84 h. Test fails before the fix, passes after |
| A processor exception lost the other URLs' reports and stopped the scheduler thread | `test_a_failing_processor_loses_no_reports` | fails before, passes after |
| An impossible date ("Feb. 30") raised in processing; "0 pm" read as noon | `test_impossible_time_is_none_not_an_error` | none of 17,113 stored texts triggered it; test fails 4 of 5 cases before, passes after |
| `/stream` skipped observations written more than 1 s after their poll began | live server, report written 9 s after its poll start, read with curl | before: not sent; after: sent once |
| Deduplicated views chose among differing copies arbitrarily | five fusions of the same store | before: 6,895 or 6,896 events; after: 6,898 three times |
| `store.replace` could fail on Windows after deleting the old table, leaving `.new` directories | fusion in a watched folder | failed 4 of 4, later 0 of 5 (intermittent); rename-aside run 3 of 3 |
| FIRMS observation ids depended on region order | code reading | keyed by region id |
| Fusion linkage cost | `components` on the store, and replicated 9 times in time | NumPy 0.54 s / 4.81 s; Rust kernel 0.06 s / 0.58 s |
| Kernel equals reference | Hypothesis test against brute force (both implementations); `test_kernel_equals_numpy` (n = 0, 1, 2,000) | passing; events of all seven regions from the same observations identical to HEAD's `fuse.events` (6,898 rows, all fields, both directions) |
| Batch classification | `derive` of 8,000 news/Telegram reports, translation stubbed | 4.13 s to 0.60 s, identical output digest |
| Existing copies on the `data` branch removed (2026-10-25, 8da148da) | each table rewritten from its deduplicated view; new view against old, `EXCEPT ALL` both ways, all columns (the comparison reports 1 when one row is dropped) | raw 86,639 → 49,782 rows, observations 87,308 → 42,502, one per id; 0 differing rows each way |
| Store layout by write date (2026-10-25) | copy of `data` at 8da148da, observations rewritten; `EXCEPT ALL` both ways; endpoint and fusion timings before and after, same machine | 0 differing rows; files 986 → 6 observation, 455 → 6 event; times in Decisions |
| Cron continuity (P2 done-when) | GitHub Actions API, 2026-10-21 to 2026-10-25 | 26 scheduled runs, all successful, every 2–5 h instead of every 15 min; schedule now twice a day |

Gates after the change: **137 Python tests**, **28 Node tests**, Ruff, pip check,
rustfmt and clippy clean; production build passed; ESLint 0 errors, 17 warnings as
before. The kernel was not measured on Linux; CI builds and tests it there.

## 2026-10-24 audit

- Final local gates: **129 Python tests**, **28 Node tests**, Ruff and pip
  consistency checks passed; production Next.js build passed. ESLint completed
  with no errors and 17 existing warnings. Python reports one Starlette
  TestClient deprecation warning; Node reports package-module-type warnings.
- South Asia count indicators have no calibrated alert cutoffs. Shipping
  smoothing follows the published World Bank centered seven-day window with
  a full-calendar-window restriction; tests check its dates and missingness.
  No satellite detector, COCO-to-SAR transfer or maritime model accuracy was
  measured. The assessment is in [Maritime methods](MARITIME-METHODS.md).

- New public context: live collection returned **17 ok**, and the engine view read
  back **17 series**. Spot dates, port/chokepoint dates and country observation
  years are retained separately. Recorded provider responses are test fixtures.
- The built Energy & Trade view was checked in headless Edge at 1440 and 390 px.
  It displayed all eight country rows and stored oil/shipping data through Next.js
  and the engine, switched regional context without stale rows, and reported no
  page errors. Unrelated monitor requests were suppressed in this focused check;
  it is not an end-to-end validation of every feed.
- The live oil route returned all five futures quotes with source timestamps
  after the missing-`previousClose` fallback was exercised.
- Source research, live probes and corrected identifications are in
  [Review log](REVIEW-LOG.md); calculations and limits are in
  [Data context](DATA-CONTEXT.md).
- Compatible dependency patches resolve the critical/high findings from this
  audit. `npm audit --omit=dev` reports **0 vulnerabilities**. Full `npm audit`
  retains three moderate entries through Tailwind 3's selector-parser chain.
  Its proposed Tailwind 4 migration is not included; ordinary trusted-source
  build usage is outside the advisory's stated untrusted-selector exposure.
- Live AIS coverage, South Asia classifier accuracy, causal explanations for
  trade changes, and cross-process compaction remain unvalidated.

## Earlier checks

| Claim | Check | Status |
| --- | --- | --- |
| `conflict.json` equals the TypeScript configs | `tests/test_regions.py::test_conflict_json_is_a_fresh_export` | passing 2026-10-21 |
| Every news feed and Telegram channel has an Admiralty grade | `test_every_source_is_graded` | passing 2026-10-21 |
| RSS and Atom parse correctly | recorded BBC, Google News and Meduza responses; RFC 4287 §1.1 example | passing 2026-10-21 |
| A failed fetch is a status, not an exception | `test_failure_is_a_status_not_an_exception` (ok, empty, blocked, http_error, parse_error) | passing 2026-10-21 |
| Re-collecting is idempotent | `test_store_is_idempotent_and_empty_tables_query` | passing 2026-10-21 |
| Rebuild reproduces derived tables from raw reports | `test_rebuild_reproduces_derived_tables` | passing 2026-10-21 |
| Engine `/api/telegram` has upstream's `TelegramData` shape | `test_telegram_route_has_upstream_shape` | passing 2026-10-21 |
| Live collection, P0 (news only) | `engine collect`: 47 of 50 feeds ok, 3 `http_error` (Reuters ×2, Kyiv Independent); 585 raw reports → 465 observations | observed 2026-10-21 |
| Every adapter parses a recorded real response | `tests/test_collect.py` (Telegram, FIRMS, ADS-B, alerts.com.ua, Neptun, Yahoo, CoinGecko, Polymarket); Tzeva Adom from upstream's interface | passing 2026-10-21 |
| Engine URLs equal upstream's | `test_urls_are_built_as_upstream_builds_them`, `test_encode_uri_component_equals_javascript` (Node) | passing 2026-10-21 |
| Polymarket region filter equals upstream's | `test_polymarket_filter_equals_javascript` (Node) | passing 2026-10-21 |
| The proxy serves what was stored | `test_proxy_serves_what_was_stored`; unknown host → 502 | passing 2026-10-21 |
| Compaction keeps every row | `test_compaction_keeps_rows` | passing 2026-10-21 |
| Every route works through the engine | manual 2026-10-21: 23 routes 200; engine log 106 `/fetch`, 103 × 200, 3 × 502 (Reuters ×2, Kyiv Independent fail at origin); 0 unscheduled | observed 2026-10-21 |
| Full live collection | 176 sources: 169 ok, 1 empty, 3 http_error, 3 parse_error (preview-disabled channels, now `empty`); 4,944 raw → 4,589 observations, 38 track points | observed 2026-10-21 |
| Fusion partition = connected components of the link graph | `test_components_equal_brute_force` (Hypothesis, 200 cases, exhaustive BFS reference) | passing 2026-10-21 |
| Events do not depend on input order | `test_events_do_not_depend_on_input_order` (all permutations sampled) | passing 2026-10-21 |
| Credibility monotone in corroboration; contradiction → 5; table | `test_credibility_is_monotone_in_corroboration`, `test_contradiction_forces_improbable`, `test_credibility_table` | passing 2026-10-21 |
| Indicator and PIR status rules | `test_indicator_board` | passing 2026-10-21 |
| Live fusion | 4,589 observations → 1,036 events in 2.0 s; credibility 2: 2, 3: 564, 4: 470; 135 aircraft entities | observed 2026-10-21 |
| Indicator board through the dashboard | manual 2026-10-21: `/api/indicators` via Next for both regions; II-3 answered (II-3a 228, see Decisions, Open), RU-2 answered, RU-1b watch (18 tracks) | observed 2026-10-21 |
| Cron runs 24 h without repair (P2 done-when) | `Collect` workflow history on GitHub | open: started 2026-10-21 |
| Kind rules are accurate | 200 labelled observations: population-weighted accuracy 0.543 | measured 2026-10-21 |
| Classifier beats rules | `test_classifier_beats_rules_under_cross_validation`: 0.68 vs 0.543, 3-fold CV | passing 2026-10-21 |
| TOML regions satisfy upstream's ConflictConfig | `tsc --noEmit` over `generated.ts`; `test_generated_typescript_is_fresh` | passing 2026-10-21 |
| Region files are complete | `test_every_source_is_graded`, `test_pir_indicators` (gates, z thresholds, hypotheses reference real indicators) | passing 2026-10-21 |
| Event times are extracted correctly | `test_event_time` (clock, 24 h, previous day, announcement, vague, invalid), `test_event_time_across_new_year` | passing 2026-10-21 |
| Contradiction rule | `test_covered_silence_contradicts`, `test_no_contradiction` (4 cases), Kyiv regression | passing 2026-10-21 |
| Thermal persistence | `test_persistent_heat_source` | passing 2026-10-21 |
| Transit indicator detects a collapse; sparse isolated points and long AIS gaps do not imply coverage or crossings | `test_transit_indicator_detects_a_collapse`, `test_transit_indicator_without_history_has_no_data`, `test_segment_crossing`, `test_transit_gap_over_policy_limit_is_not_counted`, `test_isolated_ais_points_do_not_count_as_sampled_coverage` | passing 2026-10-23; sample policy not live-validated |
| ACH ranking | `test_ach_ranks_by_inconsistency` | passing 2026-10-21 |
| Products | `test_intsum_reports_each_pir_and_cites_its_standards`, `test_products_route_serves_latest_scheduled_product` | passing 2026-10-23 |
| AIS path | `test_positions_only_and_nanosecond_times`, `test_without_key_the_stream_is_blocked`, `test_with_key_messages_become_vessel_tracks` (documented format, no live message) | passing 2026-10-21 |
| Six regions live | 261 polls: 251 ok, 5 empty, 3 http_error, 2 blocked (Arab News); 20,160 items; all 44 routes of the four new regions 200 through the engine | observed 2026-10-21 |
