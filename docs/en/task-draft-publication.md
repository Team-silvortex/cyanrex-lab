# Strict task draft import and publication mapping

Status: **C2-O internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This pure boundary translates a whole text draft into a bounded publication plan, then checks supplied
Artifact content against that plan to construct the existing [Task content manifest](task-content-manifest.md).
It performs no file, database or network I/O, publishes nothing and does not call C2-M or mount HTTP.
The normal Next.js editor remains local-only; the separate opt-in host flow is described below.
A plan or matching supplied content is not a saved Task.

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

At the C2-O implementation stage, the server accepted language hints beyond the browser's 14-profile
allowlist. The **2026-10-08 Unreleased editor follow-up** now accepts that bounded hint syntax in local
drafts too, preserving unknown hints while resolving local editing capabilities to plain text with a
no-tools notice. This changes neither the Rust importer nor its stricter raw-JSON rules; a valid hint
still selects no LSP, compiler, assessment provider or execution policy. See [the editor boundary](editor.md).

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

## Unreleased browser-side existing-Draft save attempt

Included and extended in **0.6.6**; this heading retains its implementation-stage anchor. [`taskContentSave.ts`](../../frontend/src/features/tasks/taskContentSave.ts) and
[`taskSaveContract.ts`](../../frontend/src/features/tasks/taskSaveContract.ts) provide a pure,
single-use content-save attempt. `prepareTaskContentSave(draft, {task, manifest}, guard, allocations)`
freezes the inputs and computes local SHA-256 asynchronously. It performs no fetch, Cookie, environment,
storage or UI work. `TaskDraftWorkspace` and its controller remain local-only.

`prepareTaskBoundContentSave(draft, {task, manifest}, guard, allocations)` separately prepares
the [Task-bound publication contract](platform-text-artifact-connection.md#unreleased-standalone-task-bound-http).
Each publish step has an outer `task_id`; its body contains exactly `schema_version`, `expected`,
`artifact_id`, `revision_id` and `text`. Every `expected` is the same frozen complete **original**
Task/manifest, never the desired manifest or a baseline rewritten after a publication. All complete
publication envelopes are checked against 2 MiB before hashing; each text still has the 256 KiB limit.
This factory does not silently select or fall back to the older unbound factory, whose shape and behavior stay compatible.

Task-bound Artifact confirmations require the **original Task title**, even when the draft title has
changed. Only the final, separately confirmed manifest PUT uses the edited title and new exact pins;
its `expected_revision` remains the original Task revision. A confirmed Artifact is not a saved Task.
The shared synthetic `engine/tests/fixtures/task-bound-content-save.json` is checked by both the
browser planner and the strict Rust HTTP adapter; this proves wire agreement, not actual storage or authority.

The strict baseline must describe an existing manual Draft with matching Task/manifest metadata.
It pins target, owner, workspace and expected server revision; local item IDs/revisions are not uploaded.
The caller explicitly supplies new Artifact/revision UUID allocations. The local guard's `generation`,
`revision` and `contextEpoch` fence stale work; they do not authenticate a caller or grant permission.
Creating a new Task is not this module's capability: a future adapter must first obtain an empty
Task's confirmed 201 baseline or explicitly read an existing Task.

Each text item yields one publication descriptor. Only exact supplied Artifact metadata agreeing with
the allocated IDs, pinned owner/scope and planned text digest can advance to the next item. After all
items, the attempt yields the complete manifest PUT with the pinned `expected_revision`. Empty/title-only
payloads go directly to PUT. Completion requires an exact Task response at revision **expected + 1**
matching the frozen attempt; it never automatically clears the current editor's dirty state.

Handing out a descriptor records it as pending **before** any transport exists. Its outcome is
conservatively unknown, not proof that I/O was dispatched. Stop, cancellation, stale guards, conflicts
or invalid confirmations preserve the confirmed prefix and pending step without retry, adoption,
deletion, checkpoint import or restoration. A late receipt after a stale guard cannot confirm that
pending step. Public confirmation validates supplied DTO consistency only: even matching data proves
neither network provenance nor COMMIT. A trusted actual adapter is still required.

The platform Session page included in 0.6.6 calls `prepareTaskBoundContentSave` only when both default-off
`--task-text-publication` and `--session-ui` are enabled and text actually changes. A fixed-slot bridge
converts its verified snapshot/edited title, labels and text into local `slot-N` revision-1 items;
these are editing identities only. It publishes every existing slot in order, then confirms the
separate manifest replacement. Metadata-only edits retain the existing single PUT/no POST path.
Unchanged slots retain exact CRLF; textarea edits replace only the changed middle, keeping original
prefixes/suffixes and applying the first original newline style to newly inserted lines. The entire
portable draft, including JSON metadata/escaping, stays within 8 MiB. Preview defaults to Cancel.
Denial headers, pagehide and terminal conflict/unknown outcomes cannot advance or revive that plan.
Success uses confirmed metadata plus frozen submitted text, not a fresh server content read.

[`scripts/generate-platform-task-save-assets.mjs`](../../scripts/generate-platform-task-save-assets.mjs)
uses Node's built-in TypeScript stripping to generate only `taskContentSave.js`, `taskSaveContract.js`,
`taskDraft.js` and `taskPayloadLanguage.js` under the host's `session_ui/task-save` directory.
The fixed source list/import graph and `--check` reject drift; six generator tests cover this boundary.
There is no hand-maintained planner copy or new package dependency. Generation/check and runtime were
verified on Node 24.21; emitted ESM runtime was also checked on 22.22. Official SHA-256-verified
Node 22.23.3 passed generator `--check` and all six tests, producing identical bytes to 24.21.
The host serves thirteen fixed assets including this bridge
and the four mirrors. Normal Next.js drafts/SDK remain separate, with no Origin/CORS/Cookie policy change.
`npm --prefix frontend run test:task-content-save` selects pure state/contract tests, not live browser
saving, authentication, durable recovery or deployment acceptance. `platformTaskBodyContract.test.mjs`
adds 20 pure bridge/receipt/newline cases; the separate 13-case body browser fixture uses mock APIs.
The separate [real body runner](testing-guide.md#optional-independent-host-session-page) requires plain
and TLS selectors; the plain case passed natural login, two strict publications, independent Task
replacement and exact SQL/file checks, including first-POST conflict/revocation refusal. The additional
real second-POST 503/401 cases retain one confirmed Artifact without a Task change; both passed
exact SQL/file/UI checks. Both original Session/body TLS selectors passed separately in an isolated
worker, including confirmed SQL shutdown. Late-ACK/BFCache and deployment remain unaccepted; dated C2-O evidence stays unchanged.

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

The independent **2026-10-11 Unreleased [C2-Z catalogue stepper](session-catalog-task-draft-publication.md)**,
on source 0.5.4 outside the published tag, consumes the same plan through a separate typed workspace
and exact definition reference. Its no-I/O preparation freezes full definition metadata before random
target allocation; later advances publish one Artifact or finally create a catalogue Task. The plan
format, limits and pure binding semantics do not change. Its opaque attempt has no conversion into
the manual C2-P–W workflow and adds no checkpoint/journal/observation/recovery or HTTP/browser connection.

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
