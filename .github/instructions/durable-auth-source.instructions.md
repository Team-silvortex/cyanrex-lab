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
