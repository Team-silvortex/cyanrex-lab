# Ordered private download client

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-12**.
This follow-up is outside the published 0.5.7 tag. It joins the existing
[controlled chunk client](large-file-chunk-client.md), [receiver](large-file-chunk-receiver.md) and
[private assembly](large-file-chunk-assembly.md) under [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2).
It is an explicit internal call, not a mounted downloader or desktop distribution product.

## Ordered operation and local acknowledgement

`services::file_chunk_client::FileChunkClient::fetch_and_assemble(token, grant_id, upload_id, &manifest)`
uses the original client's fixed trusted origin and the **same original `ChunkStaging` handle/root**
as its receiver. It checks workspace and canonical non-nil 36-byte Session UUID before whole-flow
admission or HTTP, including empty files. Typed grant/upload selectors retain the existing contract.
The caller supplies the expected typed manifest; this operation does not fetch or authenticate it.

Each admitted flow requests index 0, waits for verified EOF/hash and actual private staging ACK,
then requests index 1 and so on. There is at most **one active chunk per flow**, no spawned network
continuation and no automatic retry, skip or adoption. Only after every selected chunk succeeds does
the original staging handle assemble the real whole-file output. Capacity one does not self-deadlock.
The existing `fetch_and_stage` call remains independent of whole-flow admission.

An empty manifest sends **zero HTTP requests** and still performs actual local empty-file assembly.
A syntactically valid token is not remotely checked on that path: empty success proves neither a
current Session nor recipient authority. Nonempty requests reuse the exact seven selectors, sensitive
per-call cookie, marker, header policy and controlled transport; no new protocol or credentials appear.

Success returns the existing move-only `AssembledFile`, exposing only `file() -> &FileVersionRef`.
Original owned 0700 root, private 0400 single-link files, fixed manifest-bound `v1`/`f1` names,
exclusive creation, actual ordered/whole digest, original-inode readback and final input/root checks
are unchanged. No user-selected filename, path, public bytes/descriptor or remote delivery receipt is returned.

## Independent limits and cancellation

Existing `FileChunkClientConfig.capacity` controls a new **independent** clone-shared whole-flow gate,
default **4**, trusted range **1–16**, as well as the existing separate single-chunk gates. No new
configuration field or global scheduler is introduced. Independent constructed clients remain additive.
Whole-flow admission does **not** reserve receiver, staging-worker, disk, bandwidth or SQL capacity.
Unrelated consumers and simultaneous flows can still fail fast at those separate existing gates.

The live flow and its current dispatched save/assembly job share one `Arc<OwnedSemaphorePermit>`.
During body collection the parent owns it; after file dispatch the actual queued/running job also owns
it. Dropping the async waiter cannot free the whole-flow slot while that job remains active. The
private cursor drops bytes, receiver permit and request permit before its final whole-flow reference;
assembly retains the reference inside the real blocking closure, not merely around its JoinHandle.
Existing public receiver/staging calls pass `None` and keep their admission semantics.

Cancellation during headers/body creates no unverified current chunk, preserves earlier confirmed
chunks and releases local flow capacity. After file dispatch, the current chunk or whole output may
still finish, but **no next HTTP chunk is requested**. Cancellation is not remote/OS hard termination,
rollback, reusable progress or permission to delete/retry. There is no background continuation.

Header, body idle and body total deadlines remain **per chunk**. `total_timeout` is not a whole-file
deadline; file work cannot be hard-killed. The receiver buffers at most one selected **4 MiB** payload
per active flow, and assembly uses the existing **64 KiB** streaming buffer and bounded
**16,384 chunks / 64 GiB** metadata contract. Chunks and the extra whole copy remain on disk.
Repeated existing per-chunk manifest cloning/serialization is not a linear-metadata performance
claim. These structural bounds are not measured RSS, throughput, global quotas or 64 GiB acceptance.

## Errors, authorization and retained evidence

`FileDownloadError` distinguishes `InvalidInput`, whole-flow `CapacityExhausted`,
`Chunk { index, error: FileChunkClientError }` and `Assembly(ChunkStagingError)`.
The index identifies the selected failed step; underlying errors remain static/sanitized and do not
include credentials, origin, paths or raw transport diagnostics. The first failure stops the flow.

Earlier saved chunks, unconfirmed files and existing outputs are never deleted, overwritten, adopted
or renamed. Repeating a partially/fully completed operation encounters the existing chunk/output
conflict rules; it is not resume. Assembly failure can retain all chunks and an unconfirmed output.
No hidden cleanup, retry, progress journal or recovered acknowledgement is added.

The server still authorizes **each** chunk using its existing current-recipient command. Revocation
between requests can refuse the next chunk without rolling back disclosed/saved earlier bytes.
This client adds no final remote authorization recheck, authenticated manifest provenance, continuous
grant guarantee or original per-chunk receipt-identity continuity across later local assembly.
Final success acknowledges the actual local whole bytes observed by assembly, not remote delivery.
HTTP message EOF remains library-framed EOF, not raw TCP suffix/connection EOF.

The separate [manifest HTTP adapter](large-file-manifest-http.md) now offers authorized metadata GET;
this client method still does not fetch it or replace each later chunk's current authorization.
A separate [exact manifest client entry](large-file-manifest-client.md) now fetches metadata before
the same download core, without changing this supplied-manifest method or its zero-HTTP empty case.
The original acknowledged output can separately enter [explicit private saving](large-file-save.md)
to a trusted host-selected new file. This standalone follow-up does not extend the download flow
lease or add a user-facing save dialog, remote authority or resume.

The new [authorized delivery call](large-file-delivery-client.md) joins these stages in one explicit operation.
One original whole-flow lease remains owned by actual chunk/assembly/save work after cancellation;
first failure stops with no retry/adoption, and UI saving, resume and normal mounting remain separate.

The separate [owned user-directory target](large-file-user-save.md) explicitly accepts 0700/0750/0755
without relaxing private staging. Original directory UID/GID/mode checks and checked saving remain;
file picking, writable shared folders, resume and live mounting are separate.

No normal AppState/listener/route, Session issuer, OpenAPI/SDK/UI, schema installer,
migration, live-auth cutover, user-selected final save, restart/resume, peer pairing or LAN deployment
acceptance is added. Versions and dependencies remain unchanged.

## Verification entry points

`file_download_client_tdd` has 11 default real-loopback/private-file cases. Four private units cover
the public queued-save and empty queued-assembly paths plus actual running save/assembly lease
boundaries; the latter are internal job tests, not public HTTP running-job hook claims.
The explicit `scripts/test-file-download-client.sh` runs one ignored current-Source SQL bridge with
both cross-chunk success and later actual grant revocation. Existing TLS and receiver/assembly
regressions remain separate selections. `fileDownloadClient` guards prove composition/inventory only.

Current bilingual notes/course copies describe this new edge. The **106 historical status blocks**,
**36 old execution tails**, published 0.5.7 validation and dated capability tensor stay unchanged;
this is not a new maturity score or relabelled release acceptance.

## Recorded execution 2026-10-12

The inert API scaffold compiled against the actual **4 MiB + 17** loopback/private-file baseline.
Behavioral Red was **0 passed / 1 expected failed**, `Assembly(Unavailable)`, zero warnings: no
requests or whole output existed. The 11-case native Green included that same baseline with actual
ordered indexes 0/1, independently reopened chunk/whole EOF/SHA/pattern, single-link 0400 files and
the original owned 0700 root. Fixture cleanup preceded its final behavior assertion.

| Fresh selection | Actual result and scope |
|---|---|
| Download default target | **11 passed**, zero failures/warnings; real two-chunk success, empty zero-HTTP output, shared default-four admission/input precedence, later metadata/hash/timeout failure, root drift, existing chunk/output conflict, wrong actual whole digest and header/partial-body cancellation |
| Cancellation unit selection | **26 passed / 7 opt-in ignored**, zero warnings; includes **4 new** queued/running whole-flow lease cases, alongside existing cancellation regressions; no extra acceptance is inferred from filtered cases |
| Explicit current-Source download bridge | **1 passed**, zero warnings; actual Published **4 MiB + 17** and healthy **200/200**, then fresh client root with first chunk ACK, exact trusted test-admin revoke and actual **200/404**, retained original first chunk, no whole output or next request; exact Source/Registry/upload/journal/publication/file snapshots unchanged except the explicitly verified recipient revoke |
| Adjacent SQL/TLS runners | **1 old client + 3 receiver + 8 whole-upload + 10 current-recipient SQL passed**, plus **1 OpenSSL CA/SAN passed**, zero warnings; with the new bridge this is **23 unique SQL cases + 1 TLS case**, not duplicate counts from its later rerun |
| Final full backend gate | `scripts/quality-gate.sh --backend-only`: **951 Rust passed / 842 opt-in ignored / 89 result groups**, **306 common passed**, zero warnings/failures; includes all 11 new defaults and 4 new units, not 15 additional passes |

The Source bridge initially had an `E0277` compile error from trying to Debug-print opaque
`AssembledFile`; the diagnostic was narrowed to the error and compiled successfully. This is not
behavioral Red. Review added explicit response-status and graceful server-join checks, reran that
same SQL bridge, then reran the complete backend gate. Native composition selection passed **25**;
the restricted file-level-only output was not used as actual per-case evidence.

No new frontend/SDK/security-audit, real LAN deployment, user-selected save/resume, disk-full/fsync/
power-loss fault, 64 GiB payload or measured RSS/throughput acceptance was performed. Passing ignored
inventory listing is not execution, and the 23 explicit SQL cases are not part of default 951.
The optional TLS control is the existing client fixture, not a new whole-download HTTPS deployment.

All source editors froze before serialized root-owned Cargo execution; production owner/ancestor
checks stayed intact. Final disposable PostgreSQL 16 audit confirmed the exact owned private data
directory, no TCP, **zero fixture schemas and zero other clients**. After stopping, only validated
`data`/`socket` were removed: **77124 KiB (about 75 MiB)** of reconstructible test data. Logs remain
at `/tmp/cyanrex-file-download-flow-S0WGh0hc`; project/deployment data was not removed.
Versions/dependencies, normal startup, live API/SDK/UI, the published 0.5.7 reports, **106 old status
blocks**, **36 old execution tails** and the capability tensor remain unchanged. No commit/push.
