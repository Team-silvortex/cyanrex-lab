# Upload identity bound private chunk staging

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased Unix trusted preparation**, source **0.5.5**, **2026-10-11**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) connects the
[immutable upload ledger](large-file-upload-intent.md) to [private chunk staging](large-file-staging.md)
through a caller-owned attempt. Its local acknowledgement is **not a persistent chunk journal**,
current-Session authorization, resume permission or a delivered file.

## Preparing and staging one chunk

[`FileUploadStaging`](../../engine/src/services/file_upload_staging/mod.rs) explicitly receives a
trusted upload store, an already opened private `ChunkStaging` and a control budget from **1 to 16**.
Their fixed workspaces must match. Construction neither installs SQL nor creates/chmods directories.
`prepare(upload_id, owner, account_id, index)` captures an exact reservation snapshot in a
REPEATABLE READ, READ ONLY transaction and freezes its complete metadata, original schema pins
and chunk coordinate. It performs no filesystem IO or current-root health check; only cached
root identity contributes to the binding. Later stage checks use fresh READ COMMITTED reads.

| Step | State and result | Boundary |
|---|---|---|
| Prepare | Returns `Ready` with the exact owner/account-generation metadata | Trusted filtering only; not authentication or an upload receipt |
| Admit `stage_chunk` | Sets `Unknown` before its first await | Foreign issuer/non-Ready attempts are rejected; control-budget refusal leaves Ready |
| Recheck original reservation | Fresh short read-only transaction | No file dispatch on this check's failure; the attempt remains Unknown |
| Stage bytes | Exclusive upload-bound file, exact digest/length, seal, sync and original-inode readback | No SQL transaction spans this file work; uncertain output is retained |
| Recheck ledger and original file | Original namespace/table identity and full metadata, then original read-only descriptor and current entry | Failures after staging return `Unconfirmed`, never a receipt |
| Acknowledge | Returns an opaque local receipt and sets `Confirmed` | In-memory acknowledgement only; not durable progress or whole-file publication |

Only the issuing adapter or its clones can use an attempt. Neither attempts nor receipts offer
serialization, cloning or restoration constructors. A consumed Unknown/Confirmed attempt cannot
execute again, including after error or cancellation. A new independently prepared attempt is a
separate trusted call, **not** a recovered attempt or a process-independent prohibition on retry.

## Identity and file provenance

The fixed private `u1` filename binds upload ID, exact Principal/account generation, the complete
typed manifest, chunk index, configured installation/namespace/table identity and the originally
opened filesystem root's device/inode. Display names never select a path. No per-attempt random
filename allows another attempt to bypass `O_EXCL`: every existing bound destination conflicts.
Legacy `v1` staging names stay unchanged and cannot supply a bound receipt.

The store captures an opaque reservation pin rather than treating two matching ordinary `get`
results as incarnation proof. Later checks use **READ COMMITTED, READ ONLY** before any selection,
so a deliberate table-lock wait cannot preserve a pre-wait ledger snapshot. They compare the
original namespace/search path/table OIDs and complete selected metadata. No locks span file IO.
These are observed checks, not an atomic filesystem/database transaction or protection against
arbitrary changes by a trusted DBA, the same UID or root after a check.
Later preparation captures another observed reservation snapshot; a trusted writer changing its metadata can
produce another binding. Logical reservation quotas are not a physical limit on retained files.

The local receipt privately retains the original read-only file descriptor and verified metadata.
Final verification checks that object and the visible private entry, rejecting even a same-byte
replacement. Retaining the descriptor prevents the original inode from being freed and reused
during the ledger check. Each retained receipt owns a descriptor until dropped; this is not a global
descriptor budget. No descriptor/path or constructor is exposed, and a fresh path-based observation
cannot manufacture a historical receipt. Permissions 0400 are not WORM isolation from the same UID/root.

## Capacity and errors

Clones share the adapter's fail-fast control budget. It limits live prepare/stage waiters, not
blocking jobs after caller cancellation, returned receipts, all handles or process memory.
Original staging workers separately retain their shared permit while queued/running, including
cancelled callers. Legacy and bound stage/verify jobs use the same worker budget on that handle.
Supplied readers can block indefinitely; this adds no job kill, bandwidth throttling or physical-disk
reservation/quota. Staging still writes actual files.

A control-budget rejection keeps Ready. Once admitted, a failed pre-file ledger check returns
`Store(error)` with Unknown; a stage worker's capacity refusal, conflict or failed root precheck can
report its known error but still leaves Unknown. Post-creation uncertainty and any failed post-stage
ledger/original-file check are `Unconfirmed`. No automatic retry, adoption, reservation release,
state reset, deletion or quota rollback follows any result. Dropping a receipt does not remove bytes.

## Verification and remaining work

Default no-database cases and opt-in `file_upload_staging_tdd` cases distinguish input admission,
real bytes, identity drift and worker cancellation. `scripts/test-file-upload-staging.sh` requires
an explicitly disposable database and checks every exact test name before running it. Source guards
check composition and inventory, not SQL or byte behavior. Dated execution below records actual runs.

This original local API keeps upload schema 1 and its installer unchanged and adds no chunk-status
SQL, filesystem migration, live `AppState`, Session command, HTTP, SDK/OpenAPI, browser, desktop or deployment wiring.
Receipt-loss/restart reconciliation, physical disk policy, current publisher/recipient authorization,
transport and safe client delivery remain separate gates.

The subsequent independent [write-ahead journal](large-file-chunk-journal.md) now wraps new attempts
with persistent coordinates. This original local API stays independent; its receipts are not journal
records, and the new wrapper adds neither restored attempts nor transport.

The subsequent [whole-file verification](large-file-whole-verification.md) reads current bound
bytes independently of these original receipts and journal status. It does not publish or recover.
Separate [publication metadata](large-file-publication.md) combines exact confirmed journal metadata
and fresh whole bytes without changing these original local receipts or restoring attempts.

## Recorded execution · 2026-10-11

The compiled baseline first ran **0 passes / 1 expected behavior failure** (`Unconfirmed`) against
an explicitly disposable private PostgreSQL 16 database. Seven temporary unused-fixture warnings
appeared before the remaining modules were mounted. Green passed the exact baseline and all
**9 exact PostgreSQL/file cases** without compiler warnings; the runner checked each ignored name
before executing it. The adjacent upload-intent runner also passed all **12 exact SQL cases**.

The new cases use actual files and patterned bytes across **4 MiB + 17 bytes**, rather than metadata
only. Real reader barriers and `pg_blocking_pids` establish post-write SQL waits before changing
owner/account, manifest, installation or table identity. A same-byte replacement inode is rejected.
Running-worker cancellation preserves Unknown and the shared actual worker permit; control-budget
refusal instead leaves Ready. The existing default legacy queued-worker unit covers its own path;
this follow-up does not add a bound queued-blocking-thread unit.

The final backend gate passed **798 default Rust cases / 229 common checks**, with **700 opt-in
cases ignored**, **70 Rust result blocks** and no compiler warnings. All **102 prior dated project
blocks** and the capability-tensor digest remain byte-for-byte unchanged. Bilingual documentation
and generated course copies distinguish local acknowledgements from durable progress.

The first full gate stopped at two CI inventory guards because the newly added workflow step
made its file 603 lines. Removing three blank lines restored the existing 600-line limit; no guard
was weakened. No network endpoint, current-Session acceptance, persistent chunk-state/restart or
lost-commit-ack recovery, real disk-full/fsync/power-loss fault, throughput/RSS benchmark,
browser/SDK saving, TLS/LAN deployment or desktop interoperability was accepted. Earlier index
SQL suites were not rerun. No version, commit, push, release or deployment change was made.

The private PostgreSQL 16 instance exposed no TCP listener, left zero synthetic upload schemas and
zero other client connections, and was stopped. Its **47 MiB** cluster and **3 MiB** regenerable Node
cache were removed; exclusive empty fixture directories were cleaned. Logs remain under
`/tmp/cyanrex-upload-bound-wQ9u2Poq`. No live data or existing build cache was removed.
