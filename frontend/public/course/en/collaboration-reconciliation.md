# ADR-012: Read-only lifecycle reconciliation

Status: **C1-K internal staging, included in 0.4.7**. The local
`cyanrex-provision reconcile` command checks whether the staged durable accounts, Sessions, identity
bindings, policies and their audit histories agree. It reports one bounded, read-only snapshot, not
an authorization decision, bootstrap receipt, secret-delivery confirmation or permission to retry.
There is no live Engine wiring, migration, repair, credential recovery or new database schema.

## Operator entry and scope

Use the private format-1 configuration described in [ADR-011](collaboration-provisioning.md), pointing
only to the independently verified local staging database, named schema, authority and legacy Workspace.
The existing private-file, explicit socket/loopback, environment isolation and replacement-connection
pinning rules still apply. Configuration `username` remains required for format compatibility, but is
not a reconciliation filter or required surviving account and is omitted from this command's target report.

```bash
cyanrex-provision reconcile --config /absolute/private/target.json
```

No confirmation, password-file, enrollment-file or repair flag is accepted. The command never opens,
rewrites or removes enrollment output. A preflight catalog snapshot checks the physical target; the
lifecycle graph is then read in its own single transaction, not assembled from preflight row counts.
Success returns `status: "consistent_snapshot"`, the explicit non-secret target and aggregate counts.
The Rust entry is `DurableAuthSource::reconcile_authority`; neither entry calls a schema installer.

A database administrator can prepare a reader with CONNECT, schema USAGE, catalog visibility and
EXECUTE on `pg_catalog.pg_control_system()`, plus SELECT on the twelve collaboration tables. On auth
tables the reader needs only `users(username, account_id)` and
`sessions(username, account_id, expires_at, token)`. CREATE, DML, sequence access and password/salt/TOTP
column privileges are unnecessary. The token column privilege is needed for an in-database digest-shape
check; the CLI does not fetch token digests. Privileges and trusted endpoint authentication must be
prepared separately; this tool never grants them to itself.

## Snapshot consistency and resource bounds

The graph transaction uses PostgreSQL **REPEATABLE READ, READ ONLY**. It does not acquire row locks,
installer fences or advisory writer locks; normal relation read locks still apply. Existing runtime
authorization and mutation readers retain their source-first locking and fresh Session checks.
Reconciliation is a separate observer, not a lock-free replacement for those paths.

Before reading data, it requires supported source/identity/access metadata versions 1/2/2, exact source
authority and legacy Workspace mapping, ordinary persistent tables, expected primary keys and selected
column types/nullability. Views, inheritance/partitions and RLS layouts are rejected even for superusers,
so filtered rows cannot silently become a successful empty observation. Existing source constraints and
source/audit trigger-shape checks also remain required. This is not an attestation of every DDL object,
trigger function body or historical action against a privileged database administrator.

The graph read has a ten-second total deadline, five-second statement deadline and two-second lock
deadline. Each scanned collection is limited to **10,000 rows, 4 MiB of selected variable metadata in
total, and 16 KiB per row**. Size/count checks run before fetching variable payloads; fixed-width field
types are checked first. These are inspection limits, not account-creation or history-retention limits.
Oversized state is left intact and requires a separately reviewed larger-scale inspection design.

SQL failure, cancellation, timeout, unsupported layout or any failed invariant produces no successful
partial report. A fixed error code identifies the first failed boundary; raw SQL, account names and
individual audit payloads are not emitted. The command does not prune expired Sessions or old history.

## Checked relationships

| Boundary | Required observation |
|---|---|
| Source accounts and Sessions | Valid unique account IDs and canonical usernames; every Session matches the current exact account incarnation; stored digest has the expected shape |
| Identity bindings | Each binding has its retained Human Principal, matching coordinates and legal status; every unretired binding still matches its source account |
| Membership and deployment | Every legacy policy has a bound subject, the exact Workspace and paired member/grant records with equal valid revisions and canonical roles/status |
| Identity and policy history | Decode every entry, verify coordinates, request digests and transition semantics, follow each subject's complete before/after chain, and match final heads to current records |
| Current management | At least one active, unretired, audited Human with an exact current source account and explicit deployment grant |

Sequence gaps are valid: rolled-back allocations and other subjects/authorities consume sequence
values. Sequence order supplies traversal, not a gapless commit cursor or wall-clock order. Baselines
describe only activation-time observations and do not invent older commands or actors. Retained audit
actors must resolve to a binding; their historical permissions are not reconstructed from current grants.

Ordinary newly registered, unbound accounts are valid. A retired binding may still have its original
source account because registry-only retirement does not delete credentials. Its retained grant is not
current management authority. A deleted account's same-name replacement with a fresh ID is a distinct
unbound account; reusing an existing account ID under a different name is rejected even after retirement.
Deployment management does not depend on an active teaching membership, teacher role or unarchived
Workspace: the explicit grant and current source-backed Human are authoritative for that count.

The report includes snapshot observation time, Workspace status, account/unbound/retired-source counts,
Session/expired-Session counts, bound/retired/disabled identity counts, policy count, current manager
count and both audit-entry counts. Retired and disabled counts overlap; `bound_identities` includes
retained retired bindings. Expiration is evaluated at the observation time, not at later use of the report.
Other authorities, future multi-Workspace/Agent policies and unrelated application tables are outside
this legacy authority check. Non-bound Principals are not presented as authenticated identities.

## Failure interpretation and recovery boundary

| Code family | Meaning and response |
|---|---|
| `reconciliation_unavailable` | No complete snapshot was confirmed; restore access or investigate contention, without treating failure as rollback |
| `reconciliation_unsupported_schema`, `reconciliation_invalid_namespace`, `reconciliation_scope_mismatch` | Target/layout is not supported; verify configuration and installation evidence, never auto-install or adopt |
| `reconciliation_limit_exceeded` | Inspection budget exceeded; preserve data, do not delete history to make the check pass |
| `reconciliation_invalid_source`, `reconciliation_invalid_identity`, `reconciliation_invalid_policy` | Current records disagree at the named boundary; preserve evidence for trusted investigation |
| `reconciliation_identity_history_mismatch`, `reconciliation_policy_history_mismatch` | Full history cannot validate current records; do not rewrite a baseline or current head |
| `reconciliation_no_current_manager` | No qualifying source-backed manager exists in the observed graph; no automatic regrant or first-user election |

Failures exit 2. Configuration, connection and preflight failures can instead use the existing CLI
codes. A successful exit 0 is still only an observation: an in-flight mutation can commit after the
snapshot, or the snapshot can legitimately precede that mutation. The report neither establishes a
maintenance window nor proves that a particular uncertain command committed or rolled back.

After uncertain bootstrap, preserve the private marker/output and establish that earlier operations
have finished before any recovery decision. `inspect` shows catalog occupancy; `reconcile` checks the
current bounded graph. Neither reads TOTP, verifies a password, resends enrollment, proves filesystem
delivery or authorizes another `apply`. Audits cannot prove history before their baselines or detect a
privileged rewrite that consistently replaces both current state and all evidence. Enrollment recovery,
reviewed backup/restore and live route/CSRF cutover remain separate work.

## Verification

The [reconciliation suite](../../engine/tests/durable_reconciliation_tdd.rs) defines one default
source-isolation test and twelve explicit PostgreSQL cases. They exercise normal lifecycle states,
read-only privileges without auth-secret SELECT, schema/RLS/type rejection, corrupted/removed older
audit entries, current-head mismatch, bounded input, no manager, interleaved commits and cancellation.
Fault fixtures use only disposable data and coordinate with the existing bootstrap event-hook guard.
An additional [CLI regression](../../engine/tests/provision_cli_tdd.rs) rejects write flags and ambient
fallback, and help output no longer contains stray patch markers.

All twelve PostgreSQL cases also passed over loopback TCP with SCRAM authentication, including the
non-superuser reader with database-enforced read-only transactions and restricted auth-column access.
CI selects each database case by exact name and rejects an empty selection. Local verification totals
and their limits are recorded in [Project Status](project-status.md). This does not imply remote CI,
frontend production, deployed database, migration, enrollment recovery or privileged kernel acceptance.
