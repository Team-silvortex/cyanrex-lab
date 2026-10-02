# ADR-008: Restricted session-authorized account deletion

Status: **C1-G restricted internal staging, included in 0.4.7**, originally based on 0.4.6 commit
`0dc195086965f6ca6972c742a36f3c825bc6be41`. Builds on [C1-F](collaboration-session-commands.md).
No live AuthService, public API/SDK, old migration, automatic account adoption or deployment change.

## Invariants mapped before choosing the slice

| Component | Existing invariant | Deletion consequence |
|---|---|---|
| Auth source (C1-E) | Committed, immutable username/incarnation; Sessions reference both; source metadata fences absent keys | Verify the exact source account; remove every matching Session and account in the same transaction |
| Identity registry (C1-A/D) | Exact incarnation/Principal; immutable tombstone; audited state must match audit head | Retire and disable, retain ownership/history; never rebind a recreated username to the old Principal |
| Policy/audit (C1-B/C) | Current explicit deployment grant, independent of teacher role; authority lock serializes mutations | Recheck the current manager; preserve policy rows/history while retirement removes effective access |
| Retirement (C1-D) | Actor/payload-bound command ID, last-manager guard, append-only receipt, commit before publication | Reuse the pending in-transaction retirement writer; its receipt alone does not prove source deletion |
| Session commands (C1-F) | Source -> registry metadata -> authority -> child locks; fresh Session time after waits and before commit | Take source `FOR UPDATE` from the start; never upgrade from `FOR SHARE` |

## Smallest supported operation

[`delete_session_account`](../../engine/src/services/auth_service/durable_source/deletion.rs) accepts
a token separately from `SessionDeleteAccountCommand`: command ID, Workspace, exact source account
reference and expected Principal. The actor is resolved from the current durable Session and audited
manager grant, never supplied by the caller. Source schema 1, identity schema 2 and access schema 2
must already be explicitly installed in the same schema. No schema installer or bootstrap is called.

The target must be **another account**, currently present at the exact incarnation and bound to the
expected active, non-retired Human. This slice rejects self-deletion even with another manager,
unbound accounts, disabled/retired cleanup, absent targets and wrong authorities/Workspaces.
These restrictions avoid self-revoking authorization and inventing a separate deletion ledger.

One borrowed transaction holds the source writer lock and registry authority lock through:

1. Current Session/manager and exact target/binding verification.
2. Existing C1-D retirement, disabled Principal and appended identity audit.
3. Target source recheck, explicit deletion of **all** target Sessions (including expired ones),
   confirmed affected-row count and exact account deletion.
4. Final source/Session absence and audited tombstone readback, then surviving actor/manager and
   exact unexpired Session checks using fresh database time.
5. Confirmed commit, then receipt publication.

The actor cannot be the target. Its final audited grant plus exact surviving source Session establishes
at least one source-backed manager; a deleted or same-name replacement account cannot satisfy that
condition. C1-D's registry last-manager check also remains. No policy revisions, memberships, grants,
resource ownership or historical audit rows are rewritten. Retired identity denies effective access.
An archived Workspace does not by itself revoke independent deployment management.

## Strict one-shot result, not a new replay protocol

The returned `LegacyIdentityReceipt` describes retirement. Only a successful return from this adapter
confirms that the associated source deletion committed with it. Existing receipt encoding, command
digests, SQL templates 0001–0010 and standalone Retire semantics remain unchanged.

This adapter **never returns a successful replay**. After success, the absent exact source account
returns `AccountMismatch`; after same-name recreation, the old incarnation still mismatches. Reusing
the command ID with replacement coordinates conflicts. A prior standalone Retire, even with the same
ID and payload, cannot be used to delete the still-present source account: retired targets are rejected,
and pending retirement receipt replay is independently rejected before any source write.

Cancellation, timeout or lost commit acknowledgement **does not prove rollback**. Keep the exact ID,
actor and target coordinates; reconcile both source state and audited identity history using trusted
maintenance access. Neither a retirement receipt nor `AccountMismatch` establishes that this adapter
performed deletion. Do not switch IDs, auto-compensate, or claim a retryable deletion receipt API.
Durable deletion receipts/recovery, self-delete and unbound/retired cleanup remain future work.

## Concurrency and failure boundaries

Source `FOR UPDATE` fences register/login/logout and C1-F commands before registry locks. A login
committed first has its Sessions removed; a login behind deletion cannot authenticate the deleted
account. Logout first invalidates admission; deletion first may commit before logout. Policy revoke
and deletion share the authority lock and can remove at most one of two managers. Ordinary source
validation and same-name recreation wait for deletion to commit, then observe absence/new incarnation.
Registry-first standalone writers never acquire the source lock in reverse order.

Suppressed writes, missing audit, trigger-induced changes, expiry after waits, deferred commit failure
and pre-commit cancellation return no success. Existing ten-second operation/two-second lock/five-second
statement limits apply. A late COMMIT can still cross expiry or lose its response; tests do not claim
continuous authorization or solve uncertain commits. Privileged SQL/DDL and the old live writer are
not fenced by this staging adapter; never co-locate activated source tables with live AuthService.

## Verification and remaining gates

[`durable_account_deletion_tdd`](../../engine/tests/durable_account_deletion_tdd.rs),
[faults](../../engine/tests/durable_account_deletion/faults.rs) and
[concurrency](../../engine/tests/durable_account_deletion/concurrency.rs) add one default closed-pool
case and 16 explicit PostgreSQL cases. Missing API compilation was observed before implementation.
CI checks each ignored test exists, then runs it by exact name against its disposable PostgreSQL 16
service; the [selector regression](../../scripts/tests/postgresCi.test.mjs) rejects omissions.

```bash
CYANREX_TEST_DATABASE_URL='<disposable PostgreSQL URL>' CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_account_deletion_tdd \
  -- --include-ignored --test-threads=4
```

The development container compiled the suite and ran its default case; PostgreSQL startup was blocked
by its root-only user mapping. Database results must come from the PR's real PostgreSQL CI run, not
the locally ignored tests. No release tag, Release Candidate Validation, live migration, restore,
deployment or kernel acceptance is implied. Unified bootstrap, public routes/CSRF/admission,
credential changes, general account lifecycle/reconciliation and live cutover remain separate gates.
