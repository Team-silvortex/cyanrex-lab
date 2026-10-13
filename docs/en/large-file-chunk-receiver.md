# Verified HTTP chunk reception and private staging

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
This is a separate client-side primitive for an **already supplied response**, not an active HTTP
client, LAN deployment or download UI. It follows [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
and the unmounted [binary chunk adapter](large-file-chunk-http.md).

## Composition and result

`services::file_chunk_receiver::FileChunkReceiver::new(staging)` takes an already-open trusted Unix
`ChunkStaging`. Construction caches its workspace and creates only a local control budget; it does
not create directories, change permissions, query SQL or start network requests.

`receive_and_stage(&manifest, index, response)` accepts an `http::Response<B>` where
`B: http_body::Body<Data = axum::body::Bytes> + Send + 'static`. `Unpin` is not required. The caller
supplies the exact typed manifest, selected index and private destination. Response metadata cannot
choose a workspace, filesystem root, output filename, Source hidden key or authorization subject.

The path is:

1. Match the cached workspace and selected manifest chunk; validate the response head.
2. Fail-fast admit the receiver control slot and collect a private bounded buffer.
3. Verify exact length, explicit body EOF and the selected SHA-256 before any file dispatch.
4. Transfer the validated buffer and its slot into the existing `ChunkStaging::stage_chunk` job.
5. Return opaque `ReceivedFileChunk` metadata only after **this call's** local staging success.

The result exposes only `file()`, `chunk_index()`, `byte_length()` and `chunk_sha256()`. It has no
public constructor, Clone/serialization, bytes, pathname or hidden-key accessor. It acknowledges a
single local staging operation, not continuous path health, current remote authority, a remote
receipt, a complete download, reconstruction, a resume token or a durable confirmation journal.

## Strict response head

Only status **200** is accepted. The receiver counts all header entries including duplicates, with
at most **64 entries** and **8192 aggregate name/value bytes**. Unknown bounded headers are ignored;
they carry no path or authority meaning. These twelve fields must occur exactly once and match:

| Header | Required value |
|---|---|
| `content-length` | Canonical decimal selected chunk length, not whole-file size |
| `content-type` | `application/octet-stream` |
| `content-disposition` | `attachment; filename="chunk.bin"` |
| `x-cyanrex-file-transfer-version` | `1` |
| `x-cyanrex-file-id` | Exact expected file UUID |
| `x-cyanrex-file-version-id` | Exact expected version UUID |
| `x-cyanrex-chunk-index` | Canonical decimal selected index |
| `x-cyanrex-chunk-sha256` | Exact selected chunk digest |
| `x-cyanrex-file-sha256` | Expected **declared** whole-file digest |
| `cache-control` | `no-store` |
| `pragma` | `no-cache` |
| `x-content-type-options` | `nosniff` |

Any `content-encoding`, `content-range`, `transfer-encoding`, `trailer`, `set-cookie`, `location`,
`etag`, `last-modified` or `retry-after` is rejected. There is no duplicate merging, decimal/header
alias normalization, decompression, redirect adoption, cookie acquisition or range reconstruction.
The whole-file header is compared to the manifest, **not freshly rehashed** by a single-chunk call.

Scope/index failures are `InvalidInput`; response/framing/hash failures are `InvalidResponse`.
Only scope/index and response-head checks precede capacity and body polling. Frame/hash verification
follows admission and collection, but still precedes staging. A full receiver returns `CapacityExhausted`
without polling the body or touching files. Private buffer reservation failure is `Unavailable`;
existing file errors remain `Staging(ChunkStagingError)`, not a successful or retryable receipt.
No raw remote error strings or response headers are returned or logged by the receiver.

## Bytes, EOF and time

The private owned buffer is limited to the selected chunk length, at most **4 MiB**. Its allocation
follows admission. Oversize frames are rejected **before copy or hash**. Copy/hash work yields
cooperatively in **64 KiB** slices; incoming frame allocation remains outside this local quota.

The collector polls frames until a real `None` EOF. Neither `content-length`, `size_hint()` nor
`is_end_stream()` proves completion. Short bodies, extra data, body errors or trailers—including an
error/trailer **after all expected data**—fail without creating a file. Empty data frames do not
refresh idle time, and ready empty frames yield so they cannot starve the timer.

Defaults are **four controls**, **five seconds idle**, **sixty seconds total**. `with_limits` accepts
1–16 controls, idle 1–60 seconds, and total at least idle and at most 300 seconds. Only accepted
nonempty bytes refresh idle; total is fixed. Explicit monotonic deadline checks and bounded frame
waits cover collection through EOF/digest, not the subsequent disk job. A trusted body whose
synchronous `poll_frame` blocks the executor cannot be forcibly killed by this timeout.

Clones share one receiver budget; independent receiver instances have additive budgets. The
already-open staging also has its own clone-shared blocking-worker gate. This is not a global
network, proxy, incoming-body, process-memory, physical-disk or multi-instance quota.

## Existing filesystem boundary and cancellation

The receiver reuses [Unix staging](large-file-staging.md), not the Source upload namespace. Local
entries retain the legacy `v1-<SHA256(full typed manifest)>-index` binding; Source `u1` keys are never
read from a response. The trusted supplied root is 0700. Creation is exclusive/no-follow; existing
regular files, symlinks, hardlinks, directories and FIFOs conflict without overwrite, adoption,
deletion or an automatic second attempt.

The existing job checks bytes and EOF again, seals 0400, syncs file and directory, rereads the
original file inode and checks the original root. An early unavailable destination differs from
an `Unconfirmed` outcome after creation; the latter may retain partial or complete output. There
is no automatic cleanup of ambiguous output and no rollback claim.

Before dispatch, cancelling collection releases its buffer/control without creating an entry.
After dispatch, a private reader owns **both the validated buffer and receiver permit inside the
actual queued/running blocking job**. Cancelling its async waiter cannot release those resources
or the staging worker early. The permit drops after the buffer. A dropped/unclaimed completed
metadata result does not retain bytes, and retrying an existing output cannot manufacture the
original completion acknowledgement.

## Deliberate omissions

Included in 0.5.8, [ordered private download](large-file-download-client.md) adds an optional
crate-private whole-flow reference retained after the request permit in the actual save reader.
The supplied-response public entry passes `None`; its policy and the old execution record stay unchanged.

The separate [controlled client](large-file-chunk-client.md) now supplies explicit fixed-origin
requests through a narrow private lease handoff. It does not turn this public supplied-response API
into a requester or change its validation/admission ordering.

No reqwest request/client construction, credentials, Session lookup, Source policy, schema, SQL,
application listener, OpenAPI/SDK/browser/desktop mounting or user-chosen path is added. A future
caller must separately control the remote origin, TLS, redirects/proxies, credentials, cancellation
and request lifecycle. Converting a supplied reqwest response must preserve frame errors/trailers;
convenience data-only iteration must not erase EOF evidence.
The checks apply to supplied headers/frames, not an upstream's original wire. If an upstream has
already decompressed bytes or rewritten headers, this layer cannot reconstruct that history; a
future caller requiring the wire contract must separately disable unsolicited decoding/transformation.

Whole-file assembly/verification, atomic final destination, save dialogs, multi-recipient scheduling,
resume/cleanup journals, current-authority rechecks and peer discovery remain separate. Passing
this preparation's tests does not enable LAN distribution or alter a deployment.

## Verification

`file_chunk_receiver_tdd` separates default adversarial-body/private-file tests from ignored actual
Source/SQL bridges. `scripts/test-file-chunk-receiver.sh` selects every wired ignored case by exact
name with an explicit disposable database URL; `fileChunkReceiver` guards check composition and
inventory only. Private units verify queued/running ownership, not just a simulated reader lifetime.
Actual execution evidence is recorded separately below; inventory is not execution evidence.

Earlier dated project-status blocks, detailed execution tails and the capability tensor are not
rewritten. Version remains 0.5.6/API 0.3.0; this slice does not authorize commit, push or live migration.

## Actual execution · 2026-10-12

The frozen inert scaffold compiled against a real authorized 200/17-byte Source HTTP response.
The baseline failed **0 passed / 1 expected failed**, `InvalidResponse`, with **zero warnings**:
the absence of actual client staging, not a missing symbol or denied filesystem, was the Red.
An initial unused old-helper reexport warning was removed by consuming it in the **new fixture**;
the same behavioral Red was rerun cleanly. The expanded matrix's first Green compile found an
unqualified new test-helper reference; `fixture::observe_chunk` repaired that test-only compilation
issue. It was not behavioral Red. Clean Green then passed **1 / 0**, zero warnings, including actual
distinct-root files, independent reopen, length/digest/mode/single-link and unchanged server snapshots.

| Fresh selection | Actual result and limit |
|---|---|
| New default receiver target | **14 passed**, **3 ignored**, zero warnings; exact heads, budgets, explicit EOF/!Unpin, adversarial/trailing frames, idle/total/ready-empty, cancellation before dispatch, private-root drift and existing target conflict |
| Private receiver worker units | **2 passed**, zero warnings; actual queued and running jobs retain receive/staging gates after waiter cancellation, finish original bytes/inode/0400, reject duplicate save and admit healthy new versions |
| Public result compile-fail contracts | **3 passed**, zero warnings; real E0451 private construction, E0599 missing Clone and E0277 missing Serialize, not missing imports |
| New explicit real Source/SQL runner | **3 passed**, zero warnings; authorized 17 bytes, both chunks of 4 MiB + 17, denied/corrupt responses, client conflict and server lease restoration |
| Adjacent real SQL runners | **6 HTTP + 10 Source reader passed**, zero warnings; original recipient/pins, corruption, byte-owner lifetime, expiry, waits and cancellation |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **903 Rust passed**, **839 ignored**, **86 result groups**, **291 common passed**, zero warnings/failures |

The **19 unique fresh SQL cases** above are not part of the default gate's ignored count. The new
14 defaults, two worker units and three compile-fail cases are already included in its 903, not
additional successes. Inventory is not SQL execution. No previous 117-case run is relabelled as
this turn's evidence, and no fresh frontend build, SDK suite, security audit, LAN/TLS deployment,
whole-file save/resume, disk-full/fsync/power-loss test, RSS/throughput benchmark or peer test was run.

All Cargo, formatting and SQL commands were serialized by the root after edit freeze. Native
private-file/Unix-socket execution preserves the production owner/ancestor checks; none were relaxed
for sandbox mappings. The disposable PostgreSQL instance used only its own Unix socket, no TCP.
Final audit found **zero fixture schemas and zero other database clients**; it was stopped and only
its validated `data`/`socket` removed, freeing **103184 KiB (about 101 MiB)** of reconstructible test
data. Logs remain at `/tmp/cyanrex-file-chunk-receiver-ElTANZ7D`; no deployment or repository output
was deleted. All **106 historical status blocks**, **30 detailed old execution tails** and the
dated capability tensor remain byte-for-byte unchanged. Bilingual current notes and generated
course copies were synchronized. The post-evidence `scripts/quality-gate.sh --format-only` passed
with **291 common checks**, zero warnings/failures, course/version/OpenAPI drift checks, tooling
checks and Rust formatting. A final note-only course sync and the same gate recheck close this record.
