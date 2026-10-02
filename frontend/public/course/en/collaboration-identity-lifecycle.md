# ADR-005: Audited identity binding and retirement

Date: **2026-09-27**. Status: **independent C1-D staging, included in source release 0.4.5**.
Implemented against **0.4.4**; the original slice's verification below remains historical.
Builds on the [identity registry](collaboration-identity-store.md) and
[policy commands/audit](collaboration-policy-audit.md). No AuthService cutover, deployed database
migration or claim of live-session revocation is made.

## Scope

[Lifecycle commands](../../engine/src/services/collaboration_identity_store/identity_command.rs) add
attributed `Bind` and `Retire` operations. The digest binds command ID, actor, fixed Workspace and full
account coordinates; Principal/binding state and append-only audit commit together. Duplicate requests
return original receipts. Identity schema 1 retains its trusted maintenance behavior; **explicitly
activating identity schema 2** requires commands instead of unattributed writes.

This is internal staging, not public registration or an account-deletion API. A trusted adapter must
authenticate the caller and verify the source `LegacyAccountId`; client-supplied actor, username, role
and UUID fields are not credentials. No password, TOTP, session token/digest, new login protocol,
environment switch or volatile fallback is introduced.

## Why old sessions are not wired in yet

Inspection of the current [authentication service](../../engine/src/services/auth_service/service.inc.rs),
[session path](../../engine/src/services/auth_service/sessions.inc.rs) and
[account mutations](../../engine/src/services/auth_service/account_mutations.inc.rs) confirms:

- Durable users/sessions reference username, without a durable account incarnation. A reused name
  cannot prove that the account is the same one.
- Legacy account/session reads and registration can fall back to process memory. That cache cannot
  establish a current durable identity for this registry.
- Legacy deletion already has a user/session transaction. Calling retirement best-effort after its
  commit would create two write authorities, not an atomic lifecycle.
- PrincipalRef, historical receipts and policy previews do not replace session, CSRF, TOTP or current
  resource-authorization checks.

This slice therefore completes identity-state transactions and management admission only. Synthetic
tests explicitly assert that old `users`/`sessions` are untouched. Retirement denies subsequent access
through the staging layer; it does not end an old Session, close a connection or clean up kernel work.
Live deletion/revocation remains with the existing auth path, pending a unified transaction/failure design.

## Explicit activation

[The upgrader](../../engine/src/services/collaboration_identity_store/identity_audit_schema.rs) requires
C1-C access schema 2 before `upgrade_identity_audit_schema` is explicitly invoked. It upgrades
**identity schema 1 to 2**, not access schema or product version. Released templates remain unchanged;
the new [0009 template](../../engine/migrations/0009_collaboration_identity_audit.sql) is not invoked
by startup, reads or construction.

One transaction locks identity metadata and authorities, verifies a remaining effective manager for each
authority, and records `baseline` entries for every legacy identity, including tombstones. Baselines
preserve observed state without inventing historical actors, command IDs, before-state or retirement reasons.
DDL, baselines and version activation commit together. Concurrent/repeated upgrades verify existing
structure rather than adding baselines or repairing conflicting tables. Missing audit structure or
append-only triggers, invalid snapshots and unknown versions fail explicitly.

The existing ten-second operation, two-second lock and five-second SQL deadlines remain. This small
activation caps authorities and legacy identities at 10,000 each; larger datasets need a reviewed
migration, not partial-success acknowledgement or an in-memory substitute.

After activation, old unattributed binding, retirement and Workspace-provisioning writers return
`IdentityAuditContextRequired`; binaries supporting only identity schema 1 also reject the new version.
Use the read-only lookup for existing Workspaces, not provisioning as a read. New-authority bootstrap,
emergency-manager recovery and general principal/Workspace state commands are not implemented here.

## Commands and state rules

`IdentityCommandId` is a distinct non-nil canonical UUID, not implicitly interchangeable with a policy
command ID. SHA-256 covers a versioned JSON tuple of command ID, actor, Workspace, action and account
coordinates. This encoding is a durable compatibility commitment. IDs are authority-scoped; reuse with
a different actor, action, incarnation or target conflicts.

Every request, including replay, checks current authority: a same-instance active non-retired Human
with an effective explicit deployment grant. Teacher/owner roles alone do not qualify. A retired or
revoked actor cannot retrieve privileged history just by holding an old command ID.

| Action | State and audit behavior |
|---|---|
| Bind | Active Workspace only; verified incarnation creates Principal/binding and `bound` audit, without Membership or Grant |
| Repeat active binding | Reuse the Principal; a new command records `unchanged`, while the original command returns history |
| Retire | Exact username, incarnation and expected Principal; tombstone, disabled Principal and `retired` audit commit together |
| New retirement command for a tombstone | Record `unchanged`, preserve original retirement time, never affect a recreated username |
| Recreate a username | Retire the old binding, supply another incarnation and bind command; allocate a fresh Principal without inherited grants |

Replaying an old Bind after retirement returns its historical receipt, not reactivation or proof of
current identity state. Binding an already retired/disabled incarnation fails. There is no username
inheritance, implicit restoration, unconditional upsert or retry with a newly generated ID. Granting
access to a new subject requires a separate C1-C policy command; binding is not authorization.

## Last manager and concurrent reads

Lifecycle and C1-C policy commands share the authority writer lock. Retirement removes effective
management authority, so it checks `LastAuthorityManager` just like grant revocation. Concurrent
retirement of manager A and revocation of manager B permit at most one success; neither can remove
the remaining effective manager. Membership suspension/Workspace archival do not themselves revoke
independent deployment authority. An archived Workspace can retire a non-last manager but cannot bind users.

Identity schema 2 lookups acquire an authority shared lock before reading identity and audit head.
Child-row locks alone do not protect absent account keys: a regression first reproduced a lookup
skipping an in-flight binding and reporting absence before its audit completed, then fixed the lock order.
The scoped [maintenance instruction](../../.github/instructions/collaboration-store.instructions.md)
captures this requirement without changing other stores' locking protocols.

Reads, effective policy decisions and new commands compare identity state with its audit head;
missing table/history or an unlogged state change cannot produce cached permission. In-transaction
write readback uses raw records until its receipt is appended, avoiding rejection of its own pending
audit. Mutations confirm affected rows and final state; audit failure cannot leave identity changes alone,
and a new Principal is not published before confirmed commit.

These guarantees cover cooperating staging APIs. Privileged SQL/DDL can bypass them; append-only
triggers are not a database-owner-resistant signed ledger. Legacy authentication remains a separate
runtime path, so no system-wide manager or distributed-session recovery guarantee follows.

## Failure, history and live-integration gates

History is restricted to current instance managers and scoped by authority/Workspace/Principal, with
increasing sequence cursors and 1–100 entries per page. Cursors are positive JSON-safe integers, gaps
are allowed, and they are not C3 event offsets. Audit UPDATE/DELETE/TRUNCATE are rejected. Identity
history contains no roles, credentials or private content bodies.

Errors, timeout, cancellation or a lost response can race commit and **do not prove rollback**. Retain
the exact command ID and payload for reconciliation; do not compensate by inventing a new incarnation
or command. After self-retirement, another effective manager must inspect the result.

The remaining lifecycle gate coordinates account creation/deletion and Session revocation with these
internal commands within one reviewed transaction path; the source/composition follow-ups are below.
Joining by username, or committing old tables before best-effort new writes, is insufficient. Real-data
work still requires source inventory, restore verification, cutover approval and rollback boundaries.

Follow-up: [ADR-006 / C1-E](collaboration-auth-source.md), included in 0.4.6, implements an explicit empty-source
account/session adapter with durable incarnations. Also included in 0.4.6, [C1-F](collaboration-session-commands.md)
unifies source verification with binding/policy transactions. Unreleased [C1-G](collaboration-account-deletion.md)
adds restricted administrative deletion/retirement. Full lifecycle and real-data cutover remain pending;
the verification below records the original C1-D slice, not those follow-ups.

## Verification

[Command tests](../../engine/tests/collaboration_identity_audit_tdd.rs),
[permission/lifecycle tests](../../engine/tests/collaboration_identity_audit/permissions.rs) and
[fault injection](../../engine/tests/collaboration_identity_audit/faults.rs) add 16 real PostgreSQL cases,
a default closed-pool test and extended ID contract coverage. Missing APIs, omitted CI selection and
the absent-key read race were observed failing before implementation/fixes.

All 16 new cases and the prior 46 database regressions passed in a separate PostgreSQL 16 cluster with
a private Unix socket and synthetic accounts. Coverage includes upgrade/bind concurrency, digest
conflicts, recreation, historical replay, lost actor authority, last-manager races, first explicit grants,
archival, bounded append-only history, untouched legacy Sessions, missing/corrupt state, baseline rollback,
suppressed/altered receipts, deferred commit failure, pre-commit cancellation and reads waiting for a bind.

[CI](../../.github/workflows/ci.yml) verifies exact names and explicitly executes ignored database cases;
[CI regression tests](../../scripts/tests/postgresCi.test.mjs) reject omissions or empty selectors.
The new CI step's 16 exact-selection commands also passed when executed locally. The complete default
Rust suite passed **331 tests, with 142 ignored**; the 62 database cases above were run explicitly,
while the other 80 ignored tests were not executed in this slice. `quality-gate.sh --format-only`
passed all 75 common regressions, formatting, file lengths, version/document synchronization,
API/SDK contracts and tooling checks. The product version remains 0.4.4.
Browser, live-kernel/LAN and migration/restore acceptance were not run for this slice. Historical release
reports and frozen API/SDK evidence are unchanged.

```bash
# Disposable CYANREX_TEST_DATABASE_URL only; never a deployed DATABASE_URL.
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_identity_audit_tdd --test collaboration_policy_audit_tdd \
  --test collaboration_identity_store_tdd --test collaboration_access_store_tdd \
  -- --include-ignored --test-threads=4
```

The implementation slice did not bump, commit, push or deploy. Its source is now included in 0.4.5;
see [release validation](../../reports/releases/0.4.5/README.md). C1-D supplies registry lifecycle audit,
not completion of C1 or live authentication cutover.
