# Explicit platform catalog identity

A **2026-10-16 Unreleased** [marker declaration](platform-marker-expectation.md) can consume this
complete catalog declaration and bind all five relation OIDs/owners before any marker-row read.
Private format 3 requires both declarations; the original format 2 diagnostic remains compatible.

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This Unreleased follow-up after **0.6.0** adds an explicit catalog declaration under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
A trusted operator can declare the original role, database owner and three namespaces before the
first connection. An otherwise valid same-name replacement cannot become that connection's baseline.
This is an operator assertion, not a database observation, permission or health receipt.

## Typed declaration and comparison

`PlatformCatalogExpectation::new(config, role_oid, database_owner_oid, namespace_oids,
namespace_owner_oids)` consumes the complete trusted host configuration. Both namespace arrays
are ordered **Source, Task, Artifact**. All eight OID positions must be nonzero; namespace OIDs must
be pairwise distinct. Owners may be shared, but the runtime role cannot own the database or any
namespace. Numerical OIDs may overlap across independent PostgreSQL catalogs; `u32::MAX` is valid.

The declaration has no Debug, Clone, serialization or observation-conversion API. Construction
does not read a database or discover missing coordinates. The trusted caller must establish the
values independently, not derive them from a potentially substituted first runtime connection.

`inspect_platform_database_expected(config, connection, expected)` rejects any complete-config
mismatch before SQL, then runs the existing read-only [catalog inspection](platform-host-preflight.md).
It compares every role/owner/namespace identity and returns only a genuine matching observation.
Names alone, equivalent privileges and a newly observed valid object cannot replace declared OIDs.

`PlatformDatabaseConnector::with_catalog_expectation(config, expected, endpoint, password)`
is pure construction. Every explicit `connect()` makes one attempt, checks the immutable declaration,
and publishes a connection only after a match, within the existing ten-second budget. Wrong identities
return fixed `Unavailable`; configuration mismatch is `InvalidConfig` before dialing.
The old observation-based constructor remains available with its original contract.
[Local transport, password and logging boundaries](platform-database-connection.md) are unchanged.

## Private JSON version 2

Keep all fields and private-file requirements from [startup input](platform-startup-input.md), set
`format_version` to 2, and require `catalog_identity`. This complete example uses synthetic coordinates,
not approved deployment values:

```json
{
  "format_version": 2,
  "origin": "https://platform.example:8443",
  "authority_id": "00000000-0000-0000-0000-000000001069",
  "workspace_id": "00000000-0000-0000-0000-00000000106a",
  "namespaces": {
    "source": "source_private", "task": "task_private", "artifacts": "artifacts_private"
  },
  "database": {
    "database": "platform_db", "role": "platform_runtime",
    "system_identifier": 123, "database_oid": 456,
    "transport": {"kind": "loopback", "address": "127.0.0.1"},
    "port": 5432, "password": "replace-with-explicit-database-password"
  },
  "session_limits": {"per_account": 4, "total": 16},
  "artifact_root": "/private/platform/artifacts",
  "catalog_identity": {
    "role_oid": 101, "database_owner_oid": 102,
    "source": {"oid": 201, "owner_oid": 102},
    "task": {"oid": 202, "owner_oid": 102},
    "artifacts": {"oid": 203, "owner_oid": 102}
  }
}
```

Version 1 remains valid without this field and forbids it even when null. Version 2 rejects a
missing/null declaration, unknown or duplicate fields at every layer, positional arrays, wrong
types, zero/negative/out-of-range OIDs and inconsistent ownership. No optional identity is filled
from the database. The entire input, including the declaration, still fits the 16 KiB limit.

`check-config` only validates the declaration's shape and consistency, then discards it without
networking. It does not establish that the OIDs exist or are trustworthy.
[check-storage](platform-storage-check.md) version 2 calls
`check_platform_database_storage_expected`: it compares the catalog before inspecting the five
current markers, closes the connection on either inspection outcome, then rechecks the original
Artifact root and process after runtime teardown. Version 1 remains a current-object diagnostic.
Neither format exports a baseline or starts a host.

## Scope and verification

Catalog identity does not pin historical marker relation IDs or a root seen before this invocation.
It does not establish complete schema compatibility, business data, current Session permission or
continuous protection after handoff. OID reuse or a physical clone preserving all configured identities
is not distinguished. Comparison happens after database authentication, not before password delivery;
independently trust the local peer. No pool, automatic retry, installer, migration, issuer or listener
is added. Future host composition and retained original marker/root bindings remain separate.

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_catalog_expectation_value_tdd
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
bash scripts/test-platform-catalog-expectation.sh
```

The SQL runner requires an explicit disposable `CYANREX_TEST_DATABASE_URL` with role/database
administration and password authentication for runtime roles; admin-only local trust is allowed.
Four exact selections cover independent administrator-supplied OIDs, twelve configuration differences,
each of the eight wrong identities, valid namespace replacement before first connection, subsequent
role/owner/privilege drift, and real CLI version-1/version-2 behavior. Fixtures use SCRAM when configured
by the test server; the runner itself does not enforce a particular password mechanism.
Tests retain unchanged snapshots and reclaim owned resources before final assertions.
These are local regression checks, not multi-machine LAN/TLS or browser-save acceptance.
