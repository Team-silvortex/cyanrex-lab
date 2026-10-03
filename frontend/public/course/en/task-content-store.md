# Task content storage and atomic editing

Status: **C2-L internal preparation included in 0.5.0**. Decision date: **2026-10-03**.
This slice persists a manual Task and its [content manifest](task-content-manifest.md) together,
with revision-fenced editing and a matching outbox record. **It is a trusted storage primitive, not a
Session-authorized command, verified Artifact read or browser save workflow.**

## Separate storage admission

`TaskContentStore` is exported from `services::task_store`, but is a separate public handle from
`TaskStore`. The old handle remains fixed to schema 2; the new handle requires schema 3. Both require
an explicitly selected, dedicated namespace. Neither can adopt the other's records, and an installer
refuses an occupied namespace. No default manifest is invented for an old task.

The stores share bounded transaction, scope and schema checks, not content read/write logic. The new
handle keeps its version-specific helper private. Existing Session manual/input/catalogue adapters
still construct the schema 2 handle, so they are not routes into content storage. There is no startup
installation, public API change, authentication cutover, automatic migration or fallback.

The fresh schema uses the three task metadata/head/outbox tables. Every content head and event has a
required manifest column bounded to 64 KiB, separate from the unchanged 32 KiB Task snapshot budget.
Core `TaskSnapshot` and `CoreSchemaVersion` remain unchanged; storage schema 3 is not product version 3.

## Content and revision contract

`TaskContentSnapshot` returns both `task` and `manifest`. Creation derives the Task title and ordered
input references from one checked manifest, starts at Draft/revision 1 and stores no catalogue
definition. A named task can contain zero text items. Filename and language remain inert metadata,
not paths, installed tooling, permissions or execution instructions.

| Operation | Behavior |
|---|---|
| `create` | Explicit stable Task ID and trusted owner; duplicate ID is conflict, not successful replay |
| `get` | Bounded read-only repeatable-read snapshot of the head and latest matching event; another owner or missing Task returns none |
| `replace` | Expected Task revision and Draft state required; replace title, labels and ordered pins as one operation |
| `transition` | Existing lifecycle graph and expected revision required; preserve the manifest exactly |

A title-only, filename-only or language-only change increments the **same Task revision once**, as does
changing payload order or adding/removing items. An identical manifest is rejected as a no-op. A stale
revision, invalid state or revision overflow cannot become a successful edit. Definition, owner,
identity and creation time are not editable through replacement.

Each successful write atomically commits the head and one outbox event containing the complete Task
and manifest. Events distinguish `cyanrex.task.created`, `cyanrex.task.content_updated` and
`cyanrex.task.status_changed`; a metadata-only edit is not mislabeled as input replacement. Prior event
records are not rewritten. Outbox recording still does not provide delivery, replay cursors or a
general history/recovery API.

## Validation and failure boundaries

Readers validate namespace/scope, owner, Task revision, manifest shape, exact title/input agreement
and complete head/latest-event equality. Writers lock metadata before the Task head, use a separate
event read after the row lock, then verify the complete expected result and schema again after writing.
Task and manifest sizes are limited before oversized database text is fetched, not only by DDL checks.

This is head consistency, not a whole-history audit or defense against a privileged database operator.
Agreement cannot prove the existence or correctness of an Artifact. In particular, a well-formed
reference can be persisted without its bytes being present: direct callers must already be trusted.
This store does not perform the C2-K text-byte check, open files, publish/delete Artifacts, authenticate
an owner, or issue a reusable authorization receipt.

Only a confirmed transaction commit publishes success. Trigger suppression, invalid read-back or
commit failure must not publish a partial edit. A timeout or cancelled wait around commit does not
prove rollback; no automatic retry or cleanup of independently published content is authorized.

## Verification and next boundary

The dedicated default and opt-in PostgreSQL tests cover the API boundary, persistence, metadata-only
updates, competing revisions and injected storage faults. The exact CI runner rejects missing test
names before executing each selected case. Actual dated results and omissions are recorded in
[project status](project-status.md), not inferred from the existence of test source.

The separate [C2-M Session adapter](session-task-content.md) now borrows storage operations into one
source-owned transaction, validates old/new Artifact text and rechecks current authorization before
commit. The direct store remains a trusted primitive. [C2-N](task-content-http.md) adds an unmounted
HTTP adapter, not Session issuance or browser save/read/conflict handling. Catalogue admission, sharing, Review, execution and
migration remain independent work.
