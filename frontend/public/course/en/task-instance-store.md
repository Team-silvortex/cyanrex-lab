# ADR-014 Durable task instances

Status: **C2-B included in source release 0.4.8**. Decision date: **2026-10-03**.

The shared platform can now persist a manual or catalogue-defined task without depending on a
teaching role, compiler or Run. A task keeps its exact definition and assessment-policy metadata;
status changes and their outbox records commit together. This is an explicit PostgreSQL staging
service, not a live workflow, authorization service or replacement for existing teaching storage.

## Contracts and ownership

| Location | Responsibility |
|---|---|
| `models/collaboration/work.rs` and `references.rs` | Scoped Task reference, snapshot and lifecycle |
| `services/task_store/draft.rs` | Validate manual input or snapshot an exact trusted catalogue definition |
| `services/task_store/commands.rs` | Create and revision-fenced lifecycle changes |
| `services/task_store/records.rs` | Bounded records and task/outbox-head consistency |
| `services/task_store/schema.rs` | Explicit empty-namespace installation and scope checks |
| `migrations/0011_collaboration_tasks.sql` | Metadata, tasks and transactional outbox tables |

`TaskStore::new(pool, scope)` performs no I/O and reads no environment configuration. The caller
must supply a dedicated pool and an explicit authority/Workspace reference. Reads never allocate
identities. The store neither creates nor proves the existence of a Principal or Workspace.

`TaskDraft::manual` requires no package or Run. `TaskDraft::from_catalog` looks up the complete
package/definition reference in a trusted `TaskCatalog` and copies its definition, evidence-schema
and assessment-policy pins. Private draft fields and the absence of deserialization prevent an
adapter from treating arbitrary client metadata as a registered definition. Missing versions fail;
there is no latest-version fallback. Reloading a stored task does not require the provider to remain
installed, but a saved snapshot does not preserve executable evaluator code.

A task has one owner, a caller-selected stable ID, a title, optional definition metadata, input
Artifact references and a positive JSON-safe revision. This slice does not support reassignment,
content edits, dependencies or definition upgrades. Title, definition and inputs remain fixed during
every transition. Titles are nonblank, control-free and at most 256 UTF-8 bytes. At most 32 inputs
are accepted, with no duplicate artifact/revision coordinates, even with different digests.

Input references must belong to the selected Workspace. Shape and scope checks do **not** establish
content existence, digest integrity or access rights. [C2-C](artifact-revision-store.md) now provides
verified content reads, but this draft constructor does not call that service or compose authorization.
References must not yet be presented as verified execution or review evidence.

## Lifecycle independent of execution and review

Creation records `draft` at revision 1. Every successful transition increments the revision once.

| Current status | Allowed next statuses |
|---|---|
| `draft` | `ready`, `cancelled` |
| `ready` | `in_progress`, `blocked`, `cancelled` |
| `in_progress` | `blocked`, `in_review`, `cancelled` |
| `blocked` | `ready`, `in_progress`, `cancelled` |
| `in_review` | `in_progress`, `cancelled` |
| `cancelled` | None |

No-op, stale-revision and terminal-state changes are rejected. Revision overflow fails without a
write. `in_review` records lifecycle intent, not the existence or verdict of a Review. There is no
`accepted`/`completed` state until authenticated, revision-bound Review and acceptance policy are
implemented. Automatic `TaskAssessment::Passed` and Run success cannot approve a task. Cancelling a
task ends only the work item; it neither stops a Run nor releases an attachment or runtime resource.

## Authorization and installation boundary

The Rust methods accept a Principal reference only from a trusted adapter that has already resolved
authentication and authorization. Owner filtering is **not authentication**: a caller who can supply
another owner's reference is not a supported untrusted caller. No C1 Session, membership, retirement
or revocation checks are composed here. A future public command must bind those checks to the mutation,
not merely precheck access and then call this service after authority could change.

`install_empty_namespace` is the only installer. It accepts one resolved, dedicated schema in
`search_path`, excluding `public` and system schemas. An advisory transaction lock serializes task
installers. Existing relations, functions or types cause refusal; repeated or partial installation
is not adopted or repaired. DDL and authority/Workspace metadata commit together. There is no startup
hook, migration of old teaching records, CLI entry or automatic memory/file fallback.

Each operation verifies schema version 1 and the selected scope. Runtime checks require three
permanent ordinary tables without row-level security, partitioning, inheritance or temporary-table
shadowing. Relation locks pin the checked identities; writers also lock metadata before the task row
and recheck scope after the event write. These are targeted compatibility/corruption checks, not a
complete column/constraint/function fingerprint or protection against a privileged database operator.

## Transaction and failure semantics

Creation or a status change writes the task and one uniquely revision-bound outbox entry in the same
transaction. Events have a stable generated event ID, owner actor, full resulting snapshot and type
`cyanrex.task.created` or `cyanrex.task.status_changed`. Row counts and post-write readback must match;
success is returned only after confirmed commit. Existing outbox entries are never modified by this
service. A failed or suppressed event insert cannot publish a successful task change.

Writers lock metadata, then the task. The latest outbox head is read in a separate statement after
the task lock, so a waiting writer sees a concurrent winner's committed revision and event together.
Readers use one read-only repeatable-read snapshot. Task identity, owner, revision and snapshot must
match the latest event, including event type and actor. This is **head consistency**, not full-history
verification, proof against coordinated tampering, or current authorization from a read snapshot.

Stored JSON is strictly decoded and limited to 32 KiB per task/event snapshot. Database reads bound
the fetched payload even if a stored check constraint has been removed. Operations have a 10-second
overall deadline, 5-second statement timeout and 2-second lock timeout; errors disclose no raw SQL or
connection details and never fall back to volatile success.

Duplicate creation returns a conflict, not idempotent success replay. A stale transition must be
resolved against the current revision. A timeout, cancellation or connection failure around commit
does not prove rollback and must not trigger blind retry. There are no command receipts, reconciliation
API, dispatcher, consumer cursor, global commit order or exactly-once delivery guarantees in this slice.
This outbox is not wired to the legacy telemetry event bus.

## Verification and remaining work

Four default tests cover draft construction, the state graph, unavailable storage and absence of live
or domain-specific composition. Fifteen opt-in PostgreSQL cases cover reopening, exact pins, scopes,
owner filtering, concurrent winners, stale revisions, installer refusal, event failures/suppression,
tampering, deferred commit failure, cancellation before commit, missing events/RLS, temporary shadowing,
post-write scope changes, consistent reads, metadata lock waits and revision overflow.

`scripts/test-task-storage.sh` requires an explicit disposable database and selects all fifteen cases
by exact name, checking that each exists before execution. CI calls this same script; a regression
checks the explicit selection against the Rust tests. See [project status](project-status.md) for
executed checks and limits. Never use a deployed database for these fault-injection tests.

[C2-C](artifact-revision-store.md) now provides bounded immutable Artifact storage and a pinned Task
fixture. [C2-D](review-record-store.md) adds revision-bound Review judgments and history, but not
authenticated provenance or acceptance. [C2-E](session-task-commands.md) now composes current Sessions,
audited memberships and private manual Task commands. This direct store still expects trusted ownership
without a legacy-writer fence. [C2-F](session-artifact-commands.md) adds Session-authorized private
Artifact access; next compose exact references and authenticated Review into protected Task workflows.
Verify offline attempt mapping, ownership and unchanged teaching visibility before any
live cutover. Public Task APIs/UI, reliable Run orchestration and a non-teaching multi-person workflow
remain separate milestones; C2, C-M2 and C-M5 are not complete.
