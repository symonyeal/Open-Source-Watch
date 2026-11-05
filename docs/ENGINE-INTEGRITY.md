# Dataset generations and publication

The 2026-11-04 architecture review is addressed as follows:

| Finding | Boundary now enforced |
| --- | --- |
| OSW1: successful exit after failed extraction | Required-route acceptance and nonzero failure; publication gate |
| OSW2–OSW4: partial replacement, early acknowledgement, competing writers | Immutable generations, post-commit acknowledgement and OS writer lease |
| OSW5: raw-only replay claim | Preserved resource/translation inputs; explicit current-method rebuild and retained historical outputs; exact computational replay remains qualified below |
| OSW6–OSW7: defaulted required data and unchecked configuration | Required-column admission and production region validation |
| OSW8–OSW9: build-path dependency and hidden fallback | Explicit runtime resource root and recorded provenance required at snapshot acceptance |
| OSW10: repeated adapter dispatch | One registration per adapter for enumeration, parse, fetch and derive |
| OSW11: unbounded HTTP workers and streams | Request/stream permits and stream lifetime |

The data root is `OSW_DATA`, defaulting to `data` under `OSW_ROOT`. Without
`OSW_ROOT`, the engine searches the working directory's ancestors for `regions`
and `docs/labels`. A relocated binary needs its resource directory explicitly;
it no longer relies on the build machine's path.

## Commit boundary

`CURRENT.json` names immutable Parquet files for every table, a parent generation,
the last semantic materialization of each changed table and hashes of preserved
transformation inputs. A writer stages files and publishes this manifest with one
rename. The OS file lock `.writer.lock` rejects independent writers; threads in
one process serialize. A request pins one manifest for all its reads.

Raw reports, observations, tracks and collection runs commit as one poll batch.
Event/entity fusion commits together. Scheduled fusion and analysis commit
together. Rebuild and translation refresh publish all affected derived tables
together; rebuild replaces historical status/products with freshly computed ones.
Collection and scheduled analysis have different cadences: a common manifest does
not imply that every table was recomputed at the same instant. Product times and
the `materialized` map retain that distinction.

Seen-item acknowledgement follows commit. Deduplication includes region and raw
ID and reconciles against committed reports while holding the writer lease.
A failed write remains retryable. Missing referenced files and malformed required
columns fail the request; they do not silently become an empty dataset.

Existing roots migrate on their first write: the engine records a `legacy`
manifest before staging changes. Old files remain visible through that baseline
even if the first transformation aborts. Legacy inputs are unknown.

## Recovery and interpretation

Manifests remain in `.generations`; input bytes remain in `.inputs/<sha256>`.
Inputs cover engine source and Cargo files, region configuration/geography,
labels and the translation cache present after transformation. Restoring an old
manifest recovers its stored interpretation without rerunning translation.
Stop writers first, inspect the chosen manifest and its referenced files, and
atomically replace `CURRENT.json` with that manifest. Never edit a live Parquet file.

`osw rebuild` is a **current-method rebuild**, not an original-generation replay.
Reproducing an old computation requires restoring its resources and translation
cache and building its recorded source with a compatible toolchain. Toolchain,
OS and external provider availability are not frozen. Historical outputs are the
authoritative recovery artifact; exact cross-platform computational replay is
not guaranteed. Translation failures and source claims remain uncertainty, not
training labels.

Compaction changes visible file organization and retains all older files.
There is no automatic garbage collection: storage grows until a reviewed archival
policy is applied. Process-exit tests cover interrupted staging; sudden power loss
and network filesystems are outside the tested durability scope.

## Admission and publication

Required scalar columns reject missing/null data. Coordinates and numeric values
are validated before writing and after reading. Optional historical annotations
retain explicit migration defaults. Region loading validates indicator metrics,
windows, grades, source names and incompatible fields before collection starts.
One adapter registration owns source enumeration, parsing, request assembly and
derivation; its payload format stays with the adapter.

The HTTP server admits at most 32 active requests and four streams. Streams end
after five minutes. Excess admission returns 503. These bounds do not constitute
a throughput or denial-of-service benchmark.

Pages runs the quality gates before extraction. Snapshot staging requires every
retained region/route, including generated area products, to pass its shape
contract. Strict mode requires recorded engine provenance and one generation.
Accepted bytes are hashed and listed in `api/snapshot-manifest.json`, alongside
the public code and input-data Git revisions. These identify the build inputs
even when Pages' temporary generation files no longer exist. Failure
leaves diagnostic staging and exits nonzero without exposing a partial `api`
directory. A successful empty measurement is distinct from a failed request.

## Learning from evidence

Shipping references update from four preceding complete weeks, with windows and
sample counts exposed. Revision history distinguishes provider corrections from
new observations. Neither is a calibrated predictive model. Incident-rule changes
require reviewed labels and the existing validation/backtest workflow; repeated
reports do not become independent confirmations. Causes of shipping changes and
future alerts require independent evidence and held-out evaluation.
