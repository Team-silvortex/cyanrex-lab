# Durable file chunk write ahead journal

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased trusted Unix preparation**, source **0.5.5**, **2026-10-11**.
This separate follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
records an acknowledged Unknown chunk before file creation and records confirmation after local
byte verification. It does not recover an attempt, authorize a retry, publish a file or connect transport.
The [upload ledger](large-file-upload-intent.md) and [local staging adapter](large-file-upload-staging.md)
retain their existing independent contracts.

## Explicit composition

`FileUploadChunkJournal::new(pool, workspace, installation_id, namespace, max_entries)` freezes
trusted configuration without connecting or installing. The installation ID must be an RFC 4122
version 4 UUID; the dedicated namespace name follows the upload store's restricted name rules.
The global entry budget is **1–16,384** chunk coordinates, not a per-upload limit or disk reservation.
An explicit `install_empty_namespace` accepts only a fresh empty namespace and installs the new
journal's own schema 1. It does not add tables to the old upload schema 1 or migrate either store.

`JournaledFileUploadStaging::new(local, journal)` takes an existing `FileUploadStaging` and journal.
Their workspaces must match and their configured namespace names must differ. It shares the local
adapter's control gate and existing filesystem worker gate, rather than adding a second work budget.
Its own issuer identity admits only its attempts or clones; an independently constructed wrapper
cannot execute another wrapper's attempt. Construction performs no SQL or filesystem work.

`prepare(upload_id, owner, account_id, index)` captures the original upload and journal pins and
exact chunk metadata. It does not create a journal entry, validate current filesystem health or
restore an earlier attempt. Owners and account generations remain trusted attribution, not credentials.

## Committed states and local acknowledgements

| Boundary | Recorded result | In-memory result |
|---|---|---|
| Ready attempt, control admission refused | No write | Ready unchanged |
| Admitted stage begins | No row is implied yet | Unknown before the first await |
| Original source check and Unknown transaction acknowledged | One fixed Unknown coordinate | Still Unknown; recheck the original source before any file dispatch |
| Exclusive stage and original-file checks | Unknown remains | No successful receipt yet |
| Exact confirmation transaction acknowledged | Confirmed record | Recheck source and retained original file again before returning a receipt |
| Final observations pass | Confirmed record | Confirmed and a local original-file receipt |

The unique coordinate is **upload ID + chunk index** in the journal's fixed workspace. Source/root
keys and nonce are immutable binding fields; status changes only from Unknown to Confirmed. None
is an extra coordinate. Every occupied
coordinate conflicts, including identical metadata, Unknown and Confirmed. A fresh wrapper or
changed source/root cannot bypass that conflict in the same journal. No automatic release, reset,
downgrade, overwrite, adoption, deletion or retry follows an error or cancellation.
Conflict and entry budgets belong to this journal installation. A new journal does not import old
states or grant continuation; unchanged source/root destinations still conflict at the filesystem,
while changed configuration is not a recovery protocol.

An unacknowledged Unknown transaction never permits file dispatch, even if a later metadata read
finds its row. Cancellation releases the waiting control permit but does not stop the blocking worker;
its worker permit lasts until the actual queued/running job finishes, and any output is retained.
Failure before creation can retain a committed Unknown record with
no file. Failure after the final journal commit can leave a Confirmed record while the caller remains
Unknown and receives `Unconfirmed`. These stores and the filesystem are separate observations,
not an atomic cross-storage transaction. Same-UID/root and database administrators remain trusted.

## Compact records and original pins

Each row contains trusted owner/account generation, upload/index, complete-source and opened-root
binding keys, total/chunk lengths, chunk digest, a private version 4 nonce and recorded status.
The full manifest is not duplicated per chunk; the source key commits to the original reservation's
complete typed manifest and upload-store installation/namespace/table incarnation, not a database
or cluster identity. The nonce is created internally and is not
available through a metadata read or imported checkpoint.

Namespace/search-path/table OIDs, configured installation and persisted budget are rechecked against
the original journal pin. Writers use READ COMMITTED and take both journal tables in a fixed
SHARE ROW EXCLUSIVE order, keeping read-only policy intact. They validate the bounded full ledger,
exact chunk geometry and same-upload binding consistency, then compare the complete expected ledger
after deferred constraint work before requesting a synchronous commit. Oversized fixed-size fields
are rejected through bounded SQL projections before decoding.

Compact-ledger checks do not independently reconstruct every foreign manifest or prove the history
of every Confirmed row. Source checks remain in short separate transactions; no SQL lock spans
filesystem IO. Original-file verification retains the original read-only descriptor and rejects
replacement entries, rather than adopting matching bytes at a path. A metadata row cannot recreate
that private receipt.

## Read only observation

`get(upload_id, owner, account_id, index)` returns a selected record snapshot or no visible record,
with exact trusted owner/account filtering. `UploadChunkRecord` exposes chunk length/digest and
`PersistentChunkState::{Unknown, Confirmed}`, not keys, nonce, path, a runnable attempt or receipt.
The read validates selected metadata and schema, not the whole ledger or current source/file bytes.
Absence is not rollback proof, and recorded Confirmed is not current integrity or resume permission.
Even all chunks recorded Confirmed do not establish the ordered whole-file digest or publication.

## Verification and remaining boundaries

`file_upload_chunk_journal_tdd` has no-database admission tests and opt-in PostgreSQL/file tests.
`scripts/test-file-upload-chunk-journal.sh` requires an explicitly disposable database and checks
each exact ignored name before execution. Source guards check inventory and composition, not SQL
behavior. Dated execution below records actual tests and excluded acceptance.

This chunk-journal API adds no current-Session command, HTTP listener, SDK/OpenAPI, browser save, startup installation,
automatic migration, network endpoint, publication/recipient grant, physical disk quota or cleanup
API. The old local path does not require this journal. Recovery needs separately reviewed
source/root incarnation and byte reconciliation; observed status alone does not grant execution.

The subsequent [whole-file verifier](large-file-whole-verification.md) checks current bytes without
consulting this journal. Its result does not turn Unknown/Confirmed status into historical receipts.
The separate [publication preparation](large-file-publication.md) instead requires complete original
Confirmed target rows and fresh whole bytes. Its create-only metadata does not supply recovery,
current recipient authorization or transport, and does not change this chunk write-ahead protocol.

## Recorded execution · 2026-10-11

The actual TDD Red was the fresh-install acknowledgement assertion: the target compiled, the
stub returned `StorageUnavailable`, and the assertion failed after fixture cleanup. It was not a
missing-symbol failure or a Red for the entire staging chain. That temporary build reported two
library and eighteen fixture warnings. The implemented installer passed the same assertion.

The independent journal target then passed **3 no-database tests** and **11 exact opt-in
PostgreSQL tests**. The latter include an actual deferred INSERT/advisory-lock wait with no file
entry and no reader call before Unknown acknowledgement, independently visible committed Unknown
at the real input barrier, and legal source drift after that acknowledgement with no file dispatch.
Running-worker cancellation retains its permit until completion. Deferred UPDATE/source-lock
barriers distinguish a committed Confirmed row from a caller remaining Unknown after late source
or same-byte replacement-inode drift. Other cases cover retained exact files, compact corruption,
schema/query pins, read-only observation, occupied coordinates and serialized capacity admission
with poisoned repeatable-read/synchronous-commit defaults.

Adjacent upload-bound staging **9/9** and upload-intent ledger **12/12** real PostgreSQL cases
also passed. The isolated `scripts/quality-gate.sh --backend-only`, without a database URL, passed
**233 common checks** and **801 Rust tests across 71 result blocks**, with **zero warnings**.
Its **711 ignored cases** remain opt-in inventory, not additional executed passes. These runs do
not establish network lost-ack behavior, real fsync/disk-full/power-loss handling, restart recovery,
whole-file finalization, live Session/HTTP wiring, browser/desktop transport or privileged acceptance.

All SQL runs used the owned socket-only disposable PostgreSQL 16 instance and fresh private source
and journal namespaces. Those fixture namespaces/files were removed and no other database clients
remained; the server was stopped and its approximately 64 MiB data directory removed. Logs remain
under `/tmp/cyanrex-chunk-journal-oDq9MmuD/`: `red-install.log`, `green-install.log`,
`default-journal.log`, `postgres-journal.log`, `postgres-upload-staging.log`,
`postgres-upload-intent.log` and `backend-quality.log`.

Both language course copies were synchronized. All **104** pre-existing dated project-status
blocks and `docs/platform-capability-tensor.json` stayed byte-for-byte unchanged. Source remains
**0.5.5**, API **0.3.0**; no commit, push, tag or release was performed for this slice.
