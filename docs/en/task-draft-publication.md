# Strict task draft import and publication mapping

Status: **C2-O internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This pure boundary translates a whole text draft into a bounded publication plan, then checks supplied
Artifact content against that plan to construct the existing [Task content manifest](task-content-manifest.md).
It performs no file, database or network I/O, publishes nothing and does not call C2-M or mount HTTP.
The browser editor remains local-only. A plan or matching supplied content is not a saved Task.

## Import contract and compatibility

The input uses the local draft envelope
`{ format: "cyanrex.task-draft", version: 1, title, payload }`.
Each payload item contains exactly `{ kind: "text", id, revision, filename, language, text }`.
The new server-side importer is deliberately stricter than JavaScript `JSON.parse`: it rejects a BOM,
duplicate or unknown fields, positional records, invalid Unicode, unsupported versions and unsafe
integer revisions. It does not change the browser's existing import behavior.
Both `version` and `revision` require integer JSON tokens: `1.0` and `1e0` are rejected even though
the browser normalizes them through `JSON.parse`; its canonical serializer does not emit those forms.

Local item IDs and revisions are checked for valid shape and uniqueness where applicable, then
discarded. They do not select a Task, Artifact, server revision, owner, namespace or authentication
source. Server identity and expected Task revision remain separate caller inputs at the eventual
authorized command boundary; the importer does not infer them from filenames or text hashes.

| Input | Limit and behavior |
|---|---|
| Whole JSON | At most 8 MiB before parsing, including escaping and metadata |
| Title | Nonblank, at most 256 UTF-8 bytes, no control characters; otherwise preserved verbatim |
| Payload | Zero to 32 ordered text items; a named empty Task needs no dummy Artifact |
| Text | At most 256 KiB of UTF-8 per item; TAB/LF/CR allowed, other control characters rejected |
| Filename | Existing manifest display-label rules, including the 128 UTF-16-unit limit; never a filesystem path |
| Language | Existing extensible server hint syntax; a valid hint does not install or authorize language tooling |

An empty or whitespace-only local title is rejected before creating a publication plan. The importer
does not invent a title, trim labels, normalize Unicode or change line endings. Local editing may
still keep an unnamed draft. Duplicate filenames remain legal; item order and exact text are preserved.

The server's language-hint contract is broader than the browser's 14-profile allowlist. Importing a
valid extension hint here does not make that draft importable by the current browser or select an LSP,
compiler, provider or execution policy. This is a documented format boundary, not a fallback.

## Publication plan and exact binding

[`TaskDraftPublicationPlan`](../../engine/src/services/task_draft_import/mod.rs) exposes `parse_json`,
`title`, `items` and `bind_published(scope, owner, contents)`; its items are `PlannedTextPublication`
values. These accessors do not carry a Session or a storage handle. On Unix, `publication_draft(index)`
constructs an `ArtifactDraft` with kind `cyanrex.task-text`, media type `text/plain`, the Task title and
that item's exact bytes. Constructing this value does not publish it.

The plan contains bounded ordered text items and metadata needed for a later publisher. Its
construction validates the complete draft before any future publication could begin. A filename is
not reused as an Artifact title: filename limits are measured in UTF-16 units, while Artifact titles
have a different nonblank 256-byte contract. No digest-based deduplication grants access or collapses
distinct local items into one shared Artifact.

The plan does not allocate server IDs. Binding accepts one supplied `ArtifactContent` per planned item
by position. The supplied scope, owner, exact revision references, declared lengths, text bytes and
recomputed SHA-256 must agree with the
plan and the existing text-content contract. Missing, extra or byte-mismatching content is
rejected instead of producing a partial manifest. The resulting metadata uses the existing 64 KiB
manifest contract; raw draft text does not enter a C2-N manifest request.

An existing exact revision may be reused when its content passes these checks. Binding does not
require its Artifact title or kind to match the new-publication defaults, so a metadata-only Task edit
does not require republishing unchanged bytes. Duplicate Artifact/revision coordinates are still
rejected by the manifest even when two local items contain identical text; distinct revisions of one
Artifact are allowed. Swapping distinct references between equal-text items can still pass because
the plan pins expected bytes, not previously allocated server IDs or a publication-call history.

These are checks on supplied values. Even a complete matching set could have been forged by its
caller; binding is not proof of publication, storage provenance, a confirmed commit or reusable
permission. A publisher composing this pure layer must obtain results through current-Session Artifact commands, and
C2-M must independently recheck the resulting pins, owner and Session before changing a Task.

## Secrets and untrusted content

Draft text is inert data, including code, markup and configuration. Parsing and binding do not
evaluate it, read referenced paths, contact URLs or execute language tools. Unknown credential,
owner, routing and filesystem fields cannot extend the accepted envelope. Session tokens, Agent
keys and authentication material belong outside task records and exports, as required by the
[target architecture](../zh-CN/next-architecture.md).

Valid text can still contain a secret pasted by a user. Format validation is not secret detection,
redaction or a promise that content is safe to share. Callers must keep raw drafts and returned bytes
out of diagnostic logs and must not silently remove or rewrite text. Per-document bounds are not
process-wide memory, concurrency or durable-storage quotas.
The plan types do not provide a full-text `Debug` representation; this does not prevent a caller from
explicitly logging content obtained through their accessors.

## Failure and next boundary

The separate 0.5.3 [C2-P stepper](session-task-draft-publication.md) now consumes this plan for
explicit current-Session publication and Task creation. It owns an in-memory confirmed prefix and
stops after an unconfirmed step; none of those side effects or outcome rules are added to this pure API.

This slice has no publication side effects to undo. It does not implement a multi-item uploader,
operation receipt, idempotency, outcome reconciliation, retry or cleanup policy. Real Artifact
publication and Task create/replace remain separate transactions. If a later workflow confirms only
some publications, it must preserve that distinction; failure, timeout or cancellation cannot be
reported as whole-draft rollback or used to delete immutable content or blindly retry writes.

The [C2-N router](task-content-http.md) remains unmounted, uses only its existing bounded manifest
requests, and gains neither raw-text upload nor an 8 MiB request allowance from this importer.
Secure Session issuance, explicit installation, public publication and browser save/read/conflict
handling remain separate work. No schema, public OpenAPI, SDK, live authentication or local editor
contract is migrated by this preparation layer.

## Verification scope

The regression boundary is pure import, plan construction and supplied-content agreement: limits,
strict JSON, Unicode, empty titles/payloads, extension hints, ordered bindings and inconsistent bytes
are tested without credentials, a database or a browser upload. On 2026-10-07, the 18 new importer/binding
cases passed alongside 12 existing manifest and 14 supplied-content cases. A separate internal
publication-value unit passed, checking default kind/media, exact bytes and selection of the Task title;
the 22-case local browser-draft parser suite passed with three new shared-fixture regressions.
These are scoped runs, not the full quality gate. Execution records belong in
[project status](project-status.md); no connected browser save or deployment acceptance follows from
these pure checks. The [capability tensor](capability-maturity.md) records this as its own prepared
coordinate while retaining the missing public and browser edges.
