# Validation

What must be checked, how, and what has been. Checks are invariants and recorded real
responses, never snapshots of live output.

## 2026-11-05 Generations, source admission and focused views

Local Windows checks: `cargo fmt --check` and release Clippy with warnings denied
passed. The full release suite passed 146 tests. After the final change to stage
preserved input blobs before renaming them, the storage/API suite passed all 17
parent tests again. Its ignored child helper is invoked by the parent, including
eight forced process exits and an independent writer-lock attempt. The process
exits leave the previous manifest bytes and all eight tables unchanged; a retry
then commits. A pinned reader retains its old tables after a concurrent commit.

Schema tests reject absent/null required columns, missing event membership,
nonfinite values and invalid coordinates. Runtime region validation is tested
with malformed metrics, geometry, thresholds and source declarations. Source
fixtures check duplicate dates, invalid ship counts, complete-week requirements,
zero baselines, missing MND fields, revisions and acquisition-time cutoffs.

`npm test` passed 33 tests, including nonzero exit after total HTTP 503 failure,
rejection of partial snapshots, malformed payloads, engine fallback, missing
provenance and mixed generations. Accepted fixture snapshots reconcile dynamic
area products and hashes. Production build/type checking passed; lint has zero
errors and 17 existing warnings in upstream files.

Bounded public endpoint checks are recorded in [Review log](REVIEW-LOG.md).
Metadata access does not establish data-query success: PortWatch chokepoint
queries returned provider errors, while a port query succeeded; FRED CSV requests
timed out here. These results neither justify an unverified replacement nor
establish a global outage. Recorded releases, their ages and collection status
remain visible. Predictive calibration, sudden power loss, sustained load and
cross-platform exact computational replay were not measured.

The public `data` revision `8e446f1d` was copied and rebuilt offline: 169,680 raw
reports reprocessed into one generation. All 126 baseline files remained present;
all 65 preserved input blobs matched their SHA-256 names. A binary copied outside
the repository ran with packaged regions/labels via `OSW_ROOT`. Against that
server, the production Next.js app accepted 58 snapshot responses (56 required
plus two area products) from one generation, including nonempty country context
and news. This checks migration, resource relocation and extraction on this
machine; it is not a browser interaction or load test.

## 2026-11-03 Published products, incident linkage, translation, second-week sample

Measured on a copy of the `data` branch at 82ad7ac3 (158,002 raw reports), rebuilt
offline with this engine, and on the published dashboard of 2026-11-03 03:02 UTC.

| Claim | Check | Result |
| --- | --- | --- |
| The published INTSUM assessed nothing | the dashboard's seven INTSUMs; its source table | every text PIR "Cannot be assessed: collection gap"; Russia/Ukraine sources 63 ok, 45 stale: cron runs hours old read against 5-minute cadences |
| It does now | `osw analyse intsum sitrep`, then `osw serve --no-collect`, on the copy (last collection 00:42 UTC, read at 04:45) | Russia/Ukraine sources 104 ok, 4 failed, 6 unknown; RU-2, RU-3, RU-5 assessed; RU-1 a gap (alerts and drone tracks are collected only locally); board and INTSUM agree on every indicator |
| The snapshot server writes nothing | `/fetch` of the BBC feed through `osw serve --no-collect`, files of the store changed; `a_read_only_poll_stores_nothing` | 200, 30,232 bytes, 0 files; the HEAD engine on a second copy wrote a run row. Test passing |
| An alert count needs the alert feed | `an_alert_indicator_without_its_sensor_is_no_data` | passing; RU-1a was "inactive" on the published board with only news healthy |
| Cron runs are fresh for three periods | `source_health` (10 h old: ok; 37 h: stale) | passing |
| Duplicates resemble an earlier event of their flag; real events do not | overlap coefficient of each judged event against the earlier events of its flag, odd and even flags | at θ = 0.6, odd: 11 of 72 duplicates, 0 of 24 real; even: 29 of 76, 0 of 27. Jaccard at 0.3: 14 of 72 and 24 of 76 |
| Incident linkage removes artefact flags | the backtest of 2026-11-02 (`--step 6 --healthy`), scored on the judged flags | flags 64 → 39; supported 13 → 11 kept; artefacts 51 → 26 kept; RU-2c 7 → 0, RU-5a 48 → 30, RU-5b 9 → 9. The two new flags (Poltava 10-31, Kyiv Oblast 11-01) judged blind: both artefacts, Poltava borderline (2 real; the reviewer named one event that would make 3). Supported share 20 % → 28 % |
| It does not chain | event spans on the copy | Russia/Ukraine 3,864 → 3,702 events, longest 26.2 h. Without the span bound: longest 457 h, 4,995 reports (refused) |
| Other regions are unchanged | the copy fused by HEAD and by this engine, events as sets of observation ids | six regions identical |
| `reports_of_one_incident_link_across_a_day` | merge, other oblast, other text, alert feed, span bound | passing |
| The classifier holds on another week | 300 text observations of 2026-10-27 to 11-02, simple random (200 all regions, 100 South Asia), labelled blind (`docs/labels/kinds-week2.jsonl`) against the stored kind | accuracy 0.70 (all) and 0.70 (South Asia), as 0.68 under cross-validation; but of 27 violent incidents (battles, strikes, violence against civilians) 13 were stored as violent, and of 24 stored violent 13 were. South Asia: 2 of 25 labelled events of any kind stored with their kind. Of the 14 violent incidents missed, 7 are untranslated texts; 13 were stored as `report` |
| Stored texts are translated | text observations whose `text_en` is over 30 % Hebrew, Arabic or Cyrillic | 12.1 % of 41,299 on the `data` branch (5b4d3cd7), the same on the copy; by day 0 % on most days, 25 % (10-25), 41 % (10-28), 26 % (11-01), 45 % (11-02), 35 % (11-03) |
| `osw retranslate` repairs them | run on a copy | not measured: Google's endpoint returned its block page to this machine; 5,060 reports tried, 0 translated, 68 s (the pause after 20 failures bounds the run) |
| Translations are excerpts | longest `text_en` on the copy; `a_translation_is_an_excerpt` | 533 characters before; test passing. The `data` branch keeps the long rows until it is rebuilt |
| A local engine collects on Windows | `osw collect --adapter context --region south-asia` | before: 11 http_error (native-tls: no user-specified root in the chain); after: 7 ok, 4 http_error (PortWatch timeouts at 10 s) |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, tsc, ESLint, `npm test`, `npm run build` | clean; clean; 131 passed, twice; clean; 0 errors, 17 warnings as before; 28 passed; built |

Not measured: the Pages job on this engine (the next run is the check), the Docker job
in CI, the Telegram route's memoisation (it serves only when `ENGINE_URL` is unset),
and `regions/sites.tsv` (Overpass did not answer).

## 2026-11-02 Flagged oblast-days judged; area INTSUM; `data` rebuilt

The 64 oblast-days flagged in the backtest below (rebuilt store at 5b4d3cd7, every
source ok) were judged from the texts of the events each counted
(`docs/labels/unusual-flags.jsonl`, 296 events, an independent reviewer blind to the
rule, two flags checked by hand). A flag is supported if it holds at least 3 real,
distinct incidents in the oblast.

| Claim | Check | Result |
| --- | --- | --- |
| Flags are real surges | supported flags of 64 | RU-2c 0 of 7; RU-5a 8 of 48; RU-5b 5 of 9; 13 of 64 (20 %) |
| Why flags fail | the 296 events' labels | real 108, duplicate 149, misplaced 5, not an event 34. Most artefacts are one incident fused into several events: reports of one night's strike run for a day, fusion links within τ = 2 h (Kstovo refinery, 11-01: 3 events, 24 + 1 + 4 reports) |
| Counting places instead of events helps | the same backtest with each oblast's window counting distinct raions, then distinct gazetteer points, scored on the judged flags | raions: 7 flags, 2 of 13 supported kept (29 %); points: 19 flags, 3 of 13 kept (16 %). Separate strikes share a raion or the oblast's point, so neither rule was kept |
| The area INTSUM is the region's report restricted | `area_intsum` (`engine/tests/analysis.rs`) | the region INTSUM tables its oblasts; an oblast's INTSUM keeps its events and the per-oblast indicators at its count |
| `data` carries the phase 2 fields | `osw rebuild` of 5b4d3cd7 (158,002 raw reports, 43 s, offline from the translation cache) | Russia/Ukraine violent observations: target 0.9 → 70.2 %, actor 1.0 → 84.0 %, killed 0.7 → 20.6 %, site 0 → 3.3 %; 4,413 → 3,847 after the statement filter; 22 observation files changed, no raw report |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, tsc, ESLint, `npm test`, `npm run build` | clean; clean; 127 passed (126 and `area_intsum`) in three runs; a fourth, started while a local `osw serve` was stopping, failed one test, not reproduced; clean; 0 errors, 17 warnings as before; 28 passed; built |

The unusual indicators (RU-2c, RU-5a, RU-5b) are therefore right about one flag in
five on this history. Duplicate events, not the Poisson baseline, are the main error;
the fix belongs in incident linking, not in the indicator. `context_collection_reaches_recorded_view`
viewed a run at a time before it started, which health as of a time now excludes; the
test views after the run.

## 2026-11-02 Russia/Ukraine PIRs and unusual activity by oblast

Backtest (`osw backtest russia-ukraine 2026-10-14 2026-11-03 --step 6`) on a copy of the
store rebuilt with the phase 2 engine. The board ran every 6 hours (81 steps), first
with source health as of each step, then with every source ok (`--healthy`).
Unit cases are in `engine/tests/analysis.rs`.

| Indicator | Health as of each step | Every source ok: steps triggered / watch, episodes | First steps of episodes (every source ok) |
| --- | --- | --- | --- |
| RU-1a air-raid alerts | no data 80 of 81 | 0 / 0 | none (1 alert event stored) |
| RU-1b tracked drones and missiles | no data 80 | 0 / 0 | none |
| RU-2a energy strikes in one oblast (24 h, 1 / 3) | no data 80 | 14 / 42, 3 | 10-23 18:00 Odesa 3; 10-31 00:00 Samara 3; 11-01 18:00 Nizhny Novgorod 3 |
| RU-2b thermal anomalies | 29 / 12, 7 | 39 / 29, 10 | 10-17 12:00 41; 10-23 12:00 73; 10-28 06:00 77 |
| RU-2c unusual energy strikes | no data 80 | 11 / –, 3 | 10-24 12:00 Crimea 3 (expected 0.17); 10-31 Samara 3 (0.05); 11-01 Nizhny Novgorod 3 (0.03) |
| RU-3a maritime incidents | no data 80 | 0 / 0 | none |
| RU-5a unusual strikes or explosions | no data 80 | 39 / –, 4 | 10-22 12:00 Poltava, Rivne, Zaporizhzhia 4 each (expected 0.76); 10-26 Voronezh 5 (0.21); 10-28 Luhansk 4 (0.78); 10-29 Volgograd 4 (0.05) |
| RU-5b unusual battles | no data 80 | 19 / –, 7 | 10-22 Zaporizhzhia 5 (0.76); 10-27 Donetsk 7 (1.71); 11-01 Donetsk 6 (1.70) |

Findings:
- **Historical health reads as stale.** Text sources were "stale" at every step,
  because their cadences are 2 to 5 minutes (`osw serve`) and the stored history was
  collected by the scheduled workflow a few times a day. The text indicators are
  therefore "no data" on the stored history. The `--healthy` run measures the rules
  alone. Source health at a past time now counts only runs started by then
  (`cycle::source_table`); before, later runs counted.
- **A daily rate over-fires while collection grows.** RU-5a was triggered at 52 of 81
  steps with a rate per day, and at 39 with the rate per report (DECISIONS).
- **Not yet measured.** RU-5a still fires at about half of all steps: it is triggered
  when any of about 25 oblasts is unusual. Whether the flagged oblast-days were real
  surges is not measured, and no labelled sample exists yet. The full-volume history
  (from 2026-10-20) is shorter than the 28-day window, so the expected rates rest on
  the low-volume weeks before it.

## 2026-11-02 Russia/Ukraine incident detail

Incident detail (`engine/src/process/what.rs`): the statement filter, ACLED sub-event,
weapon, missile type, target class, killed, injured and the actor pair, measured
against four samples of 250 Russia/Ukraine reports labelled by hand
(`docs/labels/detail-*.jsonl`, by `detail-guide.md`). The rules were tuned on two
samples and scored on two held out. A labelling agent that did not see the rules' output
labelled each sample, and 30 rows of each held-out sample were checked by hand. A
prediction is right when it equals the label. Precision is right among the rows the
rules fill; recall is right among the rows the label fills.

| Claim | Check | Result |
| --- | --- | --- |
| The labels are right | 30 rows of each held-out sample checked by hand | test 28 agree; fresh 29 agree. The one fresh error (n 112, injured 16, a count of missiles) was corrected before scoring |
| The filter keeps real events | first held-out sample (test), rules frozen | 19 of 178 labelled events dropped (event P 0.87, R 0.89). A clause that keeps any report naming a weapon was added after this score; on the same sample it drops 10 |
| The filter keeps real events, final | second held-out sample (fresh, seed 20261112), not seen before scoring | 4 of 165 labelled events dropped (R 0.98); 57 of 85 statements dropped. On the violent kinds the filter acts on (`engine/tests/what.rs`): 4 of 163 events and 28 of 50 statements dropped |
| The fields are right | fresh sample, P / R | sub-event 0.77 / 0.88; target 0.76 / 0.78; killed 0.84 / 0.82; injured 0.71 / 0.66; actor pair 0.85 / 0.83; missile type 0.65 / 0.87; weapon 0.78 / 0.93 (drone 0.79 / 0.97, missile 0.70 / 0.95, glide bomb 1.00 / 0.67 of 3, artillery 0.88 / 0.70 of 10) |
| The engine computes what was scored | engine rules (`osw::process::what`) against the probe's output on fresh | identical |
| History takes the detail | `osw rebuild` of a copy of the store (156,355 raw reports, 9 min 11 s), against the store before phase 1b | Russia/Ukraine: 563 of 26,012 observations moved to report, events 4,047 → 3,829. Of the events, sub-event 40.6 %, weapon 27.9 %, target 27.6 %, actor 33.5 %, killed 6.6 %, injured 6.4 %, missile type 2.7 %. Targets: civilian 477, military 286, infrastructure 148, energy 146. Placements: the 223 site placements of phase 1b, no other |
| Other regions are unchanged | the same comparison for the other six regions (84,651 observations) | 0 kinds, 0 placements and 0 detail fields changed; events unchanged |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, ESLint, `npm test`, `npm run build` | clean; clean; 124 passed (121 and the three detail tests); 0 errors, 17 warnings as before; 28 passed; built |

Remaining errors, fresh sample:
- **Statements that name a weapon are kept.** The weapon clause trades them for real
  events. 19 of the 27 wrong weapons are on statements, for example "Drone violates
  Moldovan airspace" and North Korea's drone demonstration. These lower weapon precision
  (0.78) and event precision (0.85).
- **Air defence hit read as the weapon used.** "S-400 Triumph … hit" gives missile type
  S-400 (3 of the 7 wrong missile types). A "ballistic threat" before explosions gives
  ballistic (1).
- **Injured taken from the wrong number.** Ages ("two girls aged 15 and 16" gives 15)
  and periods ("699 times in past 24 hours, six injured" gives 24). Spelled-out numbers
  are missed.

The samples are text alone (280 characters at most); the rules see the same text.

## 2026-11-02 Russia/Ukraine sites

Site matching (`engine/src/process/place.rs`) on the same 18,319 stored Russia/Ukraine
text observations, before `regions/sites.tsv` exists: a town with a site word becomes a
`site` location at the town's point. The OpenStreetMap sites file is checked separately
when the Sites workflow has written it.

| Claim | Check | Result |
| --- | --- | --- |
| Reports name sites | grain of the 5,027 classified observations | site 0 → 2.7 % (135), city 52.0 → 49.6 %; of all 18,319 text observations 223 (1.2 %) changed; oblast 61.9 %, raion 24.1 % |
| The site placements are right | 100 of the 108 changed placements not seen in tuning (seed 20261106), judged by an independent reviewer from text and placement only, spot-checked | 90 right; the reviewer said 91, one overruled ("Moscow airfield" for Sheremetyevo) |
| Other regions are unchanged | the comparison on the other six regions' 21,621 text observations | 0 changed |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, ESLint, `npm test`, `npm run build` | clean; clean; 121 passed; 0 errors, 17 warnings as before; 28 passed; built |

Wrong placements remaining: a Russian adjective read as a village of another oblast
("Pridneprovskaya", "Preobrazhensk", "Kamensky" plant placed in Smolensk or Sverdlovsk
Oblast, 4); a city named for a site in its oblast ("Rostov plant", "Saint Petersburg
refinery", "Moscow airfield", 3); "works" as a verb ("Ballistics works in Kyiv"); a
mistranslation ("Akhtubinsk nuclear power plant", the airfield); an oil pumping station
named an oil depot.

## 2026-11-02 Russia/Ukraine places

Place matching (`engine/src/process/place.rs`) on the stored Russia/Ukraine reports of
the `data` branch at 21a0d63e: 5,018 observations classified as events, before and
after, and samples of the placements that changed, drawn at random and judged against
the report text.

| Claim | Check | Result |
| --- | --- | --- |
| Reports are placed more finely | grain of the 5,018 observations: city or village / oblast or raion / country / none | before 51.0 / 0 / 16.8 / 32.2 %; after 52.1 / 12.9 / 14.4 / 20.7 %; oblast known 0 → 61.8 %, raion 0 → 23.8 % |
| The changed placements are right | 100 changed placements from a held-out sample, judged by an independent reviewer and spot-checked | 89 right, 7 wrong, 4 digests naming several places; the four tuning samples before it scored 73, 80, 83 and 74 right |
| INTSUM names the place chosen | events whose first name is a country | 421 → 0 |
| Other regions are unchanged | the same comparison for the other six regions | four unchanged; Red Sea, 21 more placed (Taiz), and Black Sea, 4 more (Ukrainian spellings such as Oleshki), all right |
| History takes the units | `osw rebuild` on a copy of the store (156,355 raw reports, 11 min 53 s) | Russia/Ukraine: oblast on 62.8 % of 32,708 observations and 80.0 % of 4,047 events; raion on 14.5 % and 22.1 % |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, ESLint, `npm test`, `npm run build` | clean; clean; 120 passed (118 and the two place tests); 0 errors, 17 warnings as before; 0 failed; built |

Wrong placements remaining: person and unit names that are also places ("Vladimir
Raevsky", "Vostok … groups"), Kyiv meaning the government, and the spelling Vinnitsa,
absent from GeoNames. Sites: the section above.

The `data` branch itself was rebuilt with this engine (f61be21) on a fresh worktree of
21a0d63e: `osw rebuild` in 8 min 18 s, oblast on 62.8 % of 32,708 observations and
80.0 % of 4,047 events, as on the copy. 35 observation files changed, no raw report;
events are not tracked on `data`. Pushed as 209100de.

## 2026-11-01 Rust engine

Parity on a copy of the `data` branch at 7e2e781e (138,811 raw reports), rebuilt by the
Python engine and by `engine/`, then compared on every derived table (observation,
track_point, event, entity) and, for each region, `/api/signals`, `/api/context`,
`/api/indicators`, `/api/telegram`, `/sources`, INTSUM and SITREP at
2026-11-01 00:00 UTC, offline (no translation requests).

| Claim | Check | Result |
| --- | --- | --- |
| Derived tables and views are unchanged | the 54 tables and views, compared whole, against the Python engine with its classifier refitted at `tol = 1e-8` | 54 of 54 identical (99,825 observations, 15,292 events); `last_run`, a time of the comparison run, excluded from the source table |
| The classifier's kinds are the optimum's | the 23 observations whose kind differs from Python at its default `tol = 1e-4`, refitted at `1e-4`, `1e-8`, `1e-12` | Rust agrees with `1e-8` and `1e-12` on 23 of 23, with `1e-4` on 0 |
| Training converges | ‖∇‖∞ of the loss at the returned weights | ≤ 1e-10 after 106 iterations, 0.29 s; before the line-search change 3.5e-7 after 20,000 (273–337 s) |
| Cost | the rebuild of the same store, timed once | Python 17.8 s, Rust 7.3 s |
| `generated.ts` is the export of the region files | `osw regions`, then `git diff` | only the header line (the command name) differs |
| The server answers the dashboard's routes | `osw serve --no-collect` on the rebuilt copy: `/regions`, `/api/indicators`, `/api/signals`, `/api/context`, `/api/telegram`, `/events`, `/observations`, `/api/products` | all 200 |
| The pytest suite's behaviour holds | 109 of its 113 tests ported to `engine/tests` (111 tests with two added) on the same fixtures; the two keyword-rule tests dropped with the rules | passing; not ported: a processor that panics loses no reports, and `/fetch` serving a stored response (both need an injected fetch; the offline `/fetch` failure is tested) |
| Gates | `cargo fmt --check`, `cargo clippy --release --all-targets -D warnings`, `cargo test --release`, ESLint, `npm test`, `npm run build` | clean; clean; 118 passed; 0 errors, 17 warnings as before; 28 passed; built |

Not measured: a live collection by the Rust engine on GitHub Actions (the first
scheduled run after the push is the check), and the Linux build.

## 2026-10-29 raw reports through their adapters

Parity on copies of the `data` branch at 77246bea (2026-10-29 12:27 UTC, 113,964 raw
reports) and of the signals scratch store (11,415 raw reports): each copy rebuilt by
1841f087 and by this change, then compared on every derived table (observation,
track_point, event, entity) and, for each region, `/api/signals`, `/api/context`,
`/api/indicators` and `/api/telegram` at one fixed time, with translation off.

| Claim | Check | Result |
| --- | --- | --- |
| Derived tables and views are unchanged | the 33 tables and views of each store, compared whole | scratch store 33 of 33 identical; cron store 32 of 33: `/api/telegram` for Russia/Ukraine listed the same 120 posts, two with the same time in the other order. Ties fell in query order; they are now broken by channel and post id |
| Cost is unchanged | the same runs, timed once | rebuild 3.35 s → 3.28 s (scratch) and 11.0 s → 11.3 s (cron); views within the spread of single runs; repeated timings not measured |
| A gpsjam.org day is fetched until every region stores it | `test_gpsjam_region_cells_and_stored_days` | passing |
| A UCDP version is skipped only by the regions that store it | `test_ucdp_version_skipped_only_by_regions_that_store_it` | passing |
| The latest version of each UCDP event is kept | `test_ucdp_parse_keeps_region_events_with_version` | passing |
| An MND count the latest report does not state is no data, not 0 | `test_an_unstated_official_count_is_no_data` (board and stored snapshot) | passing; fails before the fix (count 0, "observed"). On the cron store, which holds no MND report, TW-1c's count is now null instead of 0; its status was and is "no data" |
| Rebuild survives a region or IODA country removed from the registry | `test_ioda_events_observations_and_linking`; a raw report of an unregistered region | passing; before, `KeyError` for the region and `StopIteration` for the country stopped `engine rebuild` |
| Gates | pytest, ruff, pip check, ESLint, `npm test`, `npm run build` (kernel unchanged) | 156 passed; clean; clean; 0 errors, 17 warnings, all on lines upstream wrote (diff against `upstream/main`); 28 passed; built |

## 2026-10-29 navigation, connectivity and reference signals

Live checks used a scratch store (one poll of every source except aircraft, alerts and
drone tracks, then fusion), not the `data` branch. Method in [Signals](SIGNALS.md).

| Claim | Check | Result |
| --- | --- | --- |
| The kernel's H3 cells are gpsjam.org's | every cell of gpsjam.org's 2026-10-27 file: centre, then cell of the centre; `test_kernel_cells_reproduce_gpsjam_indices` on a 474-cell sample | 47,482 of 47,482 indices reproduced; passing |
| An aircraft counts once per cell-day, bad if any report there was | `test_aircraft_counts_once_per_cell_day_bad_if_any_report_bad` | passing |
| Cell formula and bands are gpsjam.org's | `test_gpsjam_formula_and_bands` (2 % and 10 % edges, one bad aircraft discounted) | passing |
| Per-aircraft rule and validity filters follow God's Eye View | `test_per_aircraft_rule`, `test_integrity_filters_follow_gods_eye_view` (version, ground, position age, derived quality, last good position, box) | passing; the rule is not validated against known interference |
| Sources sharing a URL are parsed by their own adapters | `test_mixed_adapters_on_one_url_parse_by_their_own_adapter` | passing; before, the group took the first source's parser |
| The proxy does not take integrity snapshots at the dashboard's aircraft rate | `test_proxy_polls_a_colocated_source_by_its_own_cadence` | passing; before the fix a dashboard session stored 284 integrity cells in minutes |
| A stored gpsjam.org day is not fetched again | `test_gpsjam_region_cells_and_stored_days` | passing |
| MND reports parse to the stated counts and UTC window | report 88094 (`test_mnd_list_and_report`); year boundary and failures (`test_mnd_year_boundary_and_failures`); live backfill read against report texts | 6 sorties, 7 PLAN, 1 official, 5 ADIZ, window 22:00–22:00 UTC; 81 reports, 2026-08-10 to 2026-10-29, one per day |
| IODA outages of one country link; a failed IODA feed is a gap | `test_ioda_events_observations_and_linking` | passing; three measurements within and beyond τ give events of 2 and 1 |
| UCDP events keep their version and region | `test_ucdp_parse_keeps_region_events_with_version`; live v26.0.9 | 2,014 events; 1,080 inside six region boxes, none in Taiwan's |
| Recall windows, distance and eligibility | `test_recall_windows_distance_and_eligibility` | passing |
| Engine recall of UCDP events | `engine validate` | not measured: UCDP's newest release ends 2026-10-01, collection began 2026-10-21; 0 eligible in every region |
| Agreement of the engine's integrity snapshots with gpsjam.org | `agreement` in `/api/signals` | not measured: needs whole days of a local engine |
| First live collection | `engine collect --adapter gpsjam mnd ioda ucdp` | 48 runs: 16 ok, 32 empty (31 IODA countries with no outage in 7 days, Taiwan's UCDP box), 0 failed; 1,373 items in 116 s, of which the one-off MND backfill took 112 s |
| GNSS on live data | gpsjam.org 2026-10-22 to 10-28 | high cells per day: Iran/Israel 95–141, Black Sea 74–82, Russia/Ukraine 63–87, South Asia 69–104, Hormuz 2–25, Red Sea 3–30, Taiwan Strait 0 on all seven days |
| Dashboard and static snapshot | `/api/signals` and `/api/indicators` through Next; `scripts/snapshot.mjs`; Edge screenshots of the map and SIGNALS view at 1600 and 390 px | 200; 140 routes written, 0 failed (7 new, 44–76 KB); map draws the cells only after the hook was moved below map creation |
| Gates | pytest, ruff, pip check, cargo fmt and clippy, tsc, ESLint, `npm test`, `npm run build` | 154 passed; clean; clean; clean; clean; 0 errors, 17 warnings as before; 28 passed; built |

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
