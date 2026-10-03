# ADR-015 Immutable artifact revisions

Status: **C2-C included in source release 0.4.8**. Decision date: **2026-10-03**.

The shared platform can now publish and read exact content revisions without a teaching rule, Task
or Run. PostgreSQL records ownership, the current head, immutable revision metadata and events; a
private Unix directory holds the bytes. The service preserves old versions and verifies content on
read. It is explicit staging, not live authorization, a general file server or a teaching-data migration.

## Contracts and operations

`ArtifactRevision` in `models/collaboration/artifact.rs` carries a scoped `ArtifactRef`, owner, positive
JSON-safe sequence, optional exact parent, qualified kind, title, media type, byte length and creation
time. Its reference includes authority, Workspace, artifact ID, revision ID and server-computed SHA-256.
These are independent of the product version, now 0.4.9. Content is opaque bytes, not executable
code or an arbitrary domain-specific JSON payload.

The Unix-only `services/artifact_store/` service exposes four explicit operations:

| Operation | Behavior |
|---|---|
| `install_empty_namespace` | Install schema version 1 only into an empty, dedicated database schema |
| `create` | Publish the caller-selected artifact/revision IDs at sequence 1 with no parent |
| `revise` | Require the exact current parent, retain owner/kind, publish a new revision and advance the head |
| `read` | Return only the requested exact revision and verified bytes, subject to owner/scope filtering |

Creation accepts an `ArtifactDraft`, not deserialized metadata. The service derives digest, byte length,
parent, sequence and timestamps. Changes may edit title, media type and bytes but cannot retype the
logical artifact or replace an old revision. There is no branching, deletion, sharing, ownership
transfer, floating `latest` input or public path-based read. Duplicate IDs or existing content paths
are conflicts, not successful command replay or permission to adopt orphaned files.

Each revision accepts 0–1,048,576 bytes. Titles are nonblank, control-free and at most 256 UTF-8 bytes.
Media types use bounded lowercase `type/subtype` tokens, at most 127 bytes, without parameters. The
kind is a qualified name, not a central enum of document/source/lab types. JSON metadata/event snapshots
are strictly decoded and limited to 16 KiB, including a bound in database reads even if constraints
were removed. These are per-operation limits, not storage quotas or streaming-upload support.

## Private file storage

`ArtifactStore::open(pool, scope, root)` synchronously opens an existing absolute directory. It does
not create it, change existing permissions or read environment configuration. The directory must be
owned by the service UID and mode 0700. Ancestor traversal is descriptor-relative, rejects symbolic
links and requires trusted ownership/permissions; root-owned sticky temporary ancestors are allowed.
Use a dedicated local filesystem directory, not a network mount or the legacy runtime data root.

Content filenames derive only from typed scope/IDs/digest; clients cannot supply a filename. New files
are created exclusively with no-follow semantics, written once, made mode 0400, and synchronized along
with the directory. Readback verifies the actual bytes before database publication. Reads reject
symbolic links, hard links, special files, changed length/digest, writable files and replaced roots.
Nonblocking opens prevent a substituted FIFO from hanging a read; file work runs on blocking workers.

Every operation rechecks directory identity and private permissions. These checks detect common
replacement/corruption faults; they cannot prevent a privileged operator or the same Unix UID from
changing data after verification. Mode 0400 is not WORM storage, a signature or a backup. A read returns
bytes matching its digest, not proof of authorship or approval. Linux was tested; other Unix systems
and filesystem crash/power-loss behavior have not been accepted by this slice.

## Publication and failure semantics

Publication spans two stores and is not one filesystem/database transaction:

1. Open a bounded database transaction, verify schema/scope and lock metadata before the artifact.
   Reject known ownership, ID, kind and stale-parent conflicts before writing a new file.
2. Stage the head/revision rows inside that uncommitted transaction. Write, synchronize and verify the
   private content file; no caller can resolve those provisional rows through ordinary reads.
3. Append the matching outbox event, recheck schema, head/revision/event and content, then commit.
   Only confirmed commit returns success. Existing revisions and events are never updated by the service.

The event records the owner actor, exact resulting revision and either `cyanrex.artifact.created` or
`cyanrex.artifact.revised`. A failed/suppressed event or failed commit rolls back database publication.
The file may remain, including a partial file if its write failed. It is not visible through the service
without valid database metadata. A cancellation during blocking I/O may still finish that file write.

No error path deletes or adopts a file. A timeout, cancellation or lost connection around commit does
not establish rollback, so automatic retry or cleanup could be unsafe. Unpublished files require a
future reconciliation/retention policy; there is no garbage collector, repair command or command receipt
in this slice. Database and content must eventually be backed up/restored together. Current tests
exercise controlled failures, not recovery after an actual machine crash or disk-full event.

Async operations have a 10-second waiting deadline, with 5-second SQL statement and 2-second lock
timeouts. Expiry does not terminate an already-running filesystem operation. Errors expose no paths,
credentials or raw SQL and never fall back to memory or the legacy script directory.

## Consistency and authority boundaries

Readers use one read-only repeatable-read database snapshot. Writers lock metadata then the artifact
and read related rows in subsequent statements after the lock, so concurrent winners are observed
coherently. The current pointer must match both the newest revision and newest event; rewinding only
the pointer cannot hide a newer publication. Each returned revision must match its recorded event.
This checks the head and requested revision, not every historical parent, all historical bytes or
coordinated tampering of both stores. There is no event dispatcher or global commit-order guarantee.

Installation uses a namespace-specific advisory lock, refuses public/system/ambiguous search paths
and existing relations/functions/types, and commits DDL with authority/Workspace metadata. Runtime
checks reject RLS, temporary shadowing, nonpermanent tables, partitions and inheritance. They are not
a complete database-schema fingerprint or a defense against privileged database operators.

The supplied Principal must already be authenticated and authorized by a trusted adapter. Owner
filtering is not authentication; knowledge of another owner's UUID is not a supported credential.
Equal digests never grant access to someone else's artifact, and files are not globally deduplicated.
Workspace/Principal existence, membership, retirement, revocation and C1 Session transactions are not
composed into this direct store. No route, AppState entry, startup installer, CLI or teaching-storage
writer is added.

## Task boundary and verification

A non-teaching integration fixture publishes a document, creates a manual Task pinned to that revision,
publishes an edit, then proves the saved task still resolves the original bytes. There is no fake Run
or teacher role. This exercises the two services together; `TaskDraft` itself still checks reference
shape/scope only. Public task creation must later compose content lookup and current authorization.
This slice does not claim authenticated Review, task acceptance or a complete public workflow.

The original C2-C suite includes four default tests covering input limits, closed storage, unsafe
directory refusal and no live/domain coupling. Seventeen explicit PostgreSQL cases cover immutable history, ownership/scope/digest mismatch,
concurrency, the Task link, installer refusal, failed/suppressed/deferred writes, cancellation, corrupt
or missing files, link/FIFO substitution, root replacement, consistent reads, metadata/event faults,
RLS/temp tables, collisions, pointer rewind and bounded strict decoding. The CI helper
`scripts/test-artifact-storage.sh` checks each exact test name before running it on a disposable
database. See [project status](project-status.md) for executed checks and limits.

[C2-D](review-record-store.md) now adds pinned Review judgments and history; it does not itself verify
Artifact evidence or authenticate reviewers. [C2-F](session-artifact-commands.md) now composes current
Sessions and private Artifact commands; this direct store still expects trusted ownership. [C2-G](session-task-inputs.md) validates exact private inputs on the Task transaction without writing
or cleaning Artifact files. [C2-H](session-review-commands.md) adds private human Review
authorization and exact owned target/evidence checks. Cross-user review rights, reconciliation, quotas
and retention remain required before live adoption. Keep
the existing teaching history, private ownership and API/SDK contract unchanged until an explicitly verified migration.
