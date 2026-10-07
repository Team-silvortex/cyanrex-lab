# Draft publication metadata checkpoints

Status: **C2-R internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This Unix-only boundary exports a bounded metadata snapshot from a surviving
[C2-P attempt](session-task-draft-publication.md). An explicit trusted caller may retain the encoded
data, but export does not save it. Parsing yields data, not an attempt, authorization, a durable journal
or evidence that any reported write committed.

## Export and wire format

`attempt.checkpoint()` constructs a separate opaque `SessionDraftPublicationCheckpoint`.
Its `to_json` and `parse_json` operations use the version-1 `cyanrex.task-draft-checkpoint` format.
All three operations return `Result<_, ContractError>`; the checkpoint exposes read-only metadata
accessors, not a mutable state or public `Clone`, `Debug`, `Serialize` or `Deserialize` implementation.
The format version is independent of product version 0.5.3 and storage schema versions.
The attempt remains non-deserializable; no checkpoint operation changes its progress or performs I/O.

The envelope contains exactly `format`, `version`, `task`, `manifest`, `byte_lengths`,
`reported_state` and `reported_confirmed_artifact_count`. The manifest retains the title, ordered
display labels and **all allocated** Artifact references, including unpublished targets. Lengths
describe expected UTF-8 bytes, not observed stored bytes. No text body, token, Session fingerprint,
owner, source handle, namespace, storage path, database OID or clock is exported.

| Field or relation | Required boundary |
|---|---|
| Whole encoded document | At most `MAX_SESSION_DRAFT_CHECKPOINT_BYTES`, 64 KiB, including escaping and metadata |
| Manifest and lengths | Existing manifest title/label rules; zero to 32 ordered items with one length per item, each at most 256 KiB |
| Scope and identities | All references share the Task scope; canonical non-nil scope IDs and the allocated UUIDv4 target identities are retained |
| Allocated references | Artifact IDs and revision IDs are each unique; equal content digests do not merge distinct targets |
| Reported progress | Count is between zero and the item count; `complete` requires the full count |

Parsing rejects a BOM, unsupported formats/versions, duplicate or unknown fields, malformed Unicode,
positional records and invalid numeric forms. Integer fields require integer JSON tokens rather
than float/exponent spellings. The total input limit is checked before parsing; serialization must
also fit the limit. Metadata consistency cannot verify the original text byte by byte because it is
not present in the checkpoint.

## Reported state is data

`reported_state` is `ready`, `unconfirmed` or `complete`. For an `unconfirmed` snapshot with `N`
allocated items and reported confirmed count `c`, `reported_unconfirmed_step()` derives Artifact
`c` when `c < N`, or the Task when `c == N`. Other states have no reported unconfirmed step.
This accessor computes a data value; it does not read storage or invoke [C2-Q](session-task-draft-observation.md).
A named empty plan has no Artifact placeholders: its unconfirmed target is the Task at count zero.

A genuine in-memory attempt records results returned by its commands. Once metadata is imported,
the same field names are only caller-supplied claims: a structurally valid checkpoint may have been
forged, edited or replayed. `complete` does not prove a historical commit or acknowledgement; an old
`ready` snapshot does not prove that no write was dispatched after export. References and hashes do
not grant access, authenticate a Session or identify the writer that created matching content.

Keep checkpoints private. Titles, labels, references and digests can be sensitive even without text
or credentials. Format validation is not secret detection or permission to publish diagnostic data.

## Persistence and recovery remain separate

There is no save API, filesystem write, flush guarantee, database transaction or retention policy.
An external caller's storage is not synchronized with C2-P dispatch or commit, so a saved checkpoint
may be stale or absent after a crash. Canonical encoding does not supply integrity authentication,
anti-replay protection, a write-ahead intent record or proof of durable outcomes.

Parsing cannot restore an attempt, import confirmations, select a source, read a target, resume C2-Q,
retry a write or authorize cleanup. C2-Q still requires the surviving original attempt and its currently
authorized original Session. Public HTTP, browser saving, Session issuance and live-source installation
remain absent.

0.5.3 [C2-S](session-task-draft-inspection.md) is a separate explicit inspection using a trusted
workspace and the caller's current Session. It does not run during parsing, recreate C2-Q or verify
the checkpoint's original caller; the checkpoint remains unauthenticated data.

A future intent journal must separately define durable-before-dispatch ordering, outcome recording,
writer coordination and unknown-commit handling. Restart inspection and any retry policy require
their own authorization and failure contracts; neither follows from this metadata format.

## Verification boundary

Default codec tests cover strict parsing, bounds, allocated-reference consistency, state/count
relations and export round trips. These are pure metadata checks, not crash durability, current
authorization, historical provenance or restart recovery. Actual dated results belong in
[project status](project-status.md); [the test guide](testing-guide.md) keeps them separate from C2-P
and C2-Q database evidence.
