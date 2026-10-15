# Session lifecycle database privileges

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This 2026-10-15 Unreleased follow-up after **0.6.0** removes the whole-table credential-read
requirement from the prepared Source-only Session boundary under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
It enables a deliberately restricted database role to query and log out existing Sessions through
the [unmounted lifecycle adapter](platform-session-lifecycle.md), without password or TOTP reads.
It does not start a host, issue Sessions or change the teaching application's authentication.

## Tested role boundary

The disposable tests connect as a separate LOGIN/NOINHERIT role, not an administrator using an
application username. The role owns no source objects, has no role membership or namespace CREATE,
and receives only these table/column privileges in the fixture's private Source namespace:

| Relation | SELECT columns | UPDATE column for row locks | Table privilege |
|---|---|---|---|
| `users` | `username`, `account_id` | `username` | None |
| `sessions` | `token`, `username`, `account_id`, `expires_at` | `token` | DELETE |
| `collaboration_auth_source_schema` | `singleton`, `version`, `authority_id` | `singleton` | None |

There is no whole-table SELECT/UPDATE, INSERT or TRIGGER grant. Four independent actual projections
of `password_salt`, `password_hash`, `totp_secret` and `otp_last_counter` must each fail with 42501.
The fixture also checks effective privileges independently, including exactly nine readable columns
and three updateable columns. These are tested requirements for this boundary, not a universal
deployment role template or an exhaustive privilege audit.

Column UPDATE is a real write permission required by the existing row-locking commands; this is
not a read-only role or per-user SQL isolation. Session digests remain readable, and direct SQL
DELETE permission is not restricted to one device. Only a trusted service may hold this role.
The application commands still derive the exact target and check original Source/account/Session
identity, fresh expiry and confirmed commit. Login, registration, maintenance and registry-backed
private work have separate privilege needs; no production role is granted or repaired by this code.

## Relation locks without executing a probe

`RelationPin::capture(..., false)` sends one Simple Query batch with a fixed reserved statement
name, `cyanrex_session_source_relations_v1`. It prepares a constant projection from the three
`ONLY` relations, then immediately deallocates that statement. It never executes or explains it.
PostgreSQL [PREPARE](https://www.postgresql.org/docs/16/sql-prepare.html) separates parsing and
rewriting from later planning/execution. The tests independently observe that all three granted
AccessShareLocks remain in the transaction after deallocation and block real conflicting DDL.

The original ordinary-table, persistence, RLS, inheritance, partition, primary-key and resolved-OID
checks remain. Source-only capture additionally verifies the actual held locks against current
PID, current database and each resolved relation OID. The registry-backed branch retains its original
explicit table-lock sequence. PREPARE itself is not an ACL or authorization check: actual business
reads/row locks/deletion must still succeed under their real permissions.

This avoids running even a LIMIT 0 probe, whose planning could evaluate a malicious immutable
view or RLS expression. Explicit trap controls exercise that risk; the Source rejects the unsupported
structure without evaluating it. The existing final Session and post-write original-source checks
are unchanged.

## Failure and cancellation

A pre-existing statement with the reserved name causes failure. The source never executes,
deallocates, adopts or retries that earlier plan, and never uses DEALLOCATE ALL or a random replacement
name. Normal successful batches leave no reserved statement behind. Cancellation or an interrupted
server batch can still leave this one fixed slot on a physical connection; rollback is not a promise
that session-level prepared state was cleared. A later collision must remain a closed failure.

The relation locks do not refresh an earlier REPEATABLE READ catalog snapshot. This change does
not fix every pre-existing same-OID constraint-drift case or establish complete schema compatibility.
HTTP failures remain private and unconfirmed, do not clear the Cookie and never authorize blind retry.

## Explicit verification

With an explicitly selected disposable `CYANREX_TEST_DATABASE_URL`, run:

```bash
./scripts/test-platform-session-permissions.sh
```

The four exact SQL selections cover the credential-restricted lifecycle, missing required privileges,
actual lock/DDL waits, unsupported executable traps and reserved-plan collisions. Synthetic roles,
pools and namespaces are cleaned before final assertions, including a partial view setup that leaves
the original table renamed with no replacement. CI reuses its existing disposable
PostgreSQL fixture; never point fault tests at application data. No listener, UI/SDK wiring,
permission migration or deployed-LAN acceptance is added.
