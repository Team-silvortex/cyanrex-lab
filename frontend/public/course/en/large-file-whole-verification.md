# Current upload bytes whole file verification

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased trusted Unix preparation**, source **0.5.5**, **2026-10-11**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) checks the
current upload-bound chunks in order, including the whole-file length and SHA-256. It is a read-only
byte observation, not durable publication, an original write receipt, recovery or transfer authority.
The [local staging adapter](large-file-upload-staging.md) and [chunk journal](large-file-chunk-journal.md)
retain their independent contracts.

## Explicit read only operation

`FileUploadStaging::verify_file(upload_id, owner, account_id)` uses the existing trusted handles.
It captures the exact original reservation and database incarnation, fixes the source/root binding,
reads the existing private `u1` chunks, rechecks that original source, then rechecks the original
file snapshots before returning `UploadFileObservation`. No database lock spans file work.

The observation exposes only `upload_id`, `owner`, `account_id` and the complete immutable manifest.
It contains no bytes, path, descriptor, nonce, executable attempt or restored chunk receipt.
Trusted owner/account filtering is not authentication. A later change can invalidate the observation;
the database and filesystem checks are separate, not an atomic whole-file snapshot or permanent seal.

This operation neither consults nor changes journal status. Matching current bytes may be inspected
without an old in-memory receipt; success does not adopt those bytes as a past acknowledged write.
Unknown or Confirmed metadata alone is insufficient to establish integrity, publication or retry
permission. Missing files and failed checks do not release reservations or change any attempt.

## Ordered bytes and original observations

One worker reads chunks in manifest order with the existing **64 KiB** verification buffer.
Each chunk has an exact length, EOF and digest check; the ordered stream must also match the
whole-file SHA-256. Correct individual chunk digests do not excuse a wrong whole-file digest.
An empty upload requires the verified empty digest and a valid original private root, not a chunk file.

Access remains descriptor-relative under the originally opened **0700** root. Each opened chunk
must be an owned **0400**, single-link regular file with its exact expected length. The worker
compares its initial descriptor snapshot, the same descriptor after reading, and the visible entry.
After all bytes are read it revisits every original snapshot; it repeats those metadata checks after
the fresh original-source SQL observation. Same-byte replacement entries and observable changes
to earlier chunks are rejected, rather than treated as matching current paths.

Only a constant number of chunk descriptors are open at a time. At most **16,384** bounded names
and metadata snapshots survive the read pass; old receipts and their complete manifest copies are
not accumulated. There is no second assembled file, write, chmod, fsync, rename, deletion or cleanup.
Snapshot rechecks do not reread the entire content, and no descriptor is retained in the public result.
Same-UID/root and database administrators remain trusted; metadata checks are not WORM isolation.

The existing `u1` filename format stays unchanged. It hashes the complete manifest for each chunk,
so name generation still has quadratic manifest-hashing work at the protocol ceiling. Reusing stored
bounded names avoids repeating that work during snapshot rechecks; this is not a throughput claim.

## Shared capacity and cancellation

The existing local control permit covers the entire live caller. Both file passes use the original
`ChunkStaging` worker gate shared with legacy and upload-bound stage/verify operations. Each actual
queued or running job owns its worker permit until completion, even after its waiter is cancelled.
Separate handles still have additive budgets. No additional worker pool, global descriptor/memory
quota, kill mechanism, disk reservation, bandwidth limit or IO deadline is introduced.

`UploadFileVerificationError` distinguishes initial `CapacityExhausted`, exact `NotVisible`,
`Store(error)`, filesystem `Unavailable` and `Unconfirmed`. Once the initial byte pass has completed,
failure of the original-source or final-file checks returns `Unconfirmed` without an observation.
This is uncertainty about the read result, not a new write or a permission to repeat a write.
No attempt or journal state changes on success, error or cancellation; evidence remains untouched.

## Verification and remaining boundaries

`file_upload_whole_tdd` contains default no-database checks and explicit disposable PostgreSQL/file
cases. `scripts/test-file-upload-whole.sh` checks each exact ignored name before execution.
`fileUploadWhole` common guards check composition and inventory, not actual byte or SQL behavior.
Dated execution below records the verified runs and their limits.

The existing source and journal schemas, startup, AppState, Session commands, HTTP, OpenAPI/SDK,
browser, desktop and deployment composition remain unchanged. Separate [publication metadata](large-file-publication.md)
uses its own one-shot original-pin checks; this read-only operation stays independent. Restart recovery, physical disk policy, publisher/recipient
grants, central transport and safe client delivery remain separate work.

## Recorded execution · 2026-10-11

The baseline actually compiled and ran against a private, disposable PostgreSQL 16 database with
one real 17-byte upload-bound chunk. Red produced **0 passes / 1 expected behavior failure**:
the placeholder returned `Unavailable` after the old staging operation had succeeded. This was
not a missing-symbol compile failure. The temporary build had **4 library / 6 fixture warnings**.
After implementation the exact same baseline passed, without compiler warnings.

The final exact runner passed all **8 PostgreSQL/file cases**; every ignored name was listed and
matched before execution. Real patterned files cover **4 MiB + 17 bytes**, empty uploads and two
equal-length 4 MiB chunks with different contents. Positive controls precede wrong whole digests,
swapped chunks, missing/corrupt/short/long chunks, symlinks, hardlinks, directories, unsafe modes
and a FIFO with no writer. Empty uploads also reject an unsafe original root. Read-only pools with
repeatable-read defaults pass without changing their settings. A healthy namespace without a
chunk journal proves that installing a journal is not a prerequisite; live journal-status variations
were not exercised.

A private test-pool connection-return barrier proves the initial transaction is over before an
independent exclusive table lock. `pg_blocking_pids` then establishes the real fresh post-IO SQL wait.
Owner, account, manifest, installation and relation changes return `Unconfirmed`; expected complete
rows and OIDs are captured inside the injecting transaction **before COMMIT**, not after the result.
Same-byte inode substitution, content/mode/hardlink changes and a replacement root during this gap
also return no observation. Checks include the retained original files outside the substituted root.
Successful and rejected reads preserve exact database rows/OIDs and file bytes/identity/mode/mtime/ctime.

All **3 default no-database cases** and **3 private filesystem units** passed. Private units prove
actual queued/running whole-worker cancellation retains its slot, and a real same-length write to an
earlier chunk after a later chunk is scanned is rejected. The integration budget case separately
checks shared caller capacity and an existing legacy worker; it does not substitute for those units.
Adjacent exact runners passed **11 chunk-journal / 9 upload-staging / 12 upload-intent cases**.

The final backend gate passed **807 default Rust cases / 237 common checks**, **72 Rust result
blocks**, with **719 opt-in cases ignored**, zero failures and no compiler warnings. Ignored cases
are not passed SQL acceptance. The first common run stopped at **236 passes / 1 failure** because
the inherited production guard classified the new private test file as production. Fixed exact
test/worker inventories and the assertion that production rechecking precedes test-only helpers
preserve the production restrictions; the subsequent complete gate passed. Bilingual sources and
generated course copies are synchronized. All **104 prior dated project blocks** and the capability
tensor digest remain byte-for-byte unchanged; source **0.5.5** and API baseline **0.3.0** stay fixed.

This run did not accept publication/recovery, current-Session or HTTP authority, restart/receipt-loss
reconciliation, physical disk-full/fsync/power-loss behavior, maximum-size throughput/RSS, browser
or SDK delivery, desktop pairing or LAN deployment. Earlier index SQL and frontend/SDK build gates
were not rerun. No version upgrade, commit, push, release or deployment was performed.

The socket-only private database had no synthetic namespaces or other clients remaining and is now
stopped. Its **64 MiB** data directory, temporary Node caches and empty fixture directories were removed;
only execution logs remain under `/tmp/cyanrex-upload-whole-7PgwAybZ` (about **0.4 MiB**).
