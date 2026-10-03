# ADR-017 Session authorized private manual tasks

Status: **C2-E included in source release 0.4.8**. Decision date: **2026-10-03**.

Durable Sessions, C1 identities/memberships and C2 Task operations now share one database transaction.
An active member can create, read and advance their own manual tasks; the server derives ownership
without a caller-selected actor. This is internal authorization staging, not a public Task workflow.
Source release 0.4.8 leaves live teaching authentication/routes, deployments and existing data unchanged.

## Permission and task scope

Three `DurableAuthSource` operations require a current Session and trusted `SessionTaskWorkspace`
configuration.

| Operation | Behavior |
|---|---|
| `create_session_manual_task` | Accept an ID and title; create a private Draft without a definition or Artifact inputs |
| `get_session_manual_task` | Return only the current owner's task; absent and other-owned tasks both return none |
| `transition_session_manual_task` | Check ownership and expected revision before an existing manual status transition |

Admission requires the exact current source-account incarnation, a non-retired active Human, an active
Workspace and an audited active membership. This private-work policy neither requires nor assigns a
teaching role or deployment grant: a valid member with no role presets can use it. Teachers and deployment
managers gain no access to someone else's private tasks. Unbound, missing, suspended, retired, logged-out,
expired or audit-inconsistent actors fail closed. A recreated username cannot inherit the old Principal's work.

These three C2-E commands continue to return `UnsupportedTask` for catalogue definitions or Artifact
inputs, even to their owner. [C2-G](session-task-inputs.md) adds a separate same-transaction adapter
for definition-free private tasks with exact Artifact inputs. [C2-I](session-catalog-tasks.md) separately
admits complete server-configured catalogue definitions with 0–32 inputs, without executing providers.
Neither widens these manual methods. There is no listing, reassignment, sharing, acceptance or
automatic-completion operation. A passing rule or successful Run still cannot accept a Task.

## Database and transaction boundary

Accounts, Sessions and the C1 registry occupy one explicit private namespace; Tasks occupy a different
explicit namespace in the same database. Both use the source connection and one commit, not independently
committing pools. `SessionTaskWorkspace` accepts a canonical non-public, non-system schema name and a
Workspace; its fields are private and it has no deserializer. It must come from trusted server configuration,
not a request. These type restrictions alone are not authentication. Commands never install, upgrade,
repair or adopt namespaces.

Before authority lookup, the command checks the source namespace name/OID, one effective search-path
entry and no temporary namespace. Relation locks and catalog checks verify ownership, permanent ordinary
tables and primary keys for fourteen source/registry relations, plus the auth-source shape. RLS,
partitioning, inheritance and temporary shadows are rejected. Lock order is source metadata, registry
metadata, authority, identity/policy and Task metadata/rows. The authority write lock serializes these
commands within one authority; this is not a claim of optimized concurrency.

Crate-internal TaskStore helpers borrow the transaction and return pending results; they cannot acquire
another connection or commit. The outer transaction derives the Session actor, checks audited identity
and membership, performs the Task/outbox operation, verifies the target namespace name/OID, restores the
source search path and rechecks source, identity and membership. Its last check uses fresh database time
and the exact Session before confirmed commit publishes success. Transaction-local search-path settings
must not leak into pooled connections after success, failure or cancellation.

Protected reads follow this locking path too. They are not repeatable-read maintenance observations or
grants for later commands. Logout/revocation waits behind an admitted command; a logout that finishes
first prevents later work. Sessions expiring during waits cannot publish a write. The final check does
not promise continuous validity throughout an arbitrarily delayed COMMIT or undo already committed work.

## Failure behavior and retained limits

Task/event atomicity, revision conflicts and terminal-state rules are preserved. Suppressed writes,
deferred commit failures, post-write identity/membership/source changes and search-path replacement
must not publish success. Overall operations allow ten seconds, statements five and lock waits two.
Sanitized errors reveal no SQL, tokens, digests or database paths and never fall back to memory success.

Cancellation, timeout or a lost commit acknowledgment still does not prove rollback. There are no command
receipts, successful replay, recovery or safe automatic retries. Database checks are not a complete schema
fingerprint, full-history reconciliation or protection against a privileged database operator.

The trusted direct `TaskStore` API remains available without a legacy-writer fence. Events gain no new
provenance marker, so historical Task records cannot all be inferred to have used this authenticated
adapter. Direct ArtifactStore and ReviewStore APIs still require trusted attribution; [C2-F](session-artifact-commands.md)
adds a separate Session Artifact adapter, not Review authorization. There is no new migration template,
environment switch, HTTP/CSRF integration, UI, startup hook or provisioning CLI.

## Verification and next boundary

The original C2-E suite includes three default tests covering namespace input, failure without fallback
and absence of detached authentication or live composition. Seventeen explicit PostgreSQL cases cover
private ownership without teaching/deployment roles, unbound/revoked actors, domain-task rejection,
revision races, terminal states, retirement/recreation,
target namespaces, missing audits, suspension/archive, event/commit faults, post-write authority changes,
RLS/temp shadows, a valid cloned-namespace replacement, logout/expiry/revocation ordering, cancellation and
pooled search-path restoration.

`scripts/test-session-task-storage.sh` requires a disposable database and verifies each exact test name
before running it. CI uses the same list; a script regression checks completeness. Fault injection is for
synthetic fixtures only, never deployed data. See [project status](project-status.md) for executed results
and checks not rerun.

[C2-F](session-artifact-commands.md) provides current-Session private Artifact access. [C2-G](session-task-inputs.md) composes exact private inputs with Task commands on one source transaction.
[C2-H](session-review-commands.md) adds private human Review commands with owned exact
evidence. Domain policies, historical writer provenance and Task acceptance remain pending.
Cross-user collaboration needs explicit resource authorization, not an inferred teacher/deployment grant.
Public entry points, offline legacy mapping, trusted-writer
fencing, retention and reliable Run/event delivery remain pending. C2 and C-M2/C-M5 are not complete.
