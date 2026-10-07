# ADR-012: Read-only lifecycle reconciliation

Status: **C1-K internal staging, included in 0.4.7**. The local
`cyanrex-provision reconcile` command checks whether the staged durable accounts, Sessions, identity
bindings, policies and their audit histories agree. It reports one bounded, read-only snapshot, not
an authorization decision, bootstrap receipt, secret-delivery confirmation or permission to retry.
The observer neither installs nor migrates storage, repairs records, recovers credentials or wires
the live Engine. The OTP-consumption follow-up included in 0.5.1 now requires prepared source schema 2;
the original C1-K introduction remains the 0.4.7 milestone. The 2026-10-07 Session-expiry and registry-time
validation follow-ups below are included in 0.5.2 without a schema change. Their dated entries retain
the then-unreleased 0.5.1 implementation stage and its original verification scope.

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
tables the reader now needs only `users(username, account_id, otp_last_counter)` and
`sessions(username, account_id, expires_at, token)`. CREATE, DML, sequence access and password/salt/TOTP
column privileges are unnecessary. The token column privilege is needed for an in-database digest-shape
check; the CLI does not fetch token digests. Likewise, the watermark column is used only to select
an in-database range-validity boolean; its value is neither fetched nor added to the report. Update
older reader grants explicitly rather than granting credential columns. Privileges and trusted
endpoint authentication must be prepared separately; this tool never grants them to itself.

## Snapshot consistency and resource bounds

The graph transaction uses PostgreSQL **REPEATABLE READ, READ ONLY**. It does not acquire row locks,
installer fences or advisory writer locks; normal relation read locks still apply. Existing runtime
authorization and mutation readers retain their source-first locking and fresh Session checks.
Reconciliation is a separate observer, not a lock-free replacement for those paths.

Before reading data, it requires supported source/identity/access metadata versions 2/2/2, exact source
authority and legacy Workspace mapping, ordinary persistent tables, expected primary keys and selected
column types/nullability. Views, inheritance/partitions and RLS layouts are rejected even for superusers,
so filtered rows cannot silently become a successful empty observation. Existing source constraints and
source/audit trigger-shape checks also remain required. This is not an attestation of every DDL object,
trigger function body or historical action against a privileged database administrator.

Source schema 2 includes the non-null signed 64-bit OTP watermark with exact default `-1` and the
validated lower-bound check. The reader checks that stored counters are at least `-1`, without
reconstructing which code or operation advanced them. Source schema 1 is rejected, including an empty
source; reconciliation cannot upgrade it or initialize missing counters. These version checks also
do not fence legacy AuthService writers that ignore prepared-source metadata. The prepared source
must remain separate from those writers.

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
| Source accounts and Sessions | Valid unique account IDs and canonical usernames; OTP watermark is in range; every Session matches the current exact account incarnation; stored digest has the expected shape |
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

### Unreleased Session expiry range checks · 2026-10-07

Source Session expiry now stays inside SQL: first require finite `expires_at` no greater than chrono's
supported maximum, then compare it with the existing snapshot observation time. The reader fetches
only an optional `expired` boolean, not the raw timestamp. A NULL validity result becomes
`InvalidSource` (`reconciliation_invalid_source`), so `infinity`, `-infinity` and finite year 262143
cannot reach the binary timestamp decoder, silently disappear or count as ordinary expiry.
Representable finite extremes, including a PostgreSQL 4713 BC date and chrono's maximum at
microsecond precision, retain normal expired/not-expired counting.

The same read-only column grants suffice; this adds no credential, raw-expiry or token disclosure,
record repair, schema migration or new CLI/API. Snapshot timing and inspection budgets are unchanged.
That stage covered source Sessions only; registry `retired_at` and audit `recorded_at` were outside its
scope. The separate registry follow-up below adds their guards, not protection for every timestamp. Current exact test
selection and dated results are in [the testing guide](testing-guide.md) and [Project Status](project-status.md);
the original twelve-case C1-K evidence below remains historical.

### Unreleased registry timestamp decoding · 2026-10-07

The registry follow-up checks three stored columns before binary timestamp decoding:
`collaboration_legacy_identities.retired_at`, `collaboration_identity_audit.recorded_at` and
`collaboration_policy_audit.recorded_at`. SQL requires finite non-null times no greater than chrono's
maximum; infinite or out-of-range values are not filtered away, clamped or repaired. A genuinely NULL
retirement still means not retired. An independent `valid_retired_at` boolean distinguishes that state
from a malformed non-null retirement projected as NULL, which must fail as `InvalidRecord`.
Both audit timestamps are mandatory; a NULL projection likewise fails as `InvalidRecord`.

These projections are shared by locked identity reads, current audit heads, older history pages,
command replay, append readback and read-only reconciliation. Every audit query binds the range
parameter, including `LIMIT 0` schema checks. Reconciliation preserves its existing classifications:
bad retirement becomes `InvalidIdentity`, bad identity-audit time becomes `IdentityHistoryMismatch`,
and bad policy-audit time becomes `PolicyHistoryMismatch`; no partial consistent report is returned.

This is a representability guard for those three columns, not a new age, chronology, retirement or
audit-order policy. Existing state/history checks, lock order and transaction semantics remain in force.
Other timestamp paths are not thereby proven safe. It changes no dependencies, storage schema, product
version, live cutover or public interface, and rewrites no stored data. The new timestamp regression
inventory is separate from the historical C1-K execution evidence below.

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
This also applies to atomic OTP consumption: a successful snapshot is not a receipt for a particular
login or password change. Never lower/reset a watermark or blindly repeat an operation because a
previous commit acknowledgement was lost.

After uncertain bootstrap, preserve the private marker/output and establish that earlier operations
have finished before any recovery decision. `inspect` shows catalog occupancy; `reconcile` checks the
current bounded graph. Neither reads TOTP, verifies a password, resends enrollment, proves filesystem
delivery or authorizes another `apply`. Audits cannot prove history before their baselines or detect a
privileged rewrite that consistently replaces both current state and all evidence. Enrollment recovery,
reviewed backup/restore and live route/CSRF cutover remain separate work.

## Verification

The following preserves the C1-K introduction evidence against source/identity/access versions 1/2/2.
Current schema-2 follow-up results and their limits are recorded separately in [Project Status](project-status.md).

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
