---
description: Schema adoption and authentication boundaries for the durable auth source
applyTo: 'engine/src/services/auth_service/durable_source/**/*.rs'
---

## Learnings

An empty table and a successful `SELECT ... LIMIT 0` do not establish schema compatibility: before
explicit empty-source installation, verify column types/nullability and primary/foreign key targets,
including cascading deletion, rather than silently adopting or repairing incompatible tables.
Preserve the missing-session-primary-key and malformed-empty-schema regressions in
[the source integration tests](../../engine/tests/durable_auth_source_tdd.rs).
The account incarnation must come from committed registration, never username-based backfill;
a returned session snapshot is not authority to execute a later identity or policy command.

Session-authorized commands hold one transaction from source verification through registry mutation,
audit and confirmed commit: acquire source metadata, registry metadata, authority, then child rows;
recheck the exact session with fresh database time after waits and before committing.
Preserve the logout-order and expiration regressions in
[the composition fault tests](../../engine/tests/durable_collaboration/faults.rs), not a separate
`validate_session` call followed by an unrelated command transaction.
When revoking management, a remaining registry grant counts only if its audited Human identity still
matches a current durable source account incarnation; deleted or same-name recreated accounts cannot
serve as the last-manager safety net, as covered by
[the composition integration tests](../../engine/tests/durable_collaboration_tdd.rs).

Restricted account deletion takes source `FOR UPDATE` before registry locks (never upgrade from
`FOR SHARE`), admits only another active bound incarnation, and rechecks the surviving manager and
Session after deletion. An identity retirement receipt is not durable deletion evidence: reject
already-retired targets and receipt replay before source writes. Preserve both login/logout orderings,
replacement-account protection and rollback tests in `durable_account_deletion_tdd`.

Self-service password rotation derives its target from the current Session and requires the current
password plus TOTP, not a manager grant. Release snapshot locks before Argon2; reacquire source
`FOR UPDATE` and recheck the exact Session, incarnation and complete credentials before writing.
Credential replacement and all-session revocation must commit together. After deliberately deleting
the calling Session, check its verified expiry against fresh database time after the final writes;
do not skip freshness or expect the deleted row to remain. Preserve identity/TOTP/grants, count and
read back revocation, and retain the fault/concurrency/registry tests in `durable_password_change_tdd`.
A failed acknowledgement is not proof of rollback, and no credential-change replay is available.

Private Task/Artifact commands share the `private_work` guard: current audited Human membership
permits only the caller's own resources, without a teaching/deployment grant. Keep target/source
namespace pins and return neither records nor content bytes before the final Session check and commit.
Borrowed Artifact reads must lock the head before related queries under the source's READ COMMITTED
transaction; the standalone reader's unlocked lookup depends on its own REPEATABLE READ snapshot.
File publication is not rolled back with PostgreSQL. Cancellation can leave a blocking writer running;
never delete, adopt or automatically retry an unpublished file based only on a failed acknowledgment.

Fresh-authority bootstrap is trusted provisioning, never public registration or first-login election.
Before DDL, take the existing source/identity/access installer advisory fences in that order and reject
any nonempty or ambiguous namespace. Use source -> registry metadata -> authority -> child lock order,
commit account/identity/initial grant/audit activation together, and publish no Session or secret before
confirmed commit. A repeated bootstrap or deleted first account must not trigger regrant/recovery.
Keep the bootstrap fault injection counters: an early unrelated DDL error is not the intended row fault.

Maintenance reconciliation is deliberately separate from locking authorization: use one bounded
REPEATABLE READ, READ ONLY transaction, verify table/column shapes before payload reads, reject RLS
and inheritance, and inspect full audit chains with strict row/metadata budgets. Fetch no credentials
or token digests. Retired source accounts and unbound registrations can be valid, but account IDs
cannot be reused under another name. Do not infer rollback, delivery, quiescence or retry authority
from a read snapshot. Preserve read-only-role, corruption, MVCC and cancellation regressions.
