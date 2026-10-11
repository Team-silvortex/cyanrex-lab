# Task content storage and atomic editing

Source release **0.5.5** (**2026-10-11**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

Status: **C2-L internal preparation included in 0.5.0**. Decision date: **2026-10-03**.
The original slice persists a manual Task and its [content manifest](task-content-manifest.md) together,
with revision-fenced editing and a matching outbox record. **It is a trusted storage primitive, not a
Session-authorized command, verified Artifact read or browser save workflow.**

The **2026-10-08 Unreleased follow-up**, on source version 0.5.4, adds explicit catalogue-definition
admission to the same schema 3 content store. The original constructor remains manual-only; no schema
change, migration, evaluator or public save endpoint is introduced.

## Separate storage admission

`TaskContentStore` is exported from `services::task_store`, but is a separate public handle from
`TaskStore`. The old handle remains fixed to schema 2; the new handle requires schema 3. Both require
an explicitly selected, dedicated namespace. Neither can adopt the other's records, and an installer
refuses an occupied namespace. No default manifest is invented for an old task.

The stores share bounded transaction, scope and schema checks, not content read/write logic. The new
handle keeps its version-specific helper private. The existing schema 2 Session manual/input/catalogue adapters
still construct the schema 2 handle, so they are not routes into content storage. There is no startup
installation, public API change, authentication cutover, automatic migration or fallback.

The fresh schema uses the three task metadata/head/outbox tables. Every content head and event has a
required manifest column bounded to 64 KiB, separate from the unchanged 32 KiB Task snapshot budget.
Core `TaskSnapshot` and `CoreSchemaVersion` remain unchanged; storage schema 3 is not product version 3.

## Content and revision contract

`TaskContentSnapshot` returns both `task` and `manifest`. Creation derives the Task title and ordered
input references from one checked manifest and starts at Draft/revision 1. Manual creation stores no
catalogue definition; explicit catalogue creation copies the admitted definition in full. A named task
can contain zero text items. Filename and language remain inert metadata,
not paths, installed tooling, permissions or execution instructions.

| Operation | Behavior |
|---|---|
| `create` | Manual handle only; explicit stable Task ID and trusted owner; duplicate ID is conflict, not successful replay |
| `create_catalog` | Catalogue handle only; select an exact definition reference from its frozen metadata and save it with the manifest |
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

## Catalogue content admission included in 0.5.5

`TaskContentStore::from_definitions(pool, scope, &catalog)` takes a validated `TaskDefinitionCatalog`.
Its `create_catalog(id, owner, &definition_ref, manifest)` selects metadata from that frozen catalogue,
not an evaluator or a caller-supplied definition body. Task title and content labels remain independent
of the definition's display title. Present evidence-schema and assessment-policy pins are preserved
as metadata; this store neither executes their rules nor validates domain evidence.

The **2026-10-11 Unreleased multi-package follow-up**, on source 0.5.4, allows the same constructor
to retain a `TaskDefinitionCatalog::compose` result. Exact definitions from different packages or
versions share one explicit content workspace; disjoint same-package registrations are also allowed.
Composition rejects every complete-reference overlap and bounds the flattened catalogue to 256
definitions, including nested inputs. It does not merge records, upgrade definitions or change this
store's schema, mode checks or trusted-owner requirement. See [catalogue composition](task-domain-boundary.md).

The separate **2026-10-11 Unreleased assessment-binding follow-up**, on source 0.5.4, also admits
definitions that explicitly have neither automatic-assessment pin. Both JSON fields are required;
absence is two nulls, not omission or a half-bound record. Bound and unbound definitions can share
one frozen catalogue. An unbound definition remains `Some(TaskDefinition)`, so manual/catalogue mode
separation does not change. Full-definition validation rejects binding additions or removals under
the same reference as `InvalidRecord`, even when the head and event agree. Replacement preserves
the original binding choice; it cannot upgrade or downgrade the definition.

The schema number and existing bound JSON encoding stay unchanged, with no record rewrite or
migration. New unbound records need the new reader; old binaries fail closed rather than read them
as manual work. See [the definition contract](task-domain-boundary.md) for the internal Rust API
change and the unsupported downgrade boundary.

`new(pool, scope)` plus `create` still admits only definition-free manual work. Calling the wrong
creation method for a handle returns `InvalidInput`; unknown creation coordinates return
`UnknownDefinition`. For visible stored records, each handle requires its own mode: manual requires
`definition: None`, catalogue requires `Some` with the complete exact configured definition. A wrong
mode, missing definition entry or changed metadata under the same pin returns `InvalidRecord` on
read/edit/transition, even if the head and outbox agree with each other. There is no latest-version
fallback, reinterpretation, promotion from manual work or definition change through content editing.

This is handle-level admission on the existing schema, not a schema migration. The independent
[Session catalogue-content commands](session-task-content.md) supply current authorization and text
verification; direct store calls still require a trusted owner and do not read Artifact bytes.

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
HTTP adapter, not Session issuance or browser save/read/conflict handling. Its existing manual
workspace and the C2-P–W publication/observation paths do not accept the new catalogue workspace.

The separate **2026-10-11 Unreleased [C2-Y adapter](catalog-task-content-http.md)**, on source 0.5.4,
explicitly joins HTTP to the existing catalogue Session content workspace. It does not turn direct
store calls into authorization, relax full-definition equality or change schema 3. Manual HTTP and
C2-P–W remain manual-only; the catalogue router is also unmounted, without a Session issuer,
automatic router merge, browser save or migration.
Sharing, Review authority, execution and migration remain independent work.
