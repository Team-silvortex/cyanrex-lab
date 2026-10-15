# Platform host catalog preflight

Included in source **0.6.0** on **2026-10-15**; see [release validation](../../reports/releases/0.6.0/README.md).
The dated implementation scope below remains historical, not a new deployment acceptance.

This Unreleased follow-up to 0.5.9, dated 2026-10-15, adds explicit host configuration and
catalog-only database inspection under [platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
It is an unmounted Rust library boundary: no listener, Engine, Session issuer or storage installer
is constructed. The existing teaching runtime and authentication remain unchanged.

## Trusted configuration

`PlatformHostPreflightConfig` contains a canonical trusted origin, workspace reference, three
distinct private namespaces, expected database coordinates and explicit `SessionStockLimits`.
It has no defaults, request deserialization, environment lookup, connection string or secret.
Construction validates values without I/O and reuses the existing origin and namespace rules.

`PlatformNamespaces` separates Source, Task and Artifact namespaces. Source also contains its
collaboration registry. `PlatformDatabaseTarget` requires the expected database name, runtime role,
PostgreSQL system identifier and database OID. Operators must obtain these coordinates through a
separately trusted provisioning process; inspection does not discover and adopt a target for them.

The scope and Session limits are configuration bindings, not proof that stored data belongs to that
workspace or that a future login entry point uses the limits. The separate
[Session stock policy](durable-session-stock.md) and [Session lifecycle adapter](platform-session-lifecycle.md)
retain their existing boundaries.

## Read only inspection

`inspect_platform_database(config, connection)` borrows a caller-owned `PgConnection`. It rejects
an existing transaction rather than nesting into the caller's snapshot. It owns one bounded
`REPEATABLE READ, READ ONLY` transaction and returns an opaque observation only after confirmed commit.
It neither constructs a pool nor changes role, search path or database privileges.

SQLx-managed transactions are rejected directly. Because driver depth alone cannot identify a raw
SQL BEGIN, a single savepoint-and-release batch probes that boundary first. An existing caller
transaction is left open and rejected; the probe does not commit, roll back or adopt its work.

The inspection has an overall deadline and transaction-local statement and lock timeouts. Cancellation
or deadline expiry after acquiring its owned SQLx transaction object makes the driver queue rollback
for the connection's next work. An interrupted BEGIN may have reached the server without returning
that object; an error does not confirm server transaction termination. Cancellation during the boundary
probe does not authorize rolling back caller work.

The observation reads PostgreSQL catalogs and verifies:

- The actual `pg_catalog.pg_control_system()` system identifier, database name and OID match the
  configured physical target. No server-address or database-name-only fallback is available.
- Current user, session user and configured runtime role are identical. The role can log in and
  connect, but is not superuser, database owner, replication-enabled or able to bypass RLS, create
  roles or create databases. It has no direct role memberships and no database CREATE or TEMP privilege.
- All three namespaces exist, grant USAGE but not CREATE, and are not owned by the runtime role.
  Their actual OIDs and owner OIDs are retained. The effective application search path contains
  only Source, and no temporary namespace exists.

The operator must separately permit the runtime role to execute the read-only
`pg_catalog.pg_control_system()` function in the target database. A missing permission fails closed;
the library never grants it, elevates privileges or substitutes a weaker identity check.

This checks the listed catalog privileges, not every executable privileged function or all possible
database access. PostgreSQL identity coordinates are not TLS authentication and do not distinguish a
privileged operator's exact physical clone. Transport identity remains a separate requirement.

## Original observation comparison

`PlatformCatalogObservation` has no Debug, Clone or serialization implementation. Its `matches`
method compares complete configuration and observed catalog identities; it is not a health report,
connection factory, authorization permit or reusable storage guard.

`verify_platform_database(config, connection, expected)` takes a fresh read-only observation and
compares it with the original. A replaced namespace cannot be accepted merely because its name and
permissions are unchanged. Configuration drift, unsafe privileges, target mismatch and unavailable
catalog reads fail without returning raw SQL errors or repairing anything.

This is a point-in-time catalog comparison. It does not lock out later changes, pin subsequent pool
connections or replace the Source and resource guards inside each authorized business transaction.

## Verification and next gates

Default tests cover pure configuration validation. Explicit disposable PostgreSQL tests exercise the
catalog boundary through an independently created database and restricted runtime role. They require
only a deliberately selected `CYANREX_TEST_DATABASE_URL` and run through:

```bash
./scripts/test-platform-host-preflight.sh
```

The runner checks exact selectors, and CI uses its existing disposable PostgreSQL service. These tests
must not target application data. Their own databases and roles are removed before final assertions.

Empty namespaces can pass: this slice reads no business tables, credentials, Session records,
authority audit or storage schema markers. It does not establish schema compatibility, workspace
ownership, valid authority history or healthy stored content. The separate
[Artifact root preflight](platform-artifact-root.md) checks directory identity only, while
[storage marker observation](platform-storage-markers.md) checks five version declarations and their
existing scope fields without establishing full schema compatibility. Process privileges,
secure connection configuration, transport acceptance and actual low-privilege host composition remain
separate gates. There is no migration, bootstrap, automatic cleanup, public login or browser Task saving.
