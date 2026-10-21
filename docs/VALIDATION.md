# Validation

What must be checked, how, and what has been. Checks are invariants and recorded real
responses, never snapshots of live output.

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
| Transit indicator detects a collapse; no data without history | `test_transit_indicator_detects_a_collapse`, `test_transit_indicator_without_history_has_no_data`, `test_segment_crossing` | passing 2026-10-21 |
| ACH ranking | `test_ach_ranks_by_inconsistency` | passing 2026-10-21 |
| Products | `test_intsum_reports_each_pir_and_cites_its_standards` | passing 2026-10-21 |
| AIS path | `test_positions_only_and_nanosecond_times`, `test_without_key_the_stream_is_blocked`, `test_with_key_messages_become_vessel_tracks` (documented format, no live message) | passing 2026-10-21 |
| Six regions live | 261 polls: 251 ok, 5 empty, 3 http_error, 2 blocked (Arab News); 20,160 items; all 44 routes of the four new regions 200 through the engine | observed 2026-10-21 |
