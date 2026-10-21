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
| Kind rules are accurate | not measured; needs a labelled sample (P3 classifier) | open |
| Fusion and credibility properties | P2 | open |
