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
| Rebuild reproduces derived tables from raw reports | `test_rebuild_reproduces_observations` | passing 2026-10-21 |
| Engine `/api/news` has upstream's `NewsItem` shape | `test_news_route_has_upstream_shape` | passing 2026-10-21 |
| Next.js forwards to the engine when `ENGINE_URL` is set | manual: `next start` with `ENGINE_URL`, response header `x-served-by: engine`, 100 items | observed 2026-10-21 |
| Live collection | `engine collect`: 47 of 50 feeds ok, 3 `http_error` (Reuters ×2, Kyiv Independent); 585 raw reports → 465 observations | observed 2026-10-21 |
| Kind rules are accurate | not measured; needs a labelled sample (P3 classifier) | open |
| Fusion and credibility properties | P2 | open |
