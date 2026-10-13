# Authorized download and private saving

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-13**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up joins the
[exact manifest client](large-file-manifest-client.md), ordered private assembly and
[explicit private save](large-file-save.md) in one trusted-host call. It does not mount the application,
issue Sessions, choose a filename for the user, implement a UI save dialog or resume old work.

## Explicit target and one flow

`FileChunkClient::fetch_granted_and_save(token, grant_id, upload_id, &expected_file, &target)` returns
the existing move-only `SavedFile` with `file() -> &FileVersionRef`. `FileSaveTarget` is opened by the
host before dispatch: the default owned 0700 private parent or an explicit
[user-directory choice](large-file-user-save.md) using `open_user_directory` for owned 0700/0750/0755,
and an independently selected UTF-8 basename. Remote display names never become destinations;
shared-writable directories remain rejected. Construction verifies/pins the then-current parent; it reserves no output and
does not prove that later dispatch/save will see a healthy path or that remote access is authorized.

Expected workspace, canonical non-nil Session token shape and cached target-root distinction from
staging are checked before admission or HTTP. The call acquires the existing clone-shared whole-file
flow gate once (default four, trusted range 1–16). It does not reacquire through the old public
download entry, add a delivery semaphore or reserve a staging worker throughout network waits.

The parent flow remains held across these dependent stages:

1. Fetch the complete exact expected manifest through the fixed metadata endpoint and existing
   independent manifest gate. Strict headers, framed EOF/JSON, bounds and full version match apply.
2. Request each chunk only after its actual private stage ACK. The old receiver and staging budgets
   are unchanged, and each Source chunk checks current recipient authority independently.
3. Assemble the original private whole file in the same staging root; its private proof pins the
   original read-only file/root and complete manifest binding, not another equal-byte inode.
4. Borrow that original ACK to exclusively copy to the host-selected target through the existing
   three-pass source/copy/output verification, synchronization and original-inode readback.

Even an empty file requires metadata GET, then actual empty assembly and selected-file saving; no
chunk request is invented. The previous supplied-manifest and granted-assembly APIs retain their
behavior and error variants. The standalone save API still owns no whole-download flow lease.

## Cancellation and failures

The actual queued/running file job owns a shared parent flow reference as well as its original
worker/proof/target resources. Dropping the waiter does not release that flow until the job finishes.
Cancellation at metadata/HTTP waits starts no later chunk, assembly or save. Once a file job has
been dispatched, cancelling the wait returns no ACK but the original job may complete. No new
background orchestration, retry, recovery, adoption or cleanup starts.

`FileGrantedSaveError` is a new sanitized enum: `InvalidInput`, `CapacityExhausted`,
`Download(FileGrantedDownloadError)` and `Save(FileSaveError)`. The top-level capacity error is the
whole-flow gate; existing metadata/chunk/assembly and save errors retain their stage and meaning.
An inner gate failure returns the parent admission without automatic waiting or retry.

The selected parent's current path, permissions and existing entries are checked by the original
save job, not by a new early filesystem probe. A conflict or drift may therefore be reported after
chunks and `f1` were already created. Earlier failures leave the target untouched and preserve any
source prefix. Post-create uncertainty preserves output evidence. Repeating the whole call does
not skip retained chunks or adopt `f1`/saved files; it can fail before reaching the target. A new
filename is not resumption. Only a surviving original assembly ACK permits standalone explicit copies.

Success acknowledges this local selected copy, not continuous remote permission, current remote
principal provenance, public publication or browser delivery. Expiry/revocation never erases already
disclosed local bytes. The final save sends no new GET or authorization query. Network deadlines
remain per request, not one hard deadline for a file or cancellation of OS/CPU work.

The optional [live observer](large-file-delivery-progress.md) uses the same operation with one latest
per-call phase/chunk-ACK snapshot. Full staging is not saved output, and closure does not stop actual
jobs or provide a receipt. The old method reports no progress and retains its existing behavior.

An optional [explicit cancellation handle](large-file-delivery-cancellation.md) now wraps this same
observed core. It requests stopping this call's waiting at a cooperative poll boundary; dropping the
handle alone is not cancellation, and dispatched file jobs can still finish without a later ACK.

## Verification and remaining connections

The separate [complete HTTPS checks](large-file-delivery-https.md) exercise the controlled path with
native disposable peers, not a current-Source authorization or deployed-LAN proof.

`file_delivery_tdd` uses real loopback HTTP and private files. Private lifecycle tests exercise actual
job ownership, not just a sleeping future. `scripts/test-file-delivery.sh` separately selects exact
ignored current-Source SQL/network cases; `fileDelivery` checks source composition and inventory only.
Runtime results are recorded after execution, not inferred from earlier separate-step passes.

Normal AppState/Session issuance, public HTTP/SDK/UI, file picking/shared-writable-directory policy, UI/durable progress,
restart/resume, physical disk quotas, peer transfer and LAN deployment stay separate. The extra copy
and original ACK descriptor costs from private saving still apply; no new throughput/RSS or 64 GiB
payload acceptance is implied. Versions/dependencies, published 0.5.7 evidence, **106 historical status
blocks**, **44 existing execution tails** and the dated capability tensor are preserved.

## Recorded execution 2026-10-13

The inert new entry compiled against actual loopback HTTP, private roots and a **4 MiB + 17**
manifest, then failed after fixture cleanup: **0 passed / 1 expected failed**, `Save(Unavailable)`,
zero warnings. This was behavioral Red, not a missing API or setup failure. Green reran that exact
capacity-one baseline: **1 passed**, zero warnings, with real metadata/two chunks/original assembly
and the independently observed selected new file. Repeated baselines do not add unique acceptance cases.

| Fresh selection | Actual result and scope |
|---|---|
| Default delivery target | **10 passed / 2 opt-in ignored**, zero warnings; baseline 1, behavior 2, failures 3, admission 2, cancellation 2. Actual empty/17-byte/cross-chunk output, independent EOF/chunk/whole SHA/pattern/inode/uid/0400/single-link checks, default-four clone/old-entry shared flows and independent clients, nested metadata capacity, pure scope/token/same-root priority, first-error stopping, late target conflicts/drift, repeat chunk/assembly Conflict, real header/partial-body cancellation and retained prefix |
| New private lifecycle units | **4 passed**, zero warnings; actual held metadata cancellation, chunk-zero real EOF with queued stage ownership, public delivery's queued save cancellation after original ACK drop, and public delivery's real Created phase cancellation through same-inode 0400/readback completion. Flow and worker remain full while the old job is alive; unrelated metadata/control gates are free at save, and separate-root healthy controls complete afterwards |
| New explicit Source runner | **2 distinct SQL cases passed**, zero warnings; actual Session publication/recipient grants for 4 MiB + 17 and empty → metadata/chunks → original assembly → selected independent file, unchanged server snapshots and exact request order; real grant revocation after metadata before chunk delegation gives 200/404, revoked empty metadata gives 404, neither creates a target |
| Adjacent explicit runners | **2 old manifest-client SQL + 1 supplied-manifest download SQL + 1 single-chunk SQL + 1 existing OpenSSL TLS passed**, zero warnings; with the new two SQL cases this is **6 unique fresh SQL + 1 existing TLS**, not new metadata/delivery TLS or deployed-LAN acceptance |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **1043 Rust passed / 854 opt-in ignored / 93 result groups**, **326 common passed**, zero failures/warnings; the new defaults/private units and the unchanged **15 compile-fail contracts** are included, not extra totals |

Independent production/lifecycle, Source-test and bilingual reviews found no remaining defect.
Before SQL execution, the new revoked-empty delta oracle's exact grant seed was corrected from 1
to 3; this was a fixture correction, not a production change or behavioral Red. Documentation
distinguishes parent verification at target construction from later path health. Static guards now
check present actual preflight calls, not imports, and distinguish production `run` from test
`run_with`; the final native focused composition/inventory selection passed **30**. Those guards
do not establish real authorization or disk writes. CI adds the exact new runner on its existing
explicit file-transfer command line; no workflow line-limit exemption or database fallback is added.

Logs stay in `/tmp/cyanrex-file-delivery-FOeVCxkH`: `red.log`, `green-baseline.log`,
`green-default.log`, `green-delivery-units.log`, `source-green.log`,
`source-manifest-adjacent.log`, `source-ordered-adjacent.log`, `source-chunk-tls-adjacent.log`,
`guards-final.log` and `full-backend.log`. The isolated PostgreSQL 16 instance had only a Unix socket,
no TCP, and zero extra schemas/connections at the final audit. After stopping, only its validated
owned data/socket directories were removed, freeing **40,748 KiB (about 40 MiB)**. Test data can be
reconstructed; execution logs are retained.

No version/dependency/schema change, commit, tag or push occurred. No new frontend/SDK/security audit,
save dialog/ordinary-directory policy, progress/resume/restart, disk-full/fsync/power-loss acceptance,
64 GiB real payload, throughput/RSS or live/LAN deployment verification is implied. Separate opt-in
SQL/TLS runs are not default gate execution, and old dated evidence has not been refreshed.
