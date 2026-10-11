# Owner private task content metadata index

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased internal preparation**. Decision date: **2026-10-11**.
Source version: **0.5.5**. The independent `TaskContentIndexStore` and
`SessionTaskContentIndexWorkspace` list bounded metadata pages from existing schema 3 Task content.
They do not install storage, expose HTTP/SDK/UI, read Artifact files or prove that content can be reopened.

The index is separate from [content persistence](task-content-store.md) and
[Session content reads](session-task-content.md). A metadata entry is neither a content snapshot nor
a reusable read/write grant. This follow-up changes no storage version, migration or recovery workflow.

Separately, the same-day **2026-10-11 Unreleased [index HTTP adapter](task-content-index-http.md)**
on source 0.5.5 projects the existing Session page at exact `GET /platform/v1/task-index`.
It is independently constructed and unmounted; this service-layer contract and the original content
routers stay unchanged. The HTTP projection does not make these service values serializable or grant
content access, browser saving, SDK/OpenAPI exposure or recovery.

## Explicit index configuration

`TaskContentIndexStore::new(pool, scope)` admits manual records with `definition: None` only.
`TaskContentIndexStore::from_definitions(pool, scope, &catalog)` explicitly configures a neutral index:
it admits manual records and catalogue records whose complete stored definition matches the frozen
`TaskDefinitionCatalog`. The configured catalogue may compose packages and exact versions and contain
both bound and explicitly assessment-unbound definitions. An unbound definition remains `Some`, not manual work.

The Session counterpart uses `SessionTaskContentIndexWorkspace::new(namespace, scope)` or
`SessionTaskContentIndexWorkspace::from_definitions(namespace, scope, &catalog)`. The namespace is
trusted configuration, not client routing; it must be valid and non-system. No Artifact workspace or
private file root is accepted. Configuring either index installs or adopts nothing.

A selected catalogue record without a configured definition, full-definition drift or the wrong mode
fails as `InvalidRecord`; it is not silently skipped or reinterpreted. The neutral index does not relax
the original `TaskContentStore` or Session content write/get policies: those existing manual and
catalogue handles still reject the other mode. Index admission does not run a provider or assessment.

## Page and cursor contract

The trusted method is `TaskContentIndexStore::list(owner, after, limit)`. The Session method is
`DurableAuthSource::list_session_content_tasks(token, &workspace, after, limit)` and derives the owner
from the current Session rather than accepting one from the caller.

`after` is an optional `TaskRef`; `limit` defaults to **20** and accepts **1–32**. Tasks are selected
for that owner in ascending Task UUID order, strictly after the cursor. Selection reads at most
`limit + 1`, hence **33**, candidate heads. Every selected row, including the lookahead, passes the
existing strict schema 3 Task, manifest and latest matching outbox checks. A malformed owned record
cannot disappear through a JSON-based filter; invalid lookahead makes the whole page fail.
Required Task/outbox column availability is checked with fixed zero-row projections even for an
empty owner; a missing content column cannot be mistaken for a valid empty list. These checks do
not decode other owners' records or establish complete schema/row integrity.

`TaskContentIndexPage` contains `entries` and `next_after`. Only when a validated lookahead exists,
`next_after` is the last **returned** entry's reference, never the lookahead reference. A final or empty
page returns no next cursor. Each `TaskContentIndexEntry` has exactly these seven fields:

| Field | Meaning |
|---|---|
| `reference` | Scoped Task reference |
| `revision` | Current Task revision |
| `title` | Current task-content title |
| `status` | Current lifecycle status |
| `definition_ref` | Exact configured definition reference, or none for manual work |
| `updated_at` | Current Task update time |
| `payload_items` | Manifest item count, not contents or byte availability |

The page returns no body, manifest, owner, complete definition or Artifact revision/digest pins.
Metadata still belongs to the private owner and must not be treated as public catalogue data.

A cursor is a position, not authorization or proof that its Task exists or belongs to the owner.
A same-scope reference to another owner's Task or a nonexistent ID is a valid position; the query
still selects only the current owner. A different workspace/authority scope is rejected. There is no
total count, search, cross-page snapshot or complete inventory guarantee. Rows outside the bounded
selection are not inspected, so a successful page is not a whole-store integrity or health proof.
The 33-head bound limits returned/decoded candidates, not PostgreSQL's physical scan or sort work.
The existing schema has no owner/Task composite index; command and SQL deadlines bound waiting,
but this slice does not establish large-inventory performance or add an index/migration.

## Transaction and authorization boundary

Direct trusted listing uses a bounded **read-only repeatable-read** transaction and returns only
after commit. The supplied owner must already be authorized by the trusted caller; owner filtering
is not authentication. The store checks schema 3, scope and complete head/latest-event consistency,
not Artifact existence, bytes or historical provenance.

Session listing uses one transaction owned by `DurableAuthSource`. Existing source and registry
guards derive the current account incarnation and audited active membership before reading Task
metadata and the bounded candidate heads with `FOR UPDATE`. Before commit, the same guard rechecks
the original namespace identities, exact Session, account and membership using fresh database time.
The pending page is returned only after those checks and confirmed commit. The existing ten-second
command deadline and statement/lock limits remain in force.

This path neither pins an Artifact namespace nor accesses blobs or files. Listing can therefore
succeed when referenced content is absent or unreadable; reopening still needs the existing
Session content reader and its exact-byte checks. No business write, saved-write receipt, HTTP route,
Session issuer, browser saving, automatic retry, cleanup or recovered attempt follows from a page.
Cancellation or a lost acknowledgement is not proof of rollback or permission to reuse a grant.

## Source inventory and verification scope

The source selections are `task_content_index_tdd` and `session_task_content_index_tdd`.
The independent `scripts/test-task-content-index.sh` selection contains **four trusted-store** and
**eight Session SQL cases**, **12** in total. These are source inventory counts, not passing results.
They select pagination and cursor boundaries, complete record/catalogue validation, private ownership,
Artifact independence, current lifecycle checks and lock-wait cancellation/expiry boundaries.

Actual execution results and omissions belong in dated [project status](project-status.md).
Earlier content-store, Session-content or 0.5.5 release results do not establish this new selection's
execution, public integration or installed-system acceptance. See [testing guide](testing-guide.md).

Implementation references:

- [Trusted index and page types](../../engine/src/services/task_store/content_index.rs).
- [Current Session index adapter](../../engine/src/services/auth_service/durable_source/task_commands/content_index.rs).
- [Trusted-store test target](../../engine/tests/task_content_index_tdd.rs).
- [Session test target](../../engine/tests/session_task_content_index_tdd.rs).
