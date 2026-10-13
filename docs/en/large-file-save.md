# Explicit private file saving

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-12**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up connects an
original acknowledged [private assembly](large-file-chunk-assembly.md) to a new file explicitly
chosen by a trusted host. It does not mount the normal application or implement a desktop/browser
save dialog, public HTTP/SDK API, progress journal, resume, remote permission check or automatic cleanup.

## Trusted selection and original object binding

`FileSaveTarget::open(parent, leaf)` opens only an existing privately owned **0700** directory through
the existing protected-ancestor, descriptor-relative no-follow policy. It does not create/chmod the
parent, reserve a name or open an output. The UTF-8 basename is **1–255 bytes**, not empty, `.` or `..`,
and contains no slash, backslash, NUL or control character. The host supplies it independently;
`manifest.display_name()` never becomes a default destination. This default private constructor
still rejects ordinary 0755 directories. The separate [user-directory choice](large-file-user-save.md)
adds explicit `open_user_directory` for owned 0700/0750/0755 without relaxing private staging.

Target clones share their original parent descriptor, path and identity. There are no public fields,
serialization or path/descriptor getters. The selected parent must differ from the staging root.
The target is a local host capability, not a remotely supplied request or workspace permission.

`ChunkStaging::save_assembled_file(&manifest, &assembled, &target)` borrows the original
`AssembledFile` and returns move-only `SavedFile` with `file() -> &FileVersionRef` metadata only.
Neither acknowledgement has a public constructor, Clone, serialization, bytes/path getter or
writable descriptor. Existing download/assembly public methods and error variants remain compatible.

Assembly now internally retains the **original read-only `f1` descriptor**, original Snapshot,
complete typed-manifest fingerprint and original root identity/descriptor, only after its real final
checks succeed. It does not manufacture a proof by reopening a name from old version metadata.
The writer stays live until the original read-only inode has been opened and checked, then closes
before actual readback. Equal replacement bytes, the same version coordinates or another root do
not replace that original object. This follow-up extends the private lifetime, not remote authority.

Scope, full version, complete manifest fingerprint and cached original root binding are checked
before worker admission. Fingerprinting is bounded synchronous metadata work, not payload IO.
The new API does not adopt existing `f1` files as receipts. A same-byte different-inode replacement,
unlink, hardlink, mode/length/content drift or substituted root fails closed.

## Checked copy and exclusive output

The admitted job rechecks both original roots, the retained source/root descriptors and the visible
original `f1` snapshot. Once those checks pass, any existing target entry conflicts, including
empty/valid files, directory, FIFO, hardlink and dangling symlink. It never opens one for writing.
Actual ordered chunk/whole SHA-256 and exact EOF verification of the original source precede creation.
Roots and source are rechecked before exclusive `O_EXCL | O_NOFOLLOW` creation with requested **0600**.

A second source pass copies checked bytes through the existing **64 KiB** streaming core. Source
reads use the original shared descriptor with an invocation-owned positional (`pread`) cursor;
concurrent explicit copies from the same borrowed acknowledgement cannot mix cursor positions.
The original source is checked again. Output becomes **0400**, synchronizes file and parent, and
opens/checks the original read-only output inode while its writer is still live. The writer then
closes before a third actual payload pass verifies output chunks, whole digest and EOF.
Original source/output descriptors, visible names, snapshots and both roots are checked before ACK.

Success means this selected new file's actual synchronization/readback and then-observed original
path binding. It is not atomic public publication, power-loss acceptance, WORM isolation against
same-UID/root mutation, executable installation or a continuing remote grant. Previously disclosed
local bytes do not disappear when a remote grant expires; this trusted local operation makes no GET.

## Admission and uncertain outcomes

Saving shares the original **1–16** staging workers with stage/verify/assembly operations. Fail-fast
admission precedes cloning metadata and one blocking dispatch. The actual queued/running job owns
the worker, original source proof and target parent through waiter or acknowledgement cancellation.
Cancelling a waiter returns no ACK and does not stop an already queued/running file job, which may
finish. No new orchestration continuation, network request, retry, recovery or output adoption is added.
This is not a hard disk/CPU deadline or a separate whole-download flow lease.

`FileSaveError` has static `InvalidInput`, `CapacityExhausted`, `Conflict`, `Unavailable` and
`Unconfirmed`. Source/root checks may return `Unavailable` before checking target existence;
existence is checked before the full source payload pass. Pre-creation failures never create or alter
a target. With the earlier checks satisfied, existing entries return `Conflict`. Other creation
errors, post-creation failures and lost job results are conservatively `Unconfirmed`. Partial or
complete unconfirmed files remain (initial requested 0600, final 0400). Explicit repeated calls to
the same target, with valid original bindings, conflict without changing it; another can be copied anew
from the surviving original proof. No matching retained output authorizes recovery or overwrite.

Each live assembly acknowledgement retains two private descriptors, released with its last internal
proof reference, not a staging worker. A target holds its parent descriptor; jobs use a constant
descriptor set and bounded manifest metadata. Retained acknowledgements/independent staging handles
have no global descriptor/RSS quota. The **16,384-chunk / 64 GiB** declaration limits and three payload
passes are structural bounds, not measured throughput or physical disk-space reservation.
Source chunks and private `f1` stay in place; selected saving adds another full copy without deleting them.

## Verification and remaining connections

The new [authorized delivery call](large-file-delivery-client.md) can now compose exact metadata,
ordered chunks, original assembly and this selected save under one real-job-held whole-flow lease.
Standalone saving remains a local primitive with no GET or parent download lease; UI/resume are separate.

`file_save_tdd` checks real private files, original-object refusal, selected-name exclusivity and
independent EOF/SHA/pattern/inode/permissions. Private work/phase tests cover actual queued/running
ownership and deterministic source/output/root drift. `fileSave` guards check composition/inventory,
not real saving. The existing explicit manifest-client SQL bridge separately connects authorized
network download with selected local output. Runtime results are recorded below after execution.

The API remains trusted-host-only and unmounted. UI confirmation/file picking, shared-writable
directory policy, public Session issuance, SDK/OpenAPI, resume/progress/restart, peer transfer, migration and
LAN deployment remain separate. Versions/dependencies, published 0.5.7 validation, **106 historical
status blocks**, **42 existing execution tails** and the dated capability tensor remain unchanged.

## Recorded execution 2026-10-12

The inert public save entry first compiled against a real **4 MiB + 17** staged/assembled source and
an explicitly selected private parent, then failed after fixture cleanup: **0 passed / 1 expected
failed**, `Unavailable`, zero warnings. This is behavioral Red, not a missing symbol or setup failure.
Green reran the exact baseline: **1 passed**, zero warnings. The first private staging run passed
24 but reported one unused test import; removing that import and rerunning gave **24 passed**, zero
warnings. The first focused Node run was 24/1 because its old production inventory included a new
test-only `read_to_end`; the exact test-file/dispatcher inventory was updated, not the payload policy.

| Fresh selection | Actual result and scope |
|---|---|
| Default private save target | **12 passed**, no ignored cases/warnings; real empty, 17-byte and 4 MiB + 17 outputs; independent chunks/whole SHA/EOF/pattern/inode/0400/single-link observation; full manifest/version/root refusal, UTF-8 byte-bounded host names, unsafe parent refusal without repair, original source replacement and 10 drift branches, root restoration, seven existing entry types, repeat Conflict and explicit new-name copies, inert unpolled save and concurrently polled borrowed-ACK public calls |
| Private staging regression selection | **24 passed**, zero warnings, including **9 new cases**: one actual original-FD cursor observation, four queued/running/concurrent ownership cases and four deterministic source/target/root/hash drift cases; the real public queued save retains proof/work after dropping both waiter and ACK; private two-Created barriers establish overlapping jobs, not simultaneous physical reads |
| Extended existing manifest-client SQL bridge | **2 unique cases passed**, zero warnings; actual authorized publication → metadata → ordered chunks → original `f1` → selected independent new file for 4 MiB + 17 and empty, with no extra HTTP or server mutations; separate metadata-to-chunk revocation and revoked-empty refusal remain actual 200/404 and 404 with no local assembly |
| Adjacent explicit runners | **1 supplied-manifest download SQL + 1 single-chunk SQL + 1 existing OpenSSL TLS passed**, zero warnings; together with the two extended cases this is **4 unique fresh SQL + 1 existing TLS**, not new metadata/save TLS or LAN deployment acceptance |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **1029 Rust passed / 852 opt-in ignored / 92 result groups**, **321 common passed**, zero failures/warnings; defaults/private units and the **15 compile-fail contracts**, including 3 new SavedFile contracts, are already included, not extra totals |

Independent production, Source-test and bilingual reviews found no remaining production defect.
Documentation was narrowed to the actual cancellation and source-check/Conflict ordering. Static
guards were also strengthened to require present actual calls, not an imported name or a missing
`indexOf` operand; the final native focused composition/inventory selection passed **25**. These
guards do not constitute actual saving or authorization evidence. Repeated baseline/unit selections
are not additional unique full-gate cases; opt-in cases remain separately executed evidence.

Logs remain under `/tmp/cyanrex-file-save-2B4TEVUr`: `red.log`, `green-baseline.log`,
`green-default.log`, `green-staging-units-clean.log`, `source-green.log`,
`source-ordered-adjacent.log`, `source-chunk-tls-adjacent.log`, `guards-final.log` and
`full-backend.log`. The isolated PostgreSQL 16 instance used only its Unix socket (no TCP); audited
zero extra schemas/connections before stopping. Only its validated owned data/socket directories
were removed, freeing **40,300 KiB (about 39 MiB)**; logs stay and test data can be reconstructed.

No version/dependency/schema change, commit, tag or push was performed. No new frontend/SDK/security
audit, UI file dialog/ordinary-directory policy, resume/restart, disk-full/fsync/power-loss acceptance,
64 GiB payload, throughput/RSS or live/LAN deployment verification is implied. Every retained ACK
still costs two descriptors and an explicitly saved file adds a full disk copy.
