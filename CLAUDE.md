# Working contract

Read before changing anything. [README](README.md) states what the platform is;
[Design](docs/specs/2026-10-20-open-source-watch-design.md) states how it is built;
[Decisions](docs/DECISIONS.md) states why; [Validation](docs/VALIDATION.md) states what
must be, and has been, checked. This file states the rules a change must not break.
Codex reads the same file through [AGENTS.md](AGENTS.md).

Open-Source-Watch is a fork of [IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT)
(MIT, Nobler Works). The Next.js front end is upstream's, in upstream's layout; the
Rust engine in `engine/` (crate and binary `osw`) runs the intelligence cycle behind it.

## Always on

| Rule | Reason |
| --- | --- |
| Report no outcome that was not read back from the result that produced it. | A requested behaviour is not a result. |
| Name a failed collection by its status, never by its cause until the cause is shown. | A blocked endpoint and a changed format both end as an empty feed. |
| Cite only what [Sources](docs/SOURCES.md) records, and add the entry in the change that uses it. | Grades, taxonomies and endpoints are justified by their source. |
| Add a dependency only after measurement. | Each crate is code to audit and build. |
| State what was measured and what was not. | "Not measured" is a finding. |
| Leave upstream files in place; edit them minimally. | `git merge upstream/main` must keep working. |
| Upstream URLs reach the engine only through `fetchWithTimeout` (the recording proxy); a route returns an engine view only through `fromEngine`. | With `ENGINE_URL` unset the app is plain IRONSIGHT, and upstream's shaping code stays the only copy. |
| Store at most 280 characters of article or post text. | The repository and its `data` branch are public. |
| Grades come from `regions/*/region.toml`; credibility is computed, never assigned by an adapter. | Admiralty grading separates the source from the claim. |

## The model is written once

- Feeds, channels and keywords live in `src/lib/conflicts/*.ts`. The engine reads their
  export `regions/<id>/conflict.json` (`node scripts/export-regions.mjs`); never edit
  that JSON by hand.
- The table schemas live in `engine/src/schema.rs`, the event taxonomy in `KINDS` there.
- Upstream's shaping code is the only copy: the engine serves upstream responses
  (`/fetch`), and every engine source URL is built exactly as the upstream route builds
  it (tests check this against Node). `/api/telegram` is the one engine-shaped route.
- The gazetteer is `regions/geonames.tsv`, rebuilt by `osw gazetteer`.
- A new region is one file, `regions/<id>/region.toml`, with a `[conflict]` table;
  `osw regions` writes it into `src/lib/conflicts/generated.ts` (never
  edit that by hand). The two upstream theaters keep their TypeScript configs.
- Optional `[context]` countries and ports select recorded economic context.
  `engine/src/collect/context.rs` stores releases; `engine/src/context.rs` serves
  `/api/context` through `fromEngine`. Units and observation dates remain attached;
  missing measurements never become zeros. See `docs/DATA-CONTEXT.md`.
- Optional `[outages]` countries (ISO alpha-2, seat of government) select IODA
  outages. GNSS cells, outages, Taiwan MND reports and UCDP reference events are
  served by `/api/signals` (`engine/src/signals.rs`); UCDP events never become
  observations, since `osw validate` scores the engine against them. See
  `docs/SIGNALS.md`.
- Kind labels for the classifier live in `docs/labels/`; the classifier is retrained
  from them at start-up, so derived tables depend on raw reports and labels only.
- A payload's format belongs to its adapter: `engine/src/collect/<adapter>.rs` writes it
  and its processor in `engine/src/process` reads it. Every other module reads raw reports
  through `collect::stored` or the adapter's reader (`mnd::reports`,
  `ioda::events`, `ucdp::events`, `context::releases`, `rss::outlets`), never by
  parsing `payload` elsewhere.
- Raw reports are the only state that cannot be recomputed. `osw rebuild`
  regenerates every derived table from them.

## Code

Code transcribes the method: names from the design document, the rule stated in each
public docstring, comments that cite the source. A failed collection is a status, not an
exception.

## Run

`osw` is `engine/target/release/osw` after `cargo build --release` in `engine/`.

```text
osw serve                    # API on :8790, collects every source by cadence
osw collect [--region R]     # one poll of every source
osw fuse                     # recompute events and entities
osw analyse intsum sitrep    # indicator snapshot and products
osw regions                  # after editing any region.toml
osw validate                 # recall of violent events against UCDP Candidate
git worktree add data data   # once: ./data = the cron's `data` branch
AISSTREAM_KEY=... osw serve  # AIS for the transit indicators (free key)
npm run dev                  # IRONSIGHT front end; set ENGINE_URL=http://127.0.0.1:8790
```

## Before a change is finished

```text
cargo fmt --check                                     # in engine/
cargo clippy --release --all-targets -- -D warnings
cargo test --release
npm run lint
npm test
npm run build
```

## Upstream notes

- Next.js + TypeScript + Tailwind; Leaflet maps. Upstream API routes in
  `src/app/api/*/route.ts`, one per feed; panels in `src/components/panels/`; theater
  configs in `src/lib/conflicts/`; response types in `src/lib/types/api.ts`.
- No API keys required; features and polling intervals are in the README.
- Update README, `docs/`, this file and TODO.md in the same commit as the change that
  makes them wrong. A stale doc is a bug.
