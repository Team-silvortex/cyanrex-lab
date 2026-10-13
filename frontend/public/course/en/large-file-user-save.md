# Explicit saving to owned user directories

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-13**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up adds an explicit
host choice of an ordinary owned directory to [local saving](large-file-save.md) and the
[authorized delivery call](large-file-delivery-client.md). It does not expose filesystem paths in
HTTP/SDK, mount the privileged Engine, implement a file dialog or resume retained transfers.

## Separate target policy

`FileSaveTarget::open_user_directory(parent, leaf)` accepts an existing ordinary directory owned by
the current effective UID, with exact permission bits **0700**, **0750** or **0755**. Group/other write,
special bits, other modes, symlinks and missing/non-directory parents are rejected without repair.
The old `FileSaveTarget::open` remains private-0700-only. `ChunkStaging` and Artifact/private-root
policies are unchanged; no new root silently falls back to the ordinary-directory policy.

Both target constructors use the same protected-ancestor, descriptor-relative no-follow traversal:
an absolute bounded path, root/current-UID ancestors without group/other write, and the existing
root-owned sticky ancestor exception. That exception never makes a sticky final target acceptable.
The host chooses the basename independently of remote metadata; the same **1–255 UTF-8 byte** limit
and control/separator/dot-name refusals apply. Neither constructor creates a parent, changes modes,
reserves a leaf or opens an output entry.

The ordinary target retains the original directory descriptor and a security snapshot of
**device/inode, UID, GID and complete permission bits**. Each later verification compares the original
FD and a fresh no-follow walk with that original snapshot. A 0755→0750 change still fails even though
both modes are individually allowed. Restoring the original policy and original inode can make a
surviving target healthy again; a different matching directory is never adopted.

Directory mtime/ctime/length/link count are not frozen: the operation's own creation legitimately
changes directory metadata, and separate explicit leaves in the same parent may be copied concurrently.
This is a then-observed identity/policy check, not proof that no intervening mutation occurred.

## Original checked output and resource ownership

The target uses the existing original-assembly proof, staging worker gate and exclusive save core.
Source checks precede `O_EXCL | O_NOFOLLOW` creation with requested **0600**. Checked streaming copy,
final **0400**, file/parent synchronization and original-inode actual output SHA/EOF readback remain
unchanged. Existing entries conflict without being written, renamed, adopted or removed. Source
chunks and private `f1` remain in place; saving still adds a full copy.

`ChunkStaging::save_assembled_file` and `FileChunkClient::fetch_granted_and_save` accept the same opaque
target type. Same-source-root refusal remains cached input validation before worker/whole-flow
admission. No new semaphore, remote request, retry or progress journal is added. Actual queued/running
jobs retain the target descriptor, original proof, worker and optional parent flow after waiter drop.
Cancellation returns no ACK but the original job may finish; no new orchestration begins.

Invalid basenames return `InvalidInput`; unavailable parents return `Unavailable`. Parent drift
detected by a pre-creation verification returns `Unavailable` without creating a target. Races after
that check can still return `Unconfirmed`; final verification drift also returns `Unconfirmed` and
preserves any partial/complete output. These errors grant no rollback,
cleanup, overwrite, adoption or automatic retry. The old error enums and move-only `SavedFile`
metadata getter are unchanged.

## Privacy and remaining boundaries

A 0750/0755 parent is **not private staging**. Depending on ancestor permissions, other local users
may list names or observe directory/file metadata. Output Unix mode remains owner-read-only; this
does not hide filenames or isolate same-UID/root mutation, nonstandard ACLs, mounts or network filesystems.
No cross-user access test or permanent physical sealing is implied by mode checks.

The constructor is a trusted local-host choice, not an untrusted request DTO, workspace grant or
permission to write an arbitrary privileged Engine path. Ordinary shared-writable folders are still
unsupported. UI confirmation/file picking, public Session issuance/HTTP/SDK, normal-app mounting,
progress/resume/restart, physical disk quotas, peer transfer and deployment acceptance stay separate.

Optional [live delivery observation](large-file-delivery-progress.md) reports phases and chunk ACKs
for an explicit delivery using this target; the directory constructor/standalone save adds no observer.
Full staged counts and channel closure are not a selected-save receipt or job-termination evidence.

## Verification

`file_user_save_tdd` uses actual files and loopback delivery; private lifecycle/phase tests cover real
job cancellation and post-create policy drift. `fileUserSave` guards validate composition and inventory
only. The existing explicit delivery SQL bridge separately covers current authorization and selected
output; actual runtime results are recorded below. Versions/dependencies, published 0.5.7
evidence, **106 historical status blocks**, **46 existing execution tails** and the dated capability
tensor remain unchanged.

## Recorded execution 2026-10-13

The inert new constructor compiled against a real **4 MiB + 17** original assembled ACK and an
explicitly owned **0755** target. It failed after fixture cleanup with `Unavailable`: **0 passed /
1 expected failed**, zero warnings. This was behavioral Red, not missing API or fixture setup.
Green reran that exact baseline: **1 passed**, zero warnings. Repeated baselines do not add unique cases.

| Fresh selection | Actual result and scope |
|---|---|
| New default target | **10 passed**, no ignored cases or warnings: baseline 1, selection 3, drift 2, existing entries 1, concurrent reuse 1, real loopback 2. Exact 0700/0750/0755 ownership and original GID observations, empty/17-byte/cross-chunk output, cloned targets, 255-byte UTF-8 basename, malformed paths/names, no-follow/unsafe ancestors, forbidden modes, safe-to-safe drift, root replacement/restoration, seven existing-entry types including FIFO, independent whole/chunk SHA/EOF/inodes/0400, metadata cancellation and pure admission priority. Old private constructors/staging still reject 0750/0755 without repair |
| New private units | **5 passed**, zero warnings: four actual public lifecycle/phase cases cover queued save cancellation after original ACK drop, Created-phase safe-mode/parent-inode drift with retained evidence, two overlapping Created jobs in one parent, and public delivery cancellation retaining flow/worker/original inode through 0400/readback. The fifth only changes a private cached GID pin and verifies rejection while the actual directory GID is unchanged; it is not a `chgrp` or cross-user test |
| Extended delivery Source | **2 existing SQL cases passed**, zero warnings, not new case inventory. Positive execution independently delivers large and empty files into private 0700, user 0755 and user 0750 parents: six source/output pairs, exact metadata/chunk order, original private assembly and independent target digest/EOF checks, unchanged server and parent security snapshots. Real post-manifest chunk revocation and revoked-empty metadata use user 0755/0750 targets and create neither assembly nor output |
| Adjacent explicit execution | **8 SQL + 1 existing OpenSSL TLS passed**, zero warnings: manifest client 2, supplied-manifest download 1, single-chunk client 1, old private upload staging 2 and publication 2. Together with extended delivery this is **10 unique fresh SQL + 1 existing TLS**, not new delivery TLS or deployed LAN acceptance |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **1058 Rust passed / 854 opt-in ignored / 94 result groups**, **331 common passed**, zero failures/warnings. New defaults/private units and the unchanged **15 compile-fail contracts** are included in these totals, not additional counts |

Independent production/lifecycle, authorized-Source and current-document reviews found no blocking
defect. The shared path walk leaves the old owned-0700 terminal separate and unchanged. Documentation
clarifies observed parent verification versus races after that check, and ordinary-directory name/
metadata visibility versus private staging. The focused native composition/inventory selection passed
**35**; these static guards do not establish authorization or physical IO. The existing delivery runner
and CI inventory remain unchanged, without a workflow line-limit exemption or database fallback.

Logs remain in `/tmp/cyanrex-user-file-save-e2BCzZX5`: `red.log`, `green-baseline.log`,
`user-defaults.log`, `user-private.log`, `sql-and-tls.log`, `focused-guards.log` and
`full-backend.log`, with database audit/stop/cleanup logs. The disposable PostgreSQL 16 instance used
only a Unix socket, no TCP, and had zero extra schemas/connections at final audit. After stopping,
only its validated owned data/socket directories were removed, freeing **41,132 KiB (about 40 MiB)**.
These test data are reconstructible; execution logs are retained.

No version/dependency/schema change, commit, tag or push occurred. No new frontend/SDK/security audit,
GUI/file dialog, live Session issuer, HTTP/SDK path input, progress/resume/restart, shared-writable folder,
actual ownership/group change, ACL/mount/NFS isolation, disk-full/fsync/power-loss acceptance, 64 GiB
payload or throughput/RSS/deployed-LAN result is implied. Separate opt-in SQL/TLS execution is not the
default gate, and the **106 historical status blocks / 46 existing execution tails / 8 protected files**
remain byte-for-byte unchanged.
