# Working contract

Read before changing anything. [README](README.md) states what the platform is;
[Design](docs/specs/2026-10-20-open-source-watch-design.md) states how it is built;
[Decisions](docs/DECISIONS.md) states why; [Validation](docs/VALIDATION.md) states what
must be, and has been, checked. This file states the rules a change must not break.
Codex reads the same file through [AGENTS.md](AGENTS.md).

Open-Source-Watch is a fork of [IRONSIGHT](https://github.com/NoblerWorks-HQ/IRONSIGHT)
(MIT, Nobler Works). The Next.js front end is upstream's, in upstream's layout; the
Python engine in `engine/` runs the intelligence cycle behind it.

## Always on

| Rule | Reason |
| --- | --- |
| Report no outcome that was not read back from the result that produced it. | A requested behaviour is not a result. |
| Name a failed collection by its status, never by its cause until the cause is shown. | A blocked endpoint and a changed format both end as an empty feed. |
| Cite only what [Sources](docs/SOURCES.md) records, and add the entry in the change that uses it. | Grades, taxonomies and endpoints are justified by their source. |
| Add a dependency only after measurement. Use the system Python 3.14; no virtual environment. | One environment on this machine. |
| State what was measured and what was not. | "Not measured" is a finding. |
| Leave upstream files in place; edit them minimally. | `git merge upstream/main` must keep working. |
| Upstream URLs reach the engine only through `fetchWithTimeout` (the recording proxy); a route returns an engine view only through `fromEngine`. | With `ENGINE_URL` unset the app is plain IRONSIGHT, and upstream's shaping code stays the only copy. |
| Store at most 280 characters of article or post text. | The repository and its `data` branch are public. |
| Grades come from `regions/*/region.toml`; credibility is computed, never assigned by an adapter. | Admiralty grading separates the source from the claim. |

## The model is written once

- Feeds, channels and keywords live in `src/lib/conflicts/*.ts`. The engine reads their
  export `regions/<id>/conflict.json` (`node scripts/export-regions.mjs`); never edit
  that JSON by hand.
- The table schemas live in `engine/schema.py`, the event taxonomy in `KINDS` there.
- Upstream's shaping code is the only copy: the engine serves upstream responses
  (`/fetch`), and every engine source URL is built exactly as the upstream route builds
  it (tests check this against Node). `/api/telegram` is the one engine-shaped route.
- The gazetteer is `regions/geonames.tsv`, rebuilt by `py -3.14 -m engine gazetteer`.
- A new region is one file, `regions/<id>/region.toml`, with a `[conflict]` table;
  `py -3.14 -m engine regions` writes it into `src/lib/conflicts/generated.ts` (never
  edit that by hand). The two upstream theaters keep their TypeScript configs.
- Optional `[context]` countries and ports select recorded economic context.
  `engine/collect/context.py` stores releases; `engine/context.py` serves
  `/api/context` through `fromEngine`. Units and observation dates remain attached;
  missing measurements never become zeros. See `docs/DATA-CONTEXT.md`.
- Kind labels for the classifier live in `docs/labels/`; the classifier is retrained
  from them at start-up, so derived tables depend on raw reports and labels only.
- Raw reports are the only state that cannot be recomputed. `py -3.14 -m engine rebuild`
  regenerates every derived table from them.

## Code

Code transcribes the method: names from the design document, the rule stated in each
public docstring, comments that cite the source. A failed collection is a status, not an
exception.

## Run

```text
py -3.14 -m pip install ./kernels        # Rust fusion kernel; fusion needs it
py -3.14 -m engine serve                 # API on :8790, collects every source by cadence
py -3.14 -m engine collect [--region R]  # one poll of every source
py -3.14 -m engine fuse                  # recompute events and entities
py -3.14 -m engine regions               # after editing any region.toml
git worktree add data data               # once: ./data = the cron's `data` branch
AISSTREAM_KEY=... py -3.14 -m engine serve   # AIS for the transit indicators (free key)
npm run dev                              # IRONSIGHT front end; set ENGINE_URL=http://127.0.0.1:8790
```

## Before a change is finished

```text
cargo fmt --check; cargo clippy --release -- -D warnings   # in kernels/, if changed
py -3.14 -m pip install ./kernels         # after a kernel change, before pytest
py -3.14 -m pytest -q
py -3.14 -m ruff check .
py -3.14 -m pip check
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
