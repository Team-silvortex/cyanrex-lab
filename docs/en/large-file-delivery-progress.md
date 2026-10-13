# Live file delivery progress observation

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-13**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up adds bounded local
observation to [authorized delivery](large-file-delivery-client.md), not persisted progress or a new
transport. A trusted host can display the current phase and this call's acknowledged staging prefix.
It still needs the original call result to handle success or failure; snapshots grant no resume,
retry, cleanup, file access or remote authority.

## One call and one latest snapshot

`FileDeliveryObserver::channel()` returns a move-only producer and `FileDeliveryProgress` reader.
The host consumes the producer in `FileChunkClient::fetch_granted_and_save_observed`, with the same
Session token, grant/upload IDs, expected version and explicit target as the old delivery method.
The result remains `Result<SavedFile, FileGrantedSaveError>`. The old method uses the same core
without observation; old error variants, admission and transport behavior remain unchanged.

The channel starts with no snapshot. `latest()` copies the current `Option<FileDeliverySnapshot>`;
`changed().await` returns a copied latest snapshot, or `None` after the producer ends and unseen
changes have been consumed. No watch borrow escapes either method. A single overwriteable slot
holds only counts and phase; slow readers can skip intermediate updates. Unread/dropped readers
do not introduce an await for consumption, queue, callback, worker, listener or extra flow gate.
Observation is per-call, not shared across client clones or reusable by another delivery.

`FileDeliverySnapshot` has private fields and getters `phase()`, `staged_chunks()`, `staged_bytes()`,
`total_bytes()` and `total_chunks()`. `total_bytes` is the host's expected version length, not a remote
byte observation. The optional total chunk count appears only after a complete matching manifest.
Snapshots carry no file IDs, path, filename, digest, Session token, origin, error text or payload.
Neither producer/reader nor snapshots are a public HTTP/SDK request, serialized checkpoint or receipt.

## ACK boundaries and phases

| `FileDeliveryPhase` | Meaning and counter boundary |
|---|---|
| `FetchingManifest` | Pure input checks and the original whole-flow admission passed. No manifest/chunk ACK; staged counts are zero, total chunks unknown |
| `StagingChunks` | The complete exact manifest was checked. Counts advance only after each actual private chunk staging ACK, not HTTP headers, body bytes or network EOF alone |
| `Assembling` | The ordered chunk ACK prefix is complete and original assembly is about to be awaited. All staged bytes are not a selected-file save |
| `Saving` | Original private whole-file assembly returned its ACK. The selected new-file save is about to be awaited; this phase is not save completion |
| `Complete` | The original selected-file save returned `SavedFile`. The snapshot is still metadata, not that move-only acknowledgement or continuous authority |

Empty files still require metadata GET, actual empty assembly and selected saving. Zero of zero
chunks/bytes does not imply `Complete`. Counters describe ACKs observed by this one live waiter,
not current file health, permanent integrity, durable upload status or a recoverable bitmap.

## Cancellation and failures

Errors or waiter drop end observation without manufacturing `Complete`. The last snapshot can remain
readable after closure; it does not establish that a dispatched job stopped, failed, rolled back or
finished. Actual queued/running chunk, assembly and save jobs retain their original worker/flow and
file resources until completion, but hold no progress producer and emit no later observation.
Dropping the reader does not cancel delivery. With this original entry the host separately drops/stops
waiting on the call; the optional [explicit cancellation handle](large-file-delivery-cancellation.md)
requests that boundary while preserving a typed result. Neither entry hard-cancels OS file work.

The same first-error stopping, exclusive entries, original-inode proof and target policy apply.
Retained chunks, `f1` and uncertain target files are not adopted. An observed full staged prefix or
closed channel cannot authorize another call, recovery, retry, overwrite or cleanup. No new GET or
authorization lookup is added for reporting, and each later chunk remains independently authorized.

## Verification and remaining connections

`file_delivery_progress_tdd` uses actual loopback HTTP and files; private lifecycle tests separately
exercise queued/running job ownership. `fileDeliveryProgress` guards check composition/inventory only.
The existing explicit delivery Source cases independently verify current authorization and actual
selected output. Actual execution results are recorded below, not inferred from prior slices.

UI rendering/file picking, HTTP/SDK/live mounting, Session issuance, durable progress/restart/resume,
physical quota, peer transfer and deployed-LAN performance remain separate. Versions/dependencies,
published 0.5.7 evidence, **106 historical status blocks**, **48 existing execution tails**, **8 protected
files** and the dated capability tensor remain unchanged.

## Recorded execution 2026-10-13

The inert observed entry compiled against an actual **4 MiB + 17** loopback delivery, capacity one,
private staging and an owned **0755** selected target. After fixture cleanup it failed with
`Save(Unavailable)`: **0 passed / 1 expected failed**, zero warnings. This was behavioral Red, not
missing API or setup failure. Green reran the exact baseline: **1 passed**, zero warnings, with
independent chunk/whole SHA, EOF, pattern, inode/UID/0400/single-link checks and final `Complete`.
Repeated baselines do not add unique acceptance cases.

| Fresh selection | Actual result and scope |
|---|---|
| New default target | **10 passed**, no ignored cases or warnings: baseline 1, real response boundaries 2, preflight 1, failures 3, empty 1, readers 1, independent channels 1. Real held manifest headers/partial JSON keep unknown chunks; later chunk headers/partial bytes keep only the prior actual ACK prefix. Unpolled/invalid/full-flow calls close without events or extra GET; pure scope/token/same-root rejection wins over capacity. First metadata/chunk error stops; full staged bytes with a bad whole hash stop at `Assembling`, target Conflict stops at `Saving`, empty 0/0 only completes after actual save. Slow/dropped readers do not prevent output or retain gates; old prefix files are not adopted by a new call |
| New private units | **5 passed**, zero warnings: four actual public lifecycle cases cover held metadata cancellation, chunk-zero real EOF with queued stage ownership, queued empty assembly and Created-phase save cancellation. Observation closes while original jobs keep flow/worker; orphan work can finish actual 0400/hash/readback without publishing a later ACK or `Complete`. The fifth only checks copied/coalesced watch values, final unseen delivery and reader drop; it is type/channel behavior, not physical-save evidence |
| Extended delivery Source | **2 existing SQL cases passed**, zero warnings. Six independent positive source/output pairs deliver large and empty files through an old private-0700 control and observed user-0755/user-0750 calls. Actual saved output, exact HTTP order, final counters/closure and full unchanged server snapshots are independently checked. Real post-manifest chunk revocation closes at `StagingChunks` with zero ACKs and 200/404; revoked empty metadata closes at `FetchingManifest`, total 0/chunks unknown. Neither creates an assembly or target |
| Adjacent explicit runners | **4 SQL + 1 existing OpenSSL TLS passed**, zero warnings: manifest client 2, supplied-manifest download 1 and single-chunk client 1. With extended delivery this is **6 unique fresh SQL + 1 existing TLS**, not new observed-delivery TLS or deployed-LAN acceptance |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **1073 Rust passed / 854 opt-in ignored / 95 result groups**, **336 common passed**, zero failures/warnings. The new defaults/private units and unchanged **15 compile-fail contracts** are included, not additional totals |

Independent production/lifecycle, default-fixture, Source and current-document reviews found no
blocking defect. One Source assertion was narrowed: an unconnected observer's empty view is an old-API
control, not independent proof that the old core emits nothing. The actual old `None` composition and
closed-view isolation are checked separately. The focused native composition/inventory selection
passed **40**, and the post-format core selection passed **15**; static guards are not runtime IO or
authorization evidence. Existing SQL runner inventory and the 600-line CI workflow remain unchanged.

Logs remain in `/tmp/cyanrex-file-delivery-progress-8dwqZdaJ`: `red.log`, `green-baseline.log`,
`green-defaults.log`, `green-private.log`, `sql-and-tls.log`, `focused-guards.log`,
`guards-after-fmt.log`, `common-guards.log` and `full-backend.log`, with database audit/stop/cleanup
logs. The disposable PostgreSQL 16 instance used only a Unix socket, no TCP, and had zero extra
schemas/connections at final audit. After stopping, only its validated owned data/socket directories
were removed, freeing **40,612 KiB (about 40 MiB)**. Test data are reconstructible; logs are retained.

No version/dependency/schema change, commit, tag or push occurred. No UI rendering/file picking,
frontend/SDK/security audit, public Session/HTTP/SDK path, persisted progress/recovery/resume, pause,
hard OS-work cancellation, disk-full/fsync/power-loss acceptance, 64 GiB payload or throughput/RSS/
deployed-LAN verification is implied. Separate opt-in SQL/TLS runs are not default gate execution.
The **106 historical status blocks / 48 existing execution tails / 8 protected files** remain
byte-for-byte unchanged, including the dated tensor and published release evidence.
