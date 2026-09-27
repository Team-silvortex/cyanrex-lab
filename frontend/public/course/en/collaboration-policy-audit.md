# ADR-004: Attributed policy commands and transactional audit

Date: **2026-09-27**. Status: **independent C1-C staging, included in source release 0.4.5**.
Implemented against **0.4.4**; the original slice's verification below remains historical.
Builds on [ADR-003](collaboration-access-store.md); no live authentication or deployed database cutover.

Follow-up: [ADR-005 / C1-D](collaboration-identity-lifecycle.md) extends attributed commands to identity
binding/retirement through a separate identity schema 2 upgrade. Verification below records C1-C itself.

## Scope and trust boundary

[Policy commands](../../engine/src/services/collaboration_identity_store/policy_command.rs) add actor,
request ID and expected revision to changes within the fixed legacy Workspace.
[The audit store](../../engine/src/services/collaboration_identity_store/policy_audit.rs) commits receipts
with policy. No HTTP endpoint, AppState wiring, startup migration, environment switch, cache or volatile
fallback is added. Live teacher authority, personal mode, private ownership and AuthService are unchanged.

`LegacyPolicyCommand.actor` must come from a trusted adapter resolving an authenticated, non-revoked
account incarnation. **PrincipalRef is not a credential; knowing a manager UUID grants no authority.**
This layer checks the specified subject's current durable permissions, not session authentication.
Never pass a browser-supplied actor directly to it. Live integration must retain session, CSRF, TOTP,
account deletion/revocation and protected-operation admission in one reviewed authority path.

## Explicit upgrade and observed baselines

[The upgrader](../../engine/src/services/collaboration_identity_store/policy_audit_schema.rs) exposes
`upgrade_policy_audit_schema`, upgrading **access schema 1 to 2**, not identity schema or product version.
The released [0007 template](../../engine/migrations/0007_collaboration_access.sql) is unchanged;
the new [0008 template](../../engine/migrations/0008_collaboration_policy_audit.sql) runs only explicitly.

- First verify C1-A account incarnations and C1-B grants for original teachers. Every existing authority
  needs at least one non-retired active Human with an explicit deployment grant. Empty/unprepared
  authorities fail activation; no superuser is invented. This is not an account bootstrap interface.
- One transaction locks access metadata and authorities against policy/identity changes and authority
  creation during the snapshot. Each policy gets one `baseline` retaining its observed revision and
  canonical state. Actor, command ID, digest and before-state remain absent, not invented history.
- DDL, all baselines and the version update commit together. Conflicting tables, invalid policies,
  missing structure and unknown versions fail without automatic repair. Repeated/concurrent upgrades
  never add another baseline batch. Version 2 checks audit structure and append-only triggers, not a downgrade.
- This small staging migration caps authorities and policies at 10,000 each, within the existing
  ten-second operation, two-second lock and five-second SQL limits. Exceeding limits is not partial
  success; a large, batched migration needs a separate design.
- Schema 2 rejects old `replace_legacy_access` calls with `AuditContextRequired`. Old binaries accepting
  only schema 1 also reject version 2, preventing unaudited writes. Schema 1 behavior and tests remain.

This layer does not yet bootstrap new authorities after activation, recover an emergency administrator
or manage general Workspaces. Never upgrade a live database without deployment inventory, restore
verification and explicit approval, or bypass commands by changing the version marker/using old writers.

## Commands, idempotency and management authority

`PolicyCommandId` is a distinct non-nil canonical UUID type, retained before the first request and
reused on retry. SHA-256 covers a versioned canonical JSON tuple of command ID, complete actor,
expected revision and complete desired policy; role order is canonicalized. The digest version and
encoding are durable compatibility commitments: later changes must preserve old receipt decoding.

One authority writer lock/transaction performs:

1. Fixed Workspace/schema validation, followed by current actor authority: same authority, active
   non-retired Human binding and explicit deployment grant. Teacher/owner roles alone do not qualify.
2. Lookup of the command ID within that authority. Equal digest returns the original historical receipt
   with `replayed=true`, without writing policy. A different actor, expected revision, target or value
   returns `CommandConflict`. Command ID namespaces are independent between authorities.
3. For a new command, `None` means create only if absent; otherwise the exact revision must match.
   Both policy rows change together and only real changes increment revision. Equal-state commands
   still append an `unchanged` receipt, retaining durable idempotency without incrementing revision.
4. Revoking the last effective instance manager's deployment grant fails with `LastAuthorityManager`.
   Self-revocation requires another effective manager. Membership suspension/workspace archival alone
   do not revoke independent deployment authority; explicit revocation/identity disabling retain their semantics.
5. Append of before/after states, actor, command ID, digest and database recording time; readback of
   both policy and receipt; then commit. Suppressed/altered writes or commit failures cannot acknowledge
   a successful policy change without its audit record.

**Current actor authority is checked before replay.** After self-revocation, even a committed command
ID cannot recover privileged history; a still-authorized manager must reconcile it. Replaying an old
grant after subsequent revocation or account retirement returns history only, never restoring access
or assigning it to a recreated username. A receipt is not evidence of currently effective permissions.

**Errors, timeout and cancellation can race commit; they do not prove rollback.** Retain the original
command for reconciliation, never generate a new ID or automatically adopt a newer revision to regrant.
Previews are not authorization leases. Cancellation tests stop at a known pre-commit blocking point;
they do not establish rollback at every possible cancellation time.

## Audit reads and integrity limits

`legacy_policy_audit` is an internal history read restricted to current instance managers, scoped by
authority / Workspace / Principal. Keyset cursors follow increasing sequence, with 1–100 entries per
page and positive JSON-safe cursors. Gaps are allowed; this is not a C3 cross-authority commit offset
or an exactly-once guarantee for external actions.

Schema 2 policy reads, permission previews and new commands compare current policy to its latest audit
snapshot. Missing history/table, inconsistent state/revision, unknown fields, invalid scopes and malformed
snapshots fail rather than using a cached allow. Repaired storage can be read again. Snapshots contain
permission metadata, never passwords, sessions, TOTP or private content bodies; database/log access
still requires protection.

Constraints and triggers reject audit UPDATE, DELETE and TRUNCATE; application code only appends.
This is **not database-owner-resistant WORM or a signed ledger**: privileged DDL, disabling triggers,
changing functions or direct table writes remain controlled database-maintenance operations. There is
no public export, general business outbox, audit retention policy or full-history cryptographic verification.

In the original C1-C slice, the last-manager guard covers this policy command only. Follow-up
[C1-D](collaboration-identity-lifecycle.md) also guards retirement and fences unattributed identity
writes after the explicit identity schema 2 upgrade. Live authentication and privileged SQL maintenance
remain outside this protocol; it does not guarantee that every system path preserves a manager.

## Verification and next gate

[Command/concurrency tests](../../engine/tests/collaboration_policy_audit_tdd.rs),
[permission/scope tests](../../engine/tests/collaboration_policy_audit/permissions.rs) and
[fault injection](../../engine/tests/collaboration_policy_audit/faults.rs) add 16 real PostgreSQL cases,
a default closed-pool test and extended ID contract checks. Missing APIs were observed failing before
implementation; the CI-selection regression also failed before wiring in the new cases.

All 16 audit cases plus the prior 15 identity and 15 access cases passed against a new PostgreSQL 16
cluster using a private Unix socket, random schemas and synthetic accounts; no live database was read.
Coverage includes concurrent deduplication/revision conflicts, cross-authority isolation, first grants,
retirement/recreation, unattributed-write fencing, last-manager protection, bounded append-only history,
baseline rollback, missing tables, inconsistent state, suppressed/altered receipts, trigger-altered policy,
deferred commit failure and cancellation during receipt insertion.

[CI](../../.github/workflows/ci.yml) verifies each exact test name before explicitly running ignored
database cases; [CI regression tests](../../scripts/tests/postgresCi.test.mjs) reject omissions and empty
selectors. The full default Rust suite and format-only quality gate also passed. Browser, live-kernel,
LAN and migration/restore acceptance were not run for this slice; synthetic tests are not deployment approval.

The full default Rust suite recorded **330 passed and 126 ignored**; 46 database cases were explicitly
run separately, leaving 80 ignored cases unexecuted in this slice. All **74 common checks** passed;
version remains 0.4.4 and frozen OpenAPI/SDK baselines are unchanged.

```bash
# Only a disposable CYANREX_TEST_DATABASE_URL; never a running instance's DATABASE_URL.
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_policy_audit_tdd --test collaboration_access_store_tdd \
  --test collaboration_identity_store_tdd -- --include-ignored --test-threads=4
```

Next select durable account-lifecycle authority, coordinate creation/deletion/session revocation with
attributed management, and design a single-writer cutover. General grants, principal/workspace state
commands, live check/use integration and real-source reconciliation remain pending. C1-C is not all of C1.
The implementation slice did not release, commit, push or deploy. Its source is now included in 0.4.5;
see [release validation](../../reports/releases/0.4.5/README.md). Historical 0.4.4 evidence is unchanged.
