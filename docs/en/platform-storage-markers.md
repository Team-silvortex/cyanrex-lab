# Platform storage version and scope markers

Included in source **0.6.0** on **2026-10-15**; see [release validation](../../reports/releases/0.6.0/README.md).
The dated implementation scope below remains historical, not a new deployment acceptance.

This 2026-10-15 Unreleased follow-up to 0.5.9 adds an independent marker-only observer under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It binds five storage
declarations to an original [database catalog observation](platform-host-preflight.md). It is not a
complete schema compatibility check, authority audit or deployable host; the library remains unmounted.

## Supported declarations

`inspect_platform_storage_markers(config, connection, expected_catalog)` borrows a caller-owned
connection and requires the complete original catalog configuration and identities to match.
Only these five tables and their necessary metadata fields are selected:

| Configured namespace | Marker table | Version | Scope fields compared |
|---|---|---|---|
| Source | `collaboration_auth_source_schema` | 2 | Authority |
| Source | `collaboration_identity_schema` | 2 | None in this marker |
| Source | `collaboration_access_schema` | 2 | None in this marker |
| Task | `collaboration_task_schema` | 3 | Authority and workspace |
| Artifact | `collaboration_artifact_schema` | 1 | Authority and workspace |

Each marker must be an ordinary permanent table without RLS, forced RLS, inheritance or partitioning.
The exact visible columns have their expected built-in types, are non-null and are not generated or identity
columns; the singleton primary key is checked. Exactly one row must exist, with singleton true and the
supported version. Source, Task and Artifact scope fields must equal the trusted configuration.

The runtime role must not own these tables or hold trigger privileges on them. Other grants and all
business-table definitions remain outside this marker contract. Two registry version numbers do not
establish a registered workspace, active membership, valid policy or consistent audit history.

## Transaction and lock boundary

The observer reuses the catalog boundary's rejection of SQLx-managed and raw outer transactions. It
owns one bounded `REPEATABLE READ, READ ONLY` transaction and does not change role or search path.

After transaction-local timeout settings, it first attempts ACCESS SHARE locks on the five qualified
candidate names in a fixed order, before taking the catalog observation snapshot. This ordering avoids
using an earlier snapshot to approve constraints changed during a lock wait. The subsequent original
catalog comparison must succeed before any marker row is selected. Candidate-name locks can therefore
fail before physical target revalidation; error categories do not promise target-first diagnostics.

Actual held relation locks, resolved namespace/relation identities and selected table OIDs are checked.
Marker rows are read through qualified `ONLY` projections with `LIMIT 2`; guarded Boolean results avoid
materializing a changed field's arbitrary text in the caller. Transaction-local `row_security = off`
makes applicable RLS fail rather than silently filter rows; it does not grant a bypass privilege.

The overall deadline includes the transaction-boundary probe, lock waits, reads and commit/rollback. Cancellation after an
owned SQLx transaction object exists queues rollback for subsequent driver work; an interrupted BEGIN
or disconnected server still has an unknown final state. Neither failure nor timeout permits adoption,
repair, migration or automatic retry.

## Original observation comparison

`PlatformStorageMarkerObservation` has no Debug, Clone, serialization or public construction. It binds
the verified catalog and marker identities. Its `matches` method passively compares observations; it
does not reread storage or confer runtime authorization.

An initial `inspect` has no previous marker identities. It may observe a currently valid replacement
when the original catalog still matches; the catalog observation alone does not pin marker history.

`verify_platform_storage_markers(config, connection, expected)` takes another bounded observation and
compares it with the original. A healthy same-name replacement must not become the original marker.
Unsupported shape/version, mismatched scope, changed identities or unavailable reads fail with static
errors, without disclosing SQL messages, credentials or business bodies.

## Verification and remaining gates

The explicit disposable SQL suite installs real templates in its own database and uses a restricted
read-only runtime role. It independently checks marker access, zero business-column access, actual
permission-denied projections and unchanged stored-data digests. Other cases cover missing or invalid
markers, unsafe shapes, original dependencies and actual lock-wait drift. Run with an explicitly
selected disposable `CYANREX_TEST_DATABASE_URL`:

```bash
./scripts/test-platform-storage-markers.sh
```

The runner checks all exact selectors; CI uses the existing disposable PostgreSQL fixture. These tests
must not target application data. Synthetic databases and roles are cleaned before final assertions.

No users, Sessions, credentials, Task/Artifact bodies, outbox or audit history are read by this API.
Valid markers do not prove full business column/constraint compatibility, valid records, current
authority, content health or writable deployment. Registry mapping/history, complete storage contracts,
staging roots, process privileges and actual host composition remain separate gates. The
[Artifact directory observer](platform-artifact-root.md) is independent; there is no listener, Session
issuer, bootstrap, storage migration or browser Task saving.
