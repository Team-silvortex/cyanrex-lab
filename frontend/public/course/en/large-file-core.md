# Large file manifest and streaming core

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased internal preparation**, on source **0.5.5**. Date: **2026-10-11**.
This first implementation step of [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
provides transport-neutral metadata and synchronous bounded streaming checks. Centralized distribution
comes first; desktop transfers will reuse these checks with a separately reviewed identity boundary.
Neither transfer mode is connected yet. The [distribution proposal](large-file-distribution.md)
defines the remaining persistence, authorization, transport, client and deployment gates. The separate
[Unix staging follow-up](large-file-staging.md) adds private files and worker admission, not upload state.

## File identity and manifest

[`FileTransferManifest`](../../engine/src/models/collaboration/file_transfer.rs) is independent of
Task payloads, text metadata and Artifact storage. Its file/version UUID types are distinct from Task
and Artifact IDs. Required metadata binds one authority/workspace, file, version, display name,
byte length, ordered chunk digests and whole-file digest. None of those values grants access or
proves publication, provenance, persistence or agreement with actual bytes.

The separate version-1 object uses **4 MiB chunks**, at most **64 GiB per file** and **16,384 chunks**.
Its bounded JSON entry point admits at most **1,152 KiB**. All fields are required; duplicate or
unknown fields, positional record arrays, unsupported versions and noncanonical UUID/SHA-256 strings
are rejected. The digest sequence checks its count while decoding without trusting an allocation hint.
Other Serde entry points still require caller-owned wire/time budgets.

Lengths and counts must agree exactly. Empty content has no chunks and the canonical empty SHA-256;
one-chunk content requires equal chunk and whole-file digests. Repeated chunk digests are valid because
identical byte ranges are valid. Multi-chunk whole-file integrity still requires reading real bytes.
`chunk(index)` returns checked offset, exact length and digest, rejecting absent indices. A supplied
reader is not automatically sought to that offset.

Display names preserve accepted Unicode and are limited to 128 UTF-16 units. Blank or edge-whitespace
names, dot names, trailing dots, path separators, controls, bidi override/isolate characters and
Windows-special punctuation are rejected without silently normalizing a name. The label remains inert:
it never selects a path, media parser or executable. Client save rules remain a separate boundary.
Existing text-payload label rules are unchanged.

## Streaming operations

The synchronous operations in [`services::file_transfer`](../../engine/src/services/file_transfer/mod.rs) accept caller-owned
`Read`/`Write` streams with a fixed **64 KiB** byte buffer. It creates no files, opens no paths, resolves
no URLs and dispatches no background workers. Bytes are not decoded as text or buffered as a complete
file or complete chunk. The bounded digest list is metadata, not a second copy of the file.

| Operation | Meaning of success | Caller responsibility |
|---|---|---|
| `inspect_stream` | Actual sequential bytes produced a validated chunk/whole-file manifest | Supply authenticated provenance separately; inspecting is not uploading |
| `verify_chunk` | One exact chunk-only stream matched length and digest and ended | Supply the correct range; offset/source identity is not verified by `Read` |
| `copy_chunk_checked` | The exact chunk was accepted by the writer and passed byte checks | Quarantine output until success; separately confirm durable storage |
| `verify_stream` | Ordered chunks, whole-file digest, declared total length and final EOF all matched | Stream the complete file from the beginning |

Chunk checks consume at most the declared length plus one EOF-probe byte. Full-file checks probe EOF
only after the last chunk, not between chunks. Inspection rejects byte-ceiling overflow rather than
constructing an oversized manifest. Short reads/writes are handled; IO errors, `Interrupted`, zero
writes, trailing bytes and digest mismatches stop the operation. There is no implicit retry policy.

Copying is **not atomic**. A later digest, length or IO failure may leave partial or even full unverified
output in the caller's writer. Success does not flush, close or fsync it; neither success nor failure
authorizes publication, adoption, automatic deletion, retry or a durable receipt. Readers and writers
may block: a fixed memory/byte bound is not a wall-clock deadline, worker quota or bandwidth limit.
Future adapters must provide their own admission, cancellation and lifetime policies.

## Preopened Unix files

`inspect_file` and `verify_file` separately inspect a caller-opened descriptor at offset zero. They
require an ordinary regular file with one link, reject incompatible length and check device, inode,
length, link count, modification and change times before and after successful byte processing.
Reading does not compare access time, which the operation itself may change.

These checks reject observable source changes during this read. They do not open or validate a path,
pin a storage root, establish private permissions/ownership, prevent another handle from modifying the
file, prove permanent immutability, or guarantee a pathname still refers to that inode. Path-safe
private staging is a [separate explicit API](large-file-staging.md), as is
[publication metadata](large-file-publication.md). Physical sealing and durable restart recovery
remain separate work.

## Verification and compatibility

The default [`file_transfer_tdd`](../../engine/tests/file_transfer_tdd.rs) target exercises real inert
bytes and strict metadata without a database or network service. Its real-file fixture exceeds the
old 1 MiB Artifact limit and crosses a chunk boundary. Synthetic readers and writers check buffer
sizes and failure behavior; they do not establish peak process memory, performance, disk-full handling,
durable saves, authenticated recipients or a resumable network workflow.

The live application, prepared Session/HTTP adapters, Artifact/text limits, database formats,
OpenAPI/SDK, browser and release version remain unchanged. The historic capability tensor retains
its existing evidence and scores. Tests and internal Rust APIs do not establish a deployed service.

The separate staging API supplies private files and independently bounded workers. Subsequent
[upload reservations](large-file-upload-intent.md), [chunk journal](large-file-chunk-journal.md),
[whole-byte checks](large-file-whole-verification.md) and [publication metadata](large-file-publication.md)
remain explicit preparation layers. Receipt-loss/restart reconciliation, physical disk policy and
current publisher/recipient authorization remain required before central transport or client delivery.
