# Task content metadata and exact bindings

Status: **C2-K internal preparation included in 0.5.0**. Decision date: **2026-10-03**.
This slice implements a typed content manifest and pure snapshot validation. **It does not save
metadata, expose HTTP commands, switch authentication or connect the browser to the backend.**

The local editor has filenames and languages while the prepared Task store only has a title and exact
Artifact input references. The new contract states how those pieces correspond without adding fields
to existing persisted records or using browser IDs as server authority.

## Manifest contract

[`TaskContentManifest`](../../engine/src/models/collaboration/content.rs) serializes as
`{ schema_version: 1, title, payload }`. Each ordered payload entry has exactly
`{ kind: "text", filename, language, artifact }`; `artifact` is a complete existing `ArtifactRef`,
including authority/workspace, Artifact ID, revision ID and SHA-256. There is no floating latest version,
filesystem path, owner, Session, local item ID, local revision, raw text or execution field.

Private fields, checked constructors and checked deserialization enforce the same structural rules.
Manifest, entry, Artifact and workspace records must be JSON objects, not positional arrays; `kind`
must be the literal string `"text"`, not an enum-shaped object. Unknown fields are rejected.
`parse_json` additionally rejects input above 64 KiB before parsing. Parsing is not a lookup: an exact
reference can still be missing, unreadable, forged or outside the actor's permissions.

| Field | Contract |
|---|---|
| Title | Nonblank after whitespace inspection, at most 256 UTF-8 bytes, no control characters; otherwise preserved verbatim |
| Payload | 0–32 entries in order; a named empty task needs no dummy code or Artifact |
| Filename | A display label of 1–128 UTF-16 code units; no slash, backslash, control characters, `.` or `..`; no trimming or normalization |
| Language | An extensible 1–64 byte hint matching `[a-z][a-z0-9_+.-]*`; not an executable, provider, policy or editor-capability grant |
| References | One workspace throughout; duplicate workspace/Artifact/revision coordinates rejected even when digests differ; distinct revisions of the same Artifact remain distinct |

Duplicate filenames are allowed. Filename labels preserve existing local metadata, including reserved
Windows names, surrounding whitespace and bidi formatting characters. They must not be used as paths
or trusted terminal/HTML output. Exporters must apply their own safe filename handling; metadata
validation deliberately does not call the browser's download sanitizer.

The backend hint vocabulary is not the frontend's 14-profile allowlist. A new hint can be retained by
this contract without installing or selecting a language server. The current frontend still rejects
unknown language IDs in local draft JSON; no fallback or import behavior changes in this slice.

## Exact snapshot validation

`validate_task_snapshot` compares the manifest title and ordered pins with a supplied `TaskSnapshot`,
including the task workspace and owner authority. It does not constrain the task lifecycle state or
claim the snapshot revision is current; historical snapshots may be described too.

[`validate_task_content_snapshot`](../../engine/src/services/task_content.rs) additionally checks the
supplied `ArtifactContent` list, in order:

1. The list has exactly one content snapshot for each manifest entry.
2. Every full Artifact reference matches, and the content owner equals the Task owner.
3. Declared byte length matches the supplied bytes, and each item is at most 256 KiB.
4. Bytes are valid UTF-8; TAB/LF/CR are permitted but other control characters are rejected.
5. SHA-256 recomputed from those exact bytes matches the pinned digest.

Empty text, CRLF, Unicode composition and markup are preserved; there is no evaluation, normalization
or inference from a filename extension. Artifact kind/media-type labels do not replace byte validation
and do not select a compiler. The verifier performs no file, database or network I/O and returns no
credential, verified-owner handle or reusable authorization receipt.

All supplied snapshots could have been forged consistently. These checks establish internal agreement,
not trusted provenance, existence in storage or current permission. Future callers must obtain content
through the current-Session transaction adapters and preserve namespace pins, post-write checks and
final authorization. A previous successful validation cannot authorize a later save or Review.

## Compatibility and remaining work

This is **not** the `cyanrex.task-draft` browser import format. That format allows an empty title, local
IDs/revisions and raw text, caps whole JSON at 8 MiB and has JavaScript parsing semantics. This manifest
requires a nonblank server-compatible title, exact server pins, no raw text and strict versioned JSON.
It rejects a BOM, duplicate object fields and invalid Unicode. No title is silently invented, no local
ID becomes a Task ID, and no local revision becomes an expected server revision.

The 0.5.3 [C2-O publication mapping](task-draft-publication.md) adds a separate strict whole-draft
importer. It rejects blank titles, discards checked local IDs/revisions, accepts the server's extensible
language hints, and binds supplied exact content into this manifest. It performs no publication or
authorized save and does not change the browser's existing JSON parser or language allowlist.

This C2-K slice leaves `TaskSnapshot`, `ArtifactRevision`, Task schema 2, core schema 1, outbox formats,
public API and SDK unchanged. The separate [C2-L store](task-content-store.md), also included in 0.5.0, persists this
manifest in a separate schema 3 namespace and column, not inside the 32 KiB Task snapshot.
Artifact title is not reused as filename: their length/meaning constraints differ. The current Draft
input replacement command still cannot save a title-only, filename-only or language-only edit.

The C2-L trusted storage primitive provides atomic metadata updates, and [C2-M](session-task-content.md)
separately composes current Session and Artifact-text checks. [C2-N](task-content-http.md) defines
an unmounted HTTP admission layer; Session issuance and browser save/read/conflict/uncertain-outcome
handling remain absent.
Content publication remains separate from Task changes;
failure does not permit deleting content or blindly retrying. Sharing, typed domain evidence, rule
Review, acceptance, generic execution and migration remain separate boundaries.

## Verification scope

[`task_content_contract_tdd.rs`](../../engine/tests/task_content_contract_tdd.rs) tests constructor and
JSON parity, lengths, Unicode labels, extensions, exact pins, duplicates, empty payloads and Task
snapshot matching. [`task_content_binding_tdd.rs`](../../engine/tests/task_content_binding_tdd.rs) tests
byte, digest, owner and ordered-reference agreement, without connecting a database or browser.
Recorded results belong in [project status](project-status.md); these are not server-save or deployment
acceptance. The [capability tensor](capability-maturity.md) tracks this as a separate prepared coordinate,
without promoting the missing browser-save bridge.
