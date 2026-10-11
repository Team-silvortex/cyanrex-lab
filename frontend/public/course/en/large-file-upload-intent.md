# Durable large-file upload intent and logical quotas

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased trusted preparation**, source **0.5.5**, **2026-10-11**.
This third step of [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) adds a
standalone PostgreSQL reservation ledger. It does not connect [private chunk staging](large-file-staging.md)
to that ledger, confirm uploaded chunks, authenticate a caller or enable network distribution.

## Explicit trusted API

[`FileUploadStore`](../../engine/src/services/file_upload_store/mod.rs) freezes an explicitly supplied
pool, workspace, random installation UUID, private namespace and `FileUploadLimits`. Construction
does no I/O. `install_empty_namespace` explicitly installs schema 1 in an empty private namespace;
existing relations/functions/types are refused. There is no startup installer or migration fallback.

| Operation | Confirmed meaning | Not supplied |
|---|---|---|
| `reserve(upload_id, owner, account_id, manifest)` | Immutable intent and its logical byte/count reservation committed together | File bytes, chunk receipt, current authority, physical disk allocation or publication |
| `get(upload_id, owner, account_id)` | Exact current reservation for that Principal and account generation, or absence | Historical commit provenance, whole-ledger health, resume, retry or recipient access |

The workspace must match the manifest, and the supplied Principal's authority must match the store.
`LegacyAccountId` binds the account generation; a different valid owner/generation reads as absent.
These are trusted filtering checks, **not Session authentication**. No untrusted adapter calls the store.
Private typed reservations expose metadata getters, not paths, bytes or restoration constructors.

## Reservation accounting

Limits are frozen at construction and persisted in the schema marker. Changing them is rejected,
not adopted as a configuration update. Total and per-Principal byte budgets may be zero; count
budgets are positive and capped at **256 intents**. The protocol ceiling is **16 TiB** of total
claimed lengths, not a claim that such disk space exists or is reserved by the operating system.

Accounting derives lengths from validated manifests, with checked arithmetic. Empty files still
consume one count. All retained records continue occupying their reservations indefinitely: no TTL,
account deletion, cancellation, quota release or cleanup is implemented. Per-Principal quotas sum
**all recorded account generations of the same Principal**. Recreating a same-name account obtains
a fresh Principal with a separate per-Principal allowance; old records still occupy workspace totals.
Duplicate `FileUploadId` or an already reserved exact `(FileId, FileVersionId)` conflicts before
quota admission; a different version of the same file can be reserved separately.

Each write validates the entire bounded ledger, including other owners. It reads at most 257 scalar
IDs to enforce the 256-record ceiling, then only one manifest body at a time. SQL bounds each body
before returning it. Strict typed parsing and canonical byte equality reject malformed, oversized,
foreign-workspace, duplicate or over-budget records rather than treating them as zero usage.
Only compact fingerprints and bounded accounting remain between reads. This is not a global CPU,
request-concurrency, memory, bandwidth or filesystem worker budget.

## Transactions, identity and uncertain outcomes

Writers explicitly use READ COMMITTED, serialize through both table locks and the marker row, and
retain locks until commit. Reads use a read-only snapshot without taking a marker write lock.
Exact ordinary-table shape, fixed namespace/search path, workspace, installation UUID, configured
limits and persisted namespace/table OIDs are checked. A same-named replacement is not adopted.
After insertion, exact readback and the original ledger fingerprints plus exactly one new record
are checked again; the tested immediate/deferred mutations are rejected. Database administrators
and arbitrary trigger code remain trusted; re-deferral of further events is not an accepted boundary. Local synchronous
commit is requested, without claiming tested power-loss durability or protection against a DBA.

An operation timeout or storage/commit error is **unconfirmed**, not proof of rollback. No automatic
retry, reservation release, adoption of staged bytes or cleanup follows it. A later matching read
reports current metadata only. PostgreSQL installation identity is separate from the filesystem
root's device/inode identity; neither substitutes for the other.

## Verification and next boundary

`file_upload_intent_tdd` contains default no-database admission tests and opt-in real PostgreSQL
tests. `scripts/test-file-upload-intent.sh` requires an explicitly disposable database and checks
each exact test name before execution. Its fault fixtures deliberately alter synthetic schemas and
records. Source guards check inventory and composition; they do not establish SQL behavior.
The [dated project record](project-status.md) reports actual execution and excluded acceptance.

SQL fixtures use claimed metadata and synthetic digests, **not transferred large-file bytes**.
The prior streaming/staging tests establish their own byte behavior; the two layers remain separate.
Receipt-loss/restart reconciliation, physical disk policy, current publisher/recipient authorization,
transport and safe client saves remain required. Existing staged entries
cannot be adopted merely because their filename or bytes match a reservation.
No live `AppState`, Session command, HTTP, OpenAPI/SDK, browser, desktop pairing or deployment changes;
Artifact's 1 MiB and text's 256 KiB limits and the source version remain unchanged.

The separate [upload-bound staging follow-up](large-file-upload-staging.md) now connects original
reservation/source pins to real bytes and local original-file receipts. It leaves this schema 1 ledger
unchanged and still adds no persistent chunk state, adoption, reconciliation or resume permission.
The independent [chunk journal](large-file-chunk-journal.md), [whole-byte verifier](large-file-whole-verification.md)
and [publication metadata](large-file-publication.md) supply their own explicit confirmation/checks,
not recovery or current authorization, without changing this reservation ledger.

## Recorded execution · 2026-10-11

The compiled reservation baseline first ran **0 passes / 1 expected behavior failure** against a
new private PostgreSQL 16 instance; it returned `StorageUnavailable`. That Red run had six temporary
unused-fixture warnings before the remaining test modules were mounted. Green passed the exact
baseline, **5 default cases** and **12 exact real PostgreSQL cases**, with no compiler warnings.
The SQL runner checks `--ignored --list` names before `--ignored --exact --nocapture` execution;
its matcher uses the same portable exact-line tool as existing CI runners.

SQL evidence covers fresh installation/reopen, full metadata and exact owner/account binding,
duplicate IDs/versions versus quota, same-file different-version admission, empty-file counts,
per-Principal cross-generation/global budgets, independently pooled writers under REPEATABLE READ
defaults, observed real lock waits and cancellation, malformed/oversized/foreign manifests and
over-budget foreign owners, 257-record refusal, table/schema pins, immediate/deferred mutations and
pre-commit storage errors. Faults are hit-proven, compare prior snapshots and include restored
healthy controls. The 64 GiB case stores **16,384 claimed chunk digests**, not 64 GiB of file data.
No lost-commit-acknowledgement, real disk allocation/full/fsync/power-loss, RSS/throughput, Session,
browser/SDK, TLS/LAN, deployment or desktop acceptance is claimed. Earlier index SQL was not rerun.
Two early source-inventory checks failed while modules were not yet wired and because a matcher
expectation was overly specific; the completed four-guard inventory passes.
The complete backend gate passes **795 default Rust tests / 225 common checks**, with **691 opt-in
cases ignored**, **69 Rust result blocks** and no compiler warnings. Tool fixtures are not deployment
acceptance. The private no-TCP database was stopped after all synthetic schemas were dropped;
47 MiB of regenerable cluster data was removed. Logs remain at `/tmp/cyanrex-upload-intent-7iIyc9OE`.
