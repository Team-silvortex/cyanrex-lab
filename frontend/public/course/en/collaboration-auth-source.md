# ADR-006: Durable account incarnations and session source

Date: **2026-09-27**. Status: **C1-E internal staging, included in source release 0.4.6**.
Builds on [audited identity lifecycle commands](collaboration-identity-lifecycle.md). This is not an
AuthService cutover, existing-account migration, public endpoint or deployment change.

Follow-up: [ADR-007 / C1-F](collaboration-session-commands.md) composes current-session verification
with binding/policy commands, without exposing deletion/retirement or switching live authentication.
The source-only scope and verification below describe the original C1-E slice.

## Decision and scope

The [durable authentication source](../../engine/src/services/auth_service/durable_source/mod.rs)
accepts an injected PostgreSQL pool and an explicitly pinned `AuthorityId`. It has no environment
configuration, seeded teacher, in-memory credential/session store or database-failure fallback. One schema
belongs to one source authority; opening it with another authority fails rather than creating a new one.

Registration commits a random non-nil `LegacyAccountId` alongside username and credentials. Sessions
reference **both username and that exact account incarnation**. A deleted and recreated name receives
another incarnation, so it cannot inherit an old session or registry identity. User/session coordinates
and session digests cannot be updated in place through the installed schema.

| Internal operation | Confirmed result and boundary |
|---|---|
| `install_empty_schema` | Explicit installation on absent or compatible empty auth tables only; no account import/backfill |
| `register` | New account incarnation and TOTP setup returned after confirmed commit; no Principal, role, Membership or Grant |
| `login` | Password and TOTP verification, current-incarnation recheck and committed session; only its SHA-256 token digest is stored |
| `validate_session` | Fresh read-only snapshot of the exact unexpired account/session pair; no cleanup, role lookup or allocation |
| `logout` | Confirmed removal of this exact session, idempotent when already absent; not account retirement or device/kernel cleanup |

These are trusted adapter primitives. In particular, registration is not public enrollment or teacher
bootstrap. `DurableAccountRef` and `DurableSession` are forgeable data/snapshots, **not an authorization
capability** for a later identity or policy command. No routes, AppState fields or existing AuthService
behavior are switched. The existing HTTP/SDK surface and teacher/student workflow remain unchanged.

## Explicit installation, never inferred migration

[The installer](../../engine/src/services/auth_service/durable_source/schema.rs) serializes explicit
installation using a schema-scoped advisory lock. It locks both auth tables before checking emptiness,
verifies legacy column types, NOT NULL constraints, primary keys and cascading foreign-key targets,
then applies the new [0010 template](../../engine/migrations/0010_durable_auth_source.sql) and records
source schema version 1 with its pinned authority. DDL, constraints and activation commit together.
Released templates 0001–0009 are unchanged; startup and reads do not invoke this installer.

Nonempty old tables are rejected without assigning IDs, rewriting credentials or invalidating sessions.
An empty table with a missing session primary key is also incompatible: successful column projection
does not prove uniqueness. A failing regression exposed this gap; catalog checks now reject it along
with missing foreign keys, nullable credentials and wrong timestamp types. The scoped
[maintenance instruction](../../.github/instructions/durable-auth-source.instructions.md) preserves this rule.

Repeat installation verifies existing structure, immutable-coordinate triggers, version and authority;
it does not repair missing state. New account coordinates have no default, so legacy inserts omitting
them fail at the SQL boundary. **This does not fence the old in-memory AuthService or every old writer**:
the old runtime can fall back, mutate credentials or delete rows. Never point both implementations at
the same activated schema. A real cutover needs a stopped/fenced old writer and a reviewed migration.
Schema checks and triggers protect cooperating code, not against a privileged database owner.

## Transactions and authentication

Source mutations take the metadata row `FOR UPDATE`; coherent reads take `FOR SHARE` before account
and session rows, including absent keys. This deliberately serializes writes per source across pools.
A session read waits for a pending logout, then observes its committed deletion rather than a cache.
The operation deadline is ten seconds, lock wait two seconds, SQL statement five seconds.

Password hashing/verification reuses the existing Argon2 helpers off the async worker and does not hold
a database transaction during expensive verification. Login reads credentials, verifies password/TOTP,
then reacquires source/row locks and compares the full credential snapshot and exact incarnation before
inserting a session. It checks them again after insertion, since triggers can modify rows in the same
transaction. Inserts/deletes confirm affected rows and final readback before commit acknowledgement.
No account ID, setup secret or raw session token is returned before confirmed commit; secret-bearing
registration/login wrappers intentionally implement neither Debug nor Serialize.

Sessions expire twelve hours after issuance using fresh database time after lock waits. Reads check
fresh database time after their row locks, not transaction-start time. Expired sessions are rejected but
not silently deleted on reads. Cancellation, timeout and a lost commit response still **do not prove
rollback**. An uncertain registration is not retried with another identity; this slice has no durable
command receipt, secret redisclosure or automatic compensation/reconciliation API.

Login admission is bounded but **process-local**: clones share at most 1,024 username entries, five
admitted attempts per username in a fixed five-minute window. Admitted failures, including storage
errors, consume attempts; a confirmed login clears its entry. New independent source instances do not
share limits. Password input is capped at 4,096 bytes and OTP at 64 bytes; registration requires at least
eight password bytes. Distributed rate limits, public registration policy, global password-worker
backpressure and HTTP/CSRF integration remain gates before exposing this adapter to untrusted clients.

## Next integration gate

The source establishes the durable incarnation needed by C1-D, not its live composition. Next design
one transaction/lock order for current session verification, registry binding/retirement, policy/audit
and credential/session lifecycle. Do not commit auth changes and then best-effort write the registry.
The source deliberately exposes no password change, account deletion, teacher bootstrap or permission
mutation until their last-manager and revocation effects can be coordinated. Existing C1-D retirement
still does not revoke legacy Sessions, and logout here does not retire a Principal.

Real-data adoption separately requires source inventory, restore-tested backup, explicit migration
mapping, writer fencing, cutover approval and rollback/reconciliation boundaries. No deployed database,
live server, account or role was changed by this slice.

## Verification

[Source regressions](../../engine/tests/durable_auth_source_tdd.rs) and
[fault injection](../../engine/tests/durable_auth_source/faults.rs) provide 14 explicit PostgreSQL cases,
plus a default closed-pool test and a bounded/shared-admission unit test. Missing APIs, omitted CI
selection and incompatible-empty-schema adoption were observed failing before implementation/fixes.
Coverage includes concurrent installation/registration, authority mismatch, restart/recreation,
password/TOTP, hashed sessions, expiration/logout, immutable coordinates, wrong incarnations,
missing/unknown storage, suppressed/altered writes, deferred commit failure, pre-commit cancellation,
logout/read ordering and credential/generation changes during session insertion.

Local verification used PostgreSQL 16 with a private Unix socket and synthetic accounts: all 14 new
cases, 62 preceding collaboration cases and 22 legacy authentication cases passed (**98 database
tests**). The full default Rust suite passed **333 tests, with 156 ignored**; the 98 database cases were
run separately, while the other 58 ignored cases were not run in this slice. The new CI step's 14 exact
selection commands also passed locally. `quality-gate.sh --format-only` passed **76 common tests**,
formatting, file lengths, version/document sync, API/SDK contracts and tooling checks.
Browser, frontend-build, live-kernel/LAN, deployment/restore acceptance and live dependency audits were
not run for this slice. Tooling smoke checks are not actual kernel or artifact acceptance.

[CI](../../.github/workflows/ci.yml) lists and runs all 14 ignored cases by exact name; the
[CI regression](../../scripts/tests/postgresCi.test.mjs) rejects omissions or empty selectors. Use only a
disposable database via `CYANREX_TEST_DATABASE_URL`, never the deployed `DATABASE_URL`.

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test durable_auth_source_tdd -- --include-ignored --test-threads=4
```

The implementation slice was verified against 0.4.5 without a release or deployment. Its source is now
included in 0.4.6; see [release validation](../../reports/releases/0.4.6/README.md). Historical reports
and frozen API/SDK evidence remain unchanged. This source release does not cut over live authentication.
