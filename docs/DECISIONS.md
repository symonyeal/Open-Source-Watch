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

## Open

- Gazetteer: P0 geocodes against the conflict's `strikeLocations` table, so places
  outside it (for example Fujairah) fall back to a country match. GeoNames is planned for
  P1.

## Tested and refused
