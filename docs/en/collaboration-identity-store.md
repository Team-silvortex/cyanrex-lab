# ADR-002: Durable collaboration identity staging

Date: **2026-09-27**. Status: **implemented as an explicit, independent C1-A registry**.
Implemented against **0.4.3** and included in source release **0.4.4**, without live migration.
[ADR-001](collaboration-foundation.md) remains the contract baseline.

Follow-up: [ADR-003 / C1-B](collaboration-access-store.md) adds separately installed membership and
deployment-policy staging. The C1-A scope and verification below describe the original identity slice;
neither slice is wired into live authentication or protected operations.

Follow-up: [ADR-005 / C1-D](collaboration-identity-lifecycle.md), included in 0.4.5, adds an explicit identity
schema 2 upgrade, attributed binding/retirement and lifecycle audit. The original schema 1 behavior
and verification below remain historical; neither schema is wired into live sessions.

## What this slice implements

[CollaborationIdentityStore](../../engine/src/services/collaboration_identity_store/mod.rs) persists
operator-pinned authority/legacy Workspace IDs, Human Principals and legacy account-incarnation
bindings in PostgreSQL. It can provision, look up, bind and retire those records. Repeated operations
reuse committed IDs; independent store instances coordinate through database transactions and locks.

This is **staging infrastructure, not active authentication or authorization**. It is not constructed
by AppState, exposed by HTTP, or called from Engine startup. It does not read legacy users, sessions,
passwords, TOTP secrets or deployment configuration. No running deployment or existing database was
migrated. There is no new grant, membership, login cookie, deployment privilege or background writer.

## Account identity is not a username

The trusted caller supplies three independent coordinates:

| Coordinate | Purpose | Must not be inferred from |
|---|---|---|
| Authority + fixed legacy Workspace | Select the verified source instance and its teaching boundary | Hostname, filesystem path, discovery labels or browser fields |
| Canonical username | Preserve the legacy login/lookup name | Unreviewed normalization of conflicting source entries |
| `LegacyAccountId` | Identify one incarnation of that account | Password/TOTP digests, username hash, import time or an assumed creation timestamp |

`LegacyAccountId` is a new typed non-nil UUID. It must be stable across repeated imports/retries, and
must change when an account is deleted and recreated. The registry does **not** invent it while reading
an old account. C1's future authentication adapter must persist and verify it as part of account lifecycle
transactions. For ambiguous historical identity, stop for reconciliation instead of guessing.

`LegacyIdentityBinding` retains the C-M1 shape; `StoredLegacyIdentity` adds the account incarnation,
Principal state and optional retirement timestamp. Lookups include disabled/retired history for
operators. A returned record is not an authorization decision or proof of a currently authenticated user.

## Storage layout and installation

[The additive schema template](../../engine/migrations/0006_collaboration_identity.sql) defines:

| Table | Invariant |
|---|---|
| `collaboration_identity_schema` | One version row; only identity schema version 1 is understood |
| `collaboration_authorities` | Durable authority UUID, distinct from the existing instance label |
| `collaboration_workspaces` | Workspace identity/status within an authority |
| `collaboration_legacy_workspaces` | At most one fixed legacy Workspace for each authority |
| `collaboration_principals` | Scoped Principal identity/kind/status, independent of login name |
| `collaboration_legacy_identities` | One binding per authority/name/incarnation; at most one active binding per authority/name |

Composite foreign keys keep authority and local IDs together. Each account incarnation and Principal
can occur only once in the legacy identity bindings of its authority. IDs cannot be nil. The partial
unique index on active usernames allows a new incarnation only after explicit retirement of the old one.
No table has a foreign key to legacy `users`; connecting that lifecycle remains an explicit later step.

The constructor only retains an injected pool. `install_schema` is a separate operation on an explicitly
selected trusted database/schema. Its transaction serializes installation with a schema-scoped advisory
lock. The version row is written with the tables; any failure rolls the transaction back. Repeating an
installed version verifies expected columns rather than recreating missing tables. Unknown versions or
missing schema on a normal read fail visibly, without automatic repair, fallback or DDL.

Never execute this template against a deployed database merely because it exists in the repository.
Production adoption still requires the C0 deployment inventory, tested backups, selected source identity,
maintenance/cutover plan and authorization. Existing startup schema paths remain unchanged.

## Operation boundaries

1. **Provision:** supply the exact Authority/Workspace pair once. Repeating it returns the same mapping.
   A different Workspace for an already-bound authority conflicts without creating an orphan Workspace.
   A previously archived Workspace is never reactivated by provisioning or binding.
2. **Bind:** provide the same verified incarnation for retries. Existing active bindings return the same
   Principal. A competing active incarnation or reuse of the incarnation under another username fails.
   Principal creation and binding insertion commit together; no unconfirmed ID is published.
3. **Retire:** require the reviewed authority, Workspace, username, incarnation and expected Principal ID.
   The binding's tombstone and Principal's disabled status commit together. Repeating an old retirement
   does not touch a newly created same-name account. Retirement never deletes historical records or
   reassigns their ownership. It also does not delete legacy sessions or revoke live access yet.
4. **Recreate:** a new incarnation receives a new Principal ID. The old incarnation cannot be rebound,
   even after its username is reused. No roles or grants are inherited or issued by this registry.
5. **Read:** does not allocate, migrate, normalize or change records. Invalid types or unknown enum states
   in expected columns fail decoding; a retired binding pointing to an active Principal is rejected as
   inconsistent. A disabled non-retired Principal cannot be silently reactivated by binding again.

Identity mutations currently serialize on the authority row, including first-bind races for absent
usernames. This deliberately favors a simple correctness boundary for staging over high-throughput
account provisioning. It is not an in-process lock; separate pools/processes using this store share it.
Ordinary lookups use shared locks, not the authority writer lock. The schema version is held stable for
each transaction. Future migrations must honor that transaction/locking boundary.

All mutations check affected-row counts and read back the intended state before commit. Storage query,
row decode, suppressed write, deferred commit and pool errors cannot return success. The store has no
memory/file cache and does not inspect `CYANREX_DB_FALLBACK`. A repaired backend can be queried again
without a latched fallback mode.

Each call has a 10-second caller deadline; SQL transactions use a 2-second lock wait and a 5-second
statement timeout. **An error, cancellation or lost acknowledgement is not proof that a commit did not
happen.** Reconcile by looking up the same exact mapping/expected Principal before an explicit retry;
do not invent a new incarnation to bypass an uncertain result. Tests cover cancellation before commit,
not a guarantee of rollback when commit already won the race.

## Verification

[Integration cases](../../engine/tests/collaboration_identity_store_tdd.rs) and
[fault injection](../../engine/tests/collaboration_identity_store/faults.rs) run against isolated,
randomly named PostgreSQL schemas. The fixture never selects the normal application's `DATABASE_URL`.
The CI engine job explicitly enumerates all 15 durable cases, verifies each test exists before running
it, and uses its disposable PostgreSQL service. [A tooling regression](../../scripts/tests/postgresCi.test.mjs)
rejects missing cases or accidental empty selectors.

```bash
# CYANREX_TEST_DATABASE_URL must select a disposable test database, never a live instance.
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_identity_store_tdd -- --include-ignored
```

Local verification used a newly initialized PostgreSQL 16 cluster with a private Unix socket and no
TCP listener. All 15 durable cases and the default closed-pool failure case passed, including:

- concurrent schema/provision/bind, fresh-pool reload and competing account incarnations;
- cross-authority scope, expected-Principal mismatch, archived/disabled state and inconsistent rows;
- retirement/recreation, repeat retirement, old-generation replay, no orphan bootstrap rows;
- suppressed insert/update, ordinary SQL failure, deferred bind/retire commit failure and pre-commit
  cancellation, with successful retries after the injected fault is removed;
- absent/unsupported/broken schemas, read-only absence, and no successful memory fallback.

The full default Rust suite passed with **328 passed and 95 ignored**. The 15 durable cases above are
among those default-ignored tests and were explicitly run separately; the other 80 ignored tests were
not exercised in this slice. The format-only quality gate also passed, including **72 common checks**,
documentation mirrors, version consistency, OpenAPI/SDK compatibility and Rust formatting. CI coverage
was configured and checked locally; no remote CI run or release was triggered.

These tests do not establish deployed account lifecycle integration, membership/Grant enforcement,
session revocation, outage reconciliation, database backup restoration or production migration acceptance.
The synthetic database is not a backup of an existing instance.

## Remaining C1 gates

- Verify real instances, selected durable/fallback sources and restore-tested backups before importing
  real accounts. Preserve/reconcile source ownership; don't infer an old identity from a same-name login.
- Persist the legacy account incarnation and coordinate creation/deletion with AuthService in one
  reviewed authority path; plan the single-writer migration and rollback boundary first.
- Add durable Membership/Grant lifecycle and policy checks for Principal disabled state, workspace
  archival, membership suspension, revocation, private content and cross-space access.
- Only then attach the legacy adapter to authenticated requests, maintaining old session/TOTP/CSRF and
  personal teacher authority. Workspace ownership alone must still not grant instance deployment rights.

This slice advances C1-A, not all of C1, and does not change product version or publish a release.
