# ADR-003: Durable legacy membership and deployment policy staging

Date: **2026-09-27**. Status: **implemented as the independent C1-B staging slice**.
Implemented against **0.4.3** and included in source release **0.4.4**. Builds on [ADR-001](collaboration-foundation.md) and
[ADR-002](collaboration-identity-store.md), without switching the live authority path.

## Scope and implementation

[The access store](../../engine/src/services/collaboration_identity_store/access.rs) extends the
explicit identity registry with durable Membership and original-instance deployment-grant state.
[The policy preview](../../engine/src/services/collaboration_identity_store/access_policy.rs) reads
current database records for four migration-sensitive actions. There is no AppState wiring, HTTP
endpoint, startup migration, role import, account/session rewrite or permission change in a running
instance. AuthService and existing route guards remain the only live authentication/authorization path.

This is **not** the general CapabilityGrant engine, an AI delegation facility, a policy-admin API or a
new multi-workspace runtime. Writes require a trusted maintenance caller, an explicitly selected test
or migration database, and the fixed legacy Workspace from C1-A. No environment variable, public
registration, browser field, resource UUID or discovery label can supply that authority.

## Storage and revision fence

[The separate schema template](../../engine/migrations/0007_collaboration_access.sql) adds:

| Table | Invariant |
|---|---|
| `collaboration_access_schema` | Explicit access-schema version 1, independent of product/identity versions |
| `collaboration_memberships` | One authority/Workspace/Principal record with active/suspended status, role set and revision |
| `collaboration_deployment_grants` | One authority/Principal record with active/revoked status, original source Workspace and matching revision |

Composite foreign keys preserve authority and membership scope. The grant's source Workspace records
the migration origin; its effective scope is the original **instance**, not arbitrary workspaces or
other instances. A revoked grant row remains present. Missing grant rows, mismatched revisions,
unsupported schemas or invalid records fail visibly rather than becoming defaults or repaired state.

`install_access_schema` must be invoked explicitly after the identity schema is installed. It uses an
advisory lock and transactional DDL, checks an existing installation without recreating missing tables,
and never imports users or reads secrets. A failed install leaves unrelated preexisting tables intact.
Do not execute the template on a deployed database merely because it is present in the repository.

`replace_legacy_access` atomically replaces membership and explicit deployment state:

1. `expected_revision = None` creates only when no policy exists, at revision 1.
2. An existing policy requires its exact reviewed revision, even when the requested values are equal.
3. An unchanged canonical policy at the current revision is a no-op; an actual change increments both
   rows' revisions together. Revisions are positive JSON-safe integers; exhaustion is an error.
4. Old create requests and stale revisions cannot re-grant access after revocation, suspension or a
   later policy change. There is no unconditional upsert, automatic conflict retry or username-based grant.
5. Every write checks affected rows and reads back the expected state before commit. Suppressed writes,
   altered postconditions and deferred commit failures do not acknowledge success.

The staging role vocabulary is deliberately limited to `cyanrex.teaching.teacher`,
`cyanrex.teaching.learner` and `cyanrex.workspace.owner`. Order is canonicalized; duplicates, unknown
roles, teacher-plus-learner combinations and more than two roles are rejected. Owner may accompany one
teaching role. An empty role set is permitted: active membership still permits access to one's own
private content, but does not grant teaching review or deployment management.

These rows store **current state**, not a complete permission-change audit trail. There is no issuer
authentication, append-only audit, grant expiry, delegation, dynamic role catalog or outbox yet.

## Effective policy and lifecycle boundaries

All four previews require the exact authority, a current non-retired Human account binding and an
active Principal. A raw stored policy is historical/operator evidence, not effective authorization.

| Action | Additional conditions |
|---|---|
| Read private Artifact | Active original Workspace and membership; actor owns the content |
| Restore attempt | Same owner-only rule, including for teachers and Workspace owners |
| Review student attempt | Active Workspace/membership, teacher role; target has an active bound Principal, active membership and learner role in that same Workspace |
| Manage deployment | Explicit active deployment grant for the actor's original authority; no role implies this grant |

The resource adapter must provide verified existence and ownership. There is deliberately no
caller-supplied target `owner_role`: review resolves the target's current membership from the same
database transaction. Teacher access to student attempts never becomes access to all private Artifacts.

Deployment authority is **independent** of workspace membership and archival. Suspending membership
or archiving the teaching Workspace blocks workspace actions, but does not revoke an existing explicit
instance grant. Revoke deployment explicitly; disabling/retiring the Principal blocks both. This preserves
the ability to manage an instance even when its teaching workspace is inactive. A Workspace owner or
teacher role alone has no deployment authority. The eventual legacy migration must explicitly carry
over the verified original teacher's deployment grant; personal-use teacher behavior is unchanged today.

C1-A retirement leaves policy history intact but previews deny the retired/disabled Principal. A
same-name replacement receives a different Principal and has no inherited policy. Reactivation of a
retired incarnation is not offered. Deprivileging to suspended membership plus revoked deployment is
allowed even after retirement or archival; granting active workspace access requires active identity
and Workspace. Granting deployment requires an active identity, independently of Workspace status.

## Concurrency, failure and integration limits

Readers hold a shared authority-row lock; identity/policy mutations hold the corresponding writer lock.
This serializes absent-key races and coordinates policy replacement with identity retirement across
independent pools/processes using this protocol. Relevant identity, Workspace, membership, grant and
schema rows are read under shared locks. It favors a small, explicit staging correctness boundary over
high-throughput provisioning. Direct database repair and future migrations must respect these locks.

The existing limits remain: ten-second whole-call waiting, two-second SQL lock wait and five-second
statement timeout. There is no cache or memory/file fallback. Repairing a backend permits a fresh read,
without a latched allow result. **An error, timeout or cancellation is not proof that no commit occurred.**
Reconcile the exact Principal/policy revision before a separately reviewed retry; do not automatically
turn a freshly read revision into permission to re-grant a revoked privilege.

`preview_legacy_access` returns a point-in-time comparison, not a lease or bearer capability. Its lock
ends before the caller can perform a protected action. A future live adapter must bind authenticated
account incarnation, verified resource ownership, current policy and the protected operation in one
reviewed authority path. Simply calling preview and then performing an unrelated write would leave a
check/use race. Existing session, CSRF, password/TOTP and resource-lifecycle requirements still apply.

## Verification

[Store/concurrency tests](../../engine/tests/collaboration_access_store_tdd.rs),
[privacy/lifecycle tests](../../engine/tests/collaboration_access_store/policy.rs) and
[fault injection](../../engine/tests/collaboration_access_store/faults.rs) cover 15 real PostgreSQL cases
plus a default closed-pool failure test. They reuse random, disposable schemas and never select the
application's `DATABASE_URL`. All passed against a newly initialized PostgreSQL 16 cluster with a private
Unix socket and no TCP listener. The preceding 15 durable identity cases were also rerun successfully.

Coverage includes concurrent installation/create/update, independent reload, stale re-grant rejection,
teacher/student/owner privacy, role changes, suspension/archival, cross-authority scope, retirement and
same-name recreation, schema rollback, corrupt state, suppressed/altered writes, deferred commit failure,
revision exhaustion, pre-commit cancellation and a reader waiting behind a revocation transaction.

CI explicitly selects each durable test and verifies that the exact test exists before executing it;
[the CI regression](../../scripts/tests/postgresCi.test.mjs) rejects omitted cases or empty selectors.
No remote CI run or release was triggered by local testing.

The full default Rust suite passed with **329 passed and 110 ignored**. Thirty of those ignored cases
(15 identity + 15 access) were explicitly executed against the isolated database; the remaining 80
ignored cases were not run in this slice. The format-only quality gate passed, including **73 common
checks**, file limits, version metadata, documentation mirrors, OpenAPI/SDK compatibility and Rust
formatting. No browser, live-kernel, LAN, deployment migration or backup-restore acceptance is claimed.

```bash
# CYANREX_TEST_DATABASE_URL must point to a disposable database, never a running instance.
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_access_store_tdd --test collaboration_identity_store_tdd -- --include-ignored
```

## Remaining C1 work

- Persist/verify stable account incarnations in the selected AuthService lifecycle and coordinate
  creation, deletion, sessions and policy under a reviewed single-writer cutover. The legacy memory
  fallback is not a durable identity provider and must not silently dual-write into these tables.
- Add authenticated policy management, permission-change audit, general Workspace/Principal state
  commands and the broader grant model before exposing this as live authorization.
- Complete deployment inventory, restored-backup validation, source reconciliation and explicit cutover
  approval before touching real accounts; this synthetic registry is not a production backup or migration.
- Integrate protected operations with policy at the transaction/admission boundary, then validate legacy
  login, teacher/private-content access, session revocation and cross-space failures end to end.

C1-B advances staging storage and comparison; it does not complete C1 or change product version.
