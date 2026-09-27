# ADR-007: Session-authorized collaboration transactions

Date: **2026-09-27**. Status: **C1-F internal composition, included in source release 0.4.6**.
Builds on the [durable authentication source](collaboration-auth-source.md),
[identity lifecycle commands](collaboration-identity-lifecycle.md) and
[policy audit](collaboration-policy-audit.md). No live AuthService, route or deployment is switched.

## Delivered boundary

[The source adapter](../../engine/src/services/auth_service/durable_source/session_commands.rs) now
composes session verification, actor resolution, source-account verification, registry mutation and
audit on **one connection and one PostgreSQL transaction**. It never validates a detached Session and
then invokes a separate committing store method. No second pool or caller-selected actor is accepted.

| Entry point | Requested fields | Server-owned checks and mutation |
|---|---|---|
| `bind_session_account` | Token separately; command ID, Workspace and exact target account reference | Resolve current manager from Session; verify source incarnation; bind and audit without granting permissions |
| `apply_session_policy_command` | Token separately; command ID, reviewed revision and desired policy | Resolve current manager and target source account; apply revision-fenced policy and audit with last-manager protection |

Both require explicitly installed source schema 1, identity schema 2 and access schema 2 in the same
database/schema. Scope must match the source's pinned authority and existing legacy Workspace. There is
no implicit install, bootstrap, import or upgrade. The synthetic fixture deliberately provisions initial
managers before audit activation using IDs returned by real source registration; that is not a new
production bootstrap API or permission to adopt existing accounts.

The adapter checks the canonical token against durable, unexpired Session/account rows, then resolves
that exact username/incarnation through the audited identity registry. The actor must be an active,
non-retired Human with a current explicit deployment grant. A username, teacher role, caller-built
PrincipalRef, old receipt or cached Session cannot substitute. An unbound registered account cannot
bind itself or become a manager. Targets must also match current source accounts; no credentials are
read for target-policy checks, and no token or token digest enters identity/policy audit.

## Transaction and lock order

Commands hold the source metadata row `FOR SHARE`, followed by registry identity/access metadata,
the authority writer lock, and account/session/registry child rows. All are retained until commit or
rollback. This extends the existing registry metadata-before-authority order and prevents empty-key
races. Source register/login/logout writers acquire `FOR UPDATE`, so they wait behind admitted commands;
commands admitted after a committed logout cannot reuse its Session. No lock upgrade is performed.

The [registry composition helpers](../../engine/src/services/collaboration_identity_store/session_adapter.rs)
and extracted identity/policy mutation helpers are crate-private and accept a borrowed SQL transaction.
They return a **pending** receipt, never commit independently. Existing trusted-adapter store APIs
wrap the same implementations with their own transaction/commit; command digest formats and receipt
semantics remain unchanged. The source adapter checks target coordinates and the exact Session again
after mutation/audit, then requests commit and only returns after confirmation.

Fresh database time is checked after registry lock waits and again before requesting commit. A Session
that expires while the command waits in audit is rejected and its pending writes roll back. This is
not continuous authorization or a promise that a delayed COMMIT finishes before wall-clock expiry.
Source/policy revocation follows transaction order: logout cannot undo an already committed command.
The ten-second operation, two-second lock and five-second SQL deadlines remain.

The scoped [source instructions](../../.github/instructions/durable-auth-source.instructions.md) and
[registry instructions](../../.github/instructions/collaboration-store.instructions.md) record lock
order and pending-receipt ownership. Registry-first code must not acquire the source lock later.

## Replay, source identity and last manager

Current Session and manager checks apply even when replaying an existing command ID. Existing
actor/scope/payload digests, revision conflicts and exact-ID deduplication are reused. Replay returns
history, not re-granting or proof of current state; this adapter additionally requires the target source
incarnation still to exist. Same-name account recreation cannot select its old Principal or grant.
After self-revocation, the original caller cannot recover management authority by replaying a receipt.

A regression exposed an integration-specific last-manager hole: the registry alone counted a second
manager even after its source account was deleted, permitting the last source-backed manager to revoke
itself. A same-name replacement was equally insufficient. The adapter now checks, **inside the same
transaction after a new grant revocation**, that at least one remaining audited active manager still
matches a durable source account. Failure rolls back policy and receipt together. Candidate enumeration
is capped at 10,000; larger/inconsistent state fails rather than assuming a fallback manager.

This is an account-existence/incarnation safeguard, not proof that someone possesses that account's
credentials or has an active Session. Standalone trusted registry commands remain staging tools with
their existing registry-only guard; they are not silently turned into authenticated APIs or fenced by
this adapter. Privileged SQL/DDL can bypass source and registry protocols. Full lifecycle integration
must close those separate writer paths before a live cutover.

## What remains deliberately unavailable

- Account registration plus Principal/initial-policy bootstrap in one transaction.
- Account deletion plus Session revocation and identity retirement in one transaction.
- Password/TOTP changes, credential reconciliation and emergency manager recovery.
- Session-authorized audit-history/ordinary-resource APIs, HTTP/CSRF/UI wiring and bounded public admission.
- Real-account migration, old-runtime writer fencing, reviewed cutover/rollback and deployment acceptance.

The adapter exposes binding and policy commands only, not a misleading half-integrated Retire/delete
operation. The existing live teaching/teacher-authority workflow, Session fallback, API/SDK contracts,
kernel execution and running services remain unchanged. Historical registry retirement still does not
revoke old Sessions; ordinary durable source logout does not retire a Principal.

Cancellation, timeout or loss of a commit response **does not prove rollback**. Keep the exact command
ID and payload for reconciliation; retry only with a still-valid manager Session, never invent a new ID
or use a historical permission result. Pre-commit fault tests do not prove commit-race recovery.

## Verification

[Integration tests](../../engine/tests/durable_collaboration_tdd.rs) and
[fault tests](../../engine/tests/durable_collaboration/faults.rs) add 16 explicit PostgreSQL cases and one
default closed-pool case. Missing interfaces, omitted CI selection and the source-less-manager gap were
observed failing before implementation/fixes. Coverage includes explicit activation, exact source
incarnations, no implicit grants, current manager/replay checks, expired/revoked/recreated Sessions,
foreign scope, last manager, concurrent replay, missing audit, suppressed writes, deferred commit
failure, cancellation, both logout/command orderings, expiry after waits and post-write source checks.

Local verification used a disposable PostgreSQL 16 cluster with a private Unix socket and synthetic
accounts. All **114 database cases** passed: 16 new composition cases, 76 prior source/registry cases
and 22 legacy authentication cases. The full default Rust suite passed **334 tests, with 172 ignored**;
114 were run explicitly above, while the other 58 ignored cases were not executed in this slice.
The new CI step's 16 exact-selection commands also passed locally. `quality-gate.sh --format-only`
passed **77 common tests**, formatting, file lengths, version/document synchronization, API/SDK
contracts and tooling checks. Browser/frontend builds, real kernel/LAN, live dependency audits and
deployment/migration/restore acceptance were not run. Tool checks do not establish actual artifact acceptance.

[CI](../../.github/workflows/ci.yml) lists and executes each ignored case by exact name; the
[CI regression](../../scripts/tests/postgresCi.test.mjs) rejects missing or empty selections.
Use a disposable `CYANREX_TEST_DATABASE_URL`, never a deployed `DATABASE_URL`:

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test durable_collaboration_tdd -- --include-ignored --test-threads=4
```

The implementation slice was verified against 0.4.5 without a release, live migration or deployment.
Its source is now included in 0.4.6; see [release validation](../../reports/releases/0.4.6/README.md).
Prior slice verification and released reports remain historical; frozen API/SDK baselines are unchanged.
