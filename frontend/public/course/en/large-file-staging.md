# Private large file chunk staging

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased Unix preparation**, source **0.5.5**, **2026-10-11**.
This second implementation step of [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
stores exact verified chunks in an explicitly supplied private directory with an independent worker
budget. It builds on the [streaming core](large-file-core.md), without enabling network transfers.
Persistent upload state, disk reservations, restart recovery, publication and recipient grants remain
separate gates in the [distribution proposal](large-file-distribution.md).

## Trusted API and identity binding

[`ChunkStaging`](../../engine/src/services/file_transfer/staging/mod.rs) is available only on Unix.
`open(scope, root, workers)` accepts an existing absolute private root and a trusted worker count
from **1 to 16**. It neither creates nor changes that directory. The fixed workspace must equal the
manifest workspace; chunk indices must exist. These checks do not authenticate the supplied scope,
publisher or reader. No untrusted route calls this API.

| Operation | Successful result | Excluded meaning |
|---|---|---|
| `stage_chunk(manifest, index, reader)` | Exact chunk copied, verified, sealed, synchronized and read back at the originally created inode | Whole-file integrity, upload journal entry, publication, authorized delivery or replay permission |
| `verify_staged_chunk(manifest, index)` | Current private entry and actual bytes pass checks | Proof of an earlier acknowledged write, recovery, adoption or durable upload progress |

The opaque basename binds SHA-256 of the **entire canonical typed manifest**, followed by the chunk
index. Streaming serialization computes the fingerprint without allocating the whole JSON document.
The display name, request URL and caller-selected filename never choose a stored pathname.
An identical chunk under a different manifest cannot use another manifest's entry.

## Private files and failure handling

The root is an owned **0700** directory pinned by device/inode. Descriptor-relative traversal refuses
symlinks and untrusted writable ancestors, with an explicit root-owned sticky-directory exception
for paths such as `/tmp`. Each operation rechecks the root path before and after file work.
This is observed identity checking, not permanent immunity to changes by the same UID or root.

Creation uses `O_EXCL` and `O_NOFOLLOW`; **every existing destination is a conflict**, even if its
bytes are valid. The new single-link regular file requests 0600, possibly narrower under `umask`.
The existing **64 KiB** copy loop checks exact length, chunk digest and final EOF before setting
**0400**, synchronizing the file and
directory, and reading back the original inode. Visible-entry identity and metadata checks reject
observable replacement or changes. No complete-file assembled copy is required by this layer.

`InvalidInput` and `CapacityExhausted` reject before dispatch. A failed root precheck is `Unavailable`;
an existing entry is `Conflict`. Other creation errors and every failure after successful creation
return **`Unconfirmed`**. Partial private-mode bytes or complete 0400 bytes may remain. There is no overwrite,
automatic deletion, adoption or retry. Ordinary read verification failures return `Unavailable`;
a failed background job is conservatively `Unconfirmed`.

Later verification of a retained file is only a current observation: it cannot turn a lost or failed
write acknowledgement into historical confirmation. Future upload journaling must keep these states
distinct. A single chunk also cannot establish the ordered whole-file digest of a multi-chunk manifest.
Permission 0400 is not WORM storage against the same UID/root. Acknowledged local filesystem sync is
not a tested power-loss or hardware-durability guarantee.

## Worker lifetime and remaining limits

Clones share one fail-fast semaphore. Admission occurs before manifest cloning and dispatch; the
actual blocking closure owns its permit while **queued or running**. Dropping, aborting or timing out
the waiting caller does not release capacity until that job finishes. No waiting-admission queue or
implicit retry is introduced. Verification consumes the same file-work budget.

Independently opened handles have independent, additive budgets. This is not a process-wide,
distributed, password-work, HTTP-response or disk quota. Supplied readers may block indefinitely;
there is no job kill, wall-clock deadline, bandwidth cap, reserved-byte quota or global memory bound.
Creating many manifests or handles is therefore trusted-only until separate ingress and storage
admission exist. Callers own readers before admission; this budget does not limit their resources.

## Verification and integration boundary

The separate [whole-file assembler](large-file-chunk-assembly.md) now adds `assemble_staged_file`
for existing legacy chunks, with a distinct exclusive private whole output and actual whole-byte
readback. It shares worker ownership but does not alter `stage_chunk` or adopt Source `u1` files;
user-selected final saving and recovery remain outside this local operation.

The separate [HTTP receiver](large-file-chunk-receiver.md) now reuses this legacy `stage_chunk`
path only after supplied-response EOF/hash checks. Its reader owns validated bytes/control in the
actual job; no Source hidden-key adoption, active fetching or whole-file saving/resumption is added.

Default Rust tests cover private files and deterministic worker lifecycle barriers without a database
or network. Real file fixtures cross the **4 MiB** boundary; failure cases retain uncertain outputs
and include healthy controls. The dated [project status](project-status.md) records actual runs.
These tests do not establish real disk-full/fsync fault handling, power-loss recovery, RSS/throughput,
authenticated transport, browser saves, TLS/LAN deployment or desktop interoperability.

This staging API itself adds no SQL upload schema, journal, quota reservation, cleanup API, publication, recipient authorization,
HTTP route, live `AppState`, startup installation, migration, OpenAPI/SDK or client connection.
Existing Artifact/text limits and the release version remain unchanged.
The subsequent [upload-intent ledger](large-file-upload-intent.md) now supplies separate trusted
metadata and logical reservations only; it does not connect or adopt this layer's files.
The separate [upload-bound adapter](large-file-upload-staging.md) now issues local original-file
receipts for its own fixed destinations, without adopting legacy files or recording durable progress.
Independent [chunk journal](large-file-chunk-journal.md), [whole-byte verification](large-file-whole-verification.md)
and [publication metadata](large-file-publication.md) now build on that bound path, not legacy files.
Receipt-loss/restart reconciliation, physical disk policy, current authorization, transport and safe
client delivery remain separate gates; none is supplied by this private staging API.
