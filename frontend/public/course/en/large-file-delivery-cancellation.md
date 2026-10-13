# Cooperative cancellation of file delivery waiting

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-13**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up adds an explicit
local cancellation request to [observed delivery](large-file-delivery-progress.md). It stops one
call's waiting and continuation when selected, not already-dispatched file or remote work. Cancelled
does not mean rollback, missing output, revoked access, safe deletion or permission to retry.

## One request handle and one consumed token

`FileDeliveryCancelHandle::channel()` returns a move-only handle and `FileDeliveryCancellation`.
The host consumes the cancellation token and existing `FileDeliveryObserver` in
`FileChunkClient::fetch_granted_and_save_controlled`, with the same token, grant/upload IDs, expected
version and explicit target as the old delivery methods. It returns `SavedFile` or the separate
`FileControlledSaveError`: `Cancelled` or `Delivery(FileGrantedSaveError)`. Old methods, errors,
transport settings and progress phases remain unchanged.

`cancel(self)` sends one unit request and consumes the handle. It is not a stop acknowledgement,
file-job abort, current grant revocation or remote cancellation command. Calling it after the call
ends is harmless. Dropping the handle without calling `cancel` **does not cancel delivery**; a closed
unsignalled channel disables the cancellation branch instead of becoming a request or busy loop.
The two capabilities have no public raw channel, cloning, credentials, path or serialization.

## Input priority and cooperative selection

The existing pure scope/token/cached same-source-target checks run first. Invalid input stays
`Delivery(InvalidInput)` even if cancellation was requested or flow capacity is full. With valid
input, an already-requested cancellation is selected before the inner core is polled: no flow
admission, progress event, HTTP or file dispatch occurs. Otherwise the original observed core owns
the same single flow and existing independent budgets; no second admission or new worker is added.

The outer wait selects the explicit cancellation branch first when both branches are ready at
that poll. This is **cooperative**, not an instantaneous dispatch fence. A single synchronous poll
may cross ready ACKs or begin later work before a concurrently sent request is observed. Cancellation
cannot interrupt synchronous hashing, filesystem calls or already-admitted remote work. A normal
success/error can win that race; the returned result, not the button press or send, is authoritative.
Already-selected results and returned acknowledgements are not revoked by late cancellation.

## Retained work and progress

When cancellation wins, the current live orchestration future is dropped and returns `Cancelled`.
It initiates no later steps after that selection, but an already-dispatched stage/assembly/save job
retains its original flow, worker, buffer/proof and target until completion. The cancellation token
and progress producer never enter those blocking jobs. No task, callback, retry, cleanup, credential
or network request is created by the controller itself.

Observation closes with its last waiter-observed snapshot; queued work can finish without advancing
that prefix or reporting `Complete`. A cancelled save may leave a partial or fully checked 0400
file even though no `SavedFile` was returned. Zero/zero empty counters, full staged percentages and
channel closure remain neither save receipts nor proof of job termination. Dropping only the progress
reader does not cancel the call. Caller drop remains the old cancellation-of-waiting behavior.

Existing chunks, private `f1` and uncertain outputs remain evidence, not recoverable attempts or
permission to overwrite/adopt/remove them. Current source authorization still occurs independently
for later admitted chunks. This local cancellation never modifies grants or reclaims disclosed bytes.

## Verification and remaining connections

Separate [complete HTTPS checks](large-file-delivery-https.md) exercise actual TLS waits and selected
saved output; they do not substitute for the actual disk-job or current-Source authorization tests.

`file_delivery_cancel_tdd` uses actual loopback HTTP and files; private lifecycle units distinguish
an actual cancellation result from job completion. `fileDeliveryCancel` guards validate composition
and inventory only. Existing explicit Source cases separately check authorization, unchanged server
snapshots and selected output. Actual execution results are recorded below.

UI cancellation controls, normal-app/HTTP/SDK mounting, Session issuance, persistent progress,
pause/restart/resume, physical quotas, job kill/rollback and deployed-LAN performance remain separate.
Versions/dependencies, published 0.5.7 evidence, **106 historical status blocks**, **50 existing
execution tails**, **8 protected files** and the dated capability tensor remain unchanged.

## Recorded execution — 2026-10-13

These are native local executions, not UI/deployment acceptance or a new release. Source stays
**0.5.7**, API **0.3.0**; no dependency, schema, OpenAPI, tensor score, commit, tag or push changed.

| Check | Actual result and scope |
| --- | --- |
| Red → Green | The compiled capacity-one, 4 MiB + 17 byte baseline first produced the inert implementation's `Delivery(Save(Unavailable))`: **0 passed / 1 expected failure**, after fixture cleanup, not a missing API or setup failure. The same unique-handle, actual metadata/two-chunk/assembly/owned-0755 save test then **passed**; independent files, EOF, chunk/whole SHA, inode/UID/0400/single-link, parent policy and exact HTTP sequence were checked |
| New default target | **10 passed**, no ignored cases, zero warnings: baseline 1, preflight 2, handle drop 1, explicit HTTP-wait cancellation 2, empty/short success 1, original error wrapping 1, independent pairs 1 and reader lifetime 1. Pre-request cancellation emits no event/HTTP/files; invalid scope/token/same-source target still wins. Actual held headers/partial JSON and later-chunk partial bytes return `Cancelled` and preserve only the prior ACK prefix. Late requests do not retract success/errors; a new call conflicts with retained bytes rather than adopting them |
| New private units | **5 passed** by exact inventory and execution, zero warnings. Four actual public-entry request cancellations cover held metadata, real chunk EOF with queued staging, empty queued assembly and an actual Created save. Held jobs retain flow/worker capacity; after release, orphan stage/assembly/save finishes actual checked bytes/0400 without later orchestration or `Complete`. The fifth tests channel closure/request and two-ready branch priority only; it is not filesystem evidence |
| Parallel test correction | The first full parallel run exposed a new test's false assumption that flow restoration implies simultaneous worker restoration: **379 passed / 1 failed / 114 ignored** in the library group. The leased reader drops its flow after real stage IO completes, just before the enclosing job drops its worker. Only the new test was corrected: a bounded real read-only verifier waits on `CapacityExhausted`, yields, and strictly rejects every other error or timeout. Held-job assertions and production remain unchanged. Five exact units and the complete parallel gate then passed. Initial unused test imports were also removed, not suppressed |
| Fresh optional authorization/TLS | **6 unique SQL cases + 1 existing TLS case passed**, zero warnings: delivery 2, granted metadata 2, whole download 1 and chunk client 1, plus exact IP-SAN/trusted-CA chunk staging. Delivery's six large/empty output variants exercise old private-0700, observed owned-0755 and controlled owned-0750 calls, with independent output and unchanged server snapshots. New controlled calls preserve exact nested errors after real post-metadata revocation and revoked empty metadata; unused-handle drop and late request are exercised. These Source cases establish authorization/wrapping, not explicit cancellation of SQL work or whole controlled TLS acceptance |
| Composition guards | **45 passed** focused checks. Their new five cancellation checks are static composition/inventory, not duplicate native behavior evidence |
| Full backend gate | Native `--backend-only`: **1,088 Rust tests passed / 854 optional ignored**, **96 result groups**, **341 common checks passed**, zero warnings/failures. All **15 existing compile-fail examples** passed unchanged. Optional SQL/TLS above was separately executed, not inferred from ignored rows |
| Preservation/cleanup | Existing **106 status blocks / 50 execution tails / 8 protected hashes** remain byte-exact. Course copies, version/OpenAPI/tooling and file-length/format checks pass; CI remains 600 lines and English status 1,999. The dedicated PostgreSQL 16 instance had only its exact socket/port 55457, no TCP listener, extra schemas or other client connections; it was stopped and only validated owned `data`/`socket` were removed: **40,812 KiB ≈ 40 MiB**, keeping logs |

Evidence: `/tmp/cyanrex-file-delivery-cancel-eov87TS9/{red,green,defaults,private,sql,focused-guards,full-backend,post-evidence}.log`;
initial warning/parallel-failure logs are retained separately. The temporary database is not retained.
Final checks after recording rerun common/format guards, course synchronization, history protection
and unchanged HEAD/origin/version tag identity; they are not another backend or SQL acceptance run.

No frontend/SDK suite, deployed LAN, 64 GiB/RSS/throughput, physical quotas, abrupt-power-loss or
hard OS-job cancellation was tested here. UI/save/cancel controls and normal-app/Session mounting
remain absent. A cancellation result is neither proof of missing output nor authorization to retry,
overwrite, adopt or delete an uncertain file.
