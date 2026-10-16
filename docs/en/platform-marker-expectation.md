# Explicit platform marker identities

Included in **0.6.2** on **2026-10-16**, this follow-up verifies original identities of all five
storage-marker tables together with the complete [catalog declaration](platform-catalog-expectation.md),
under [host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). A matching version/scope
alone cannot admit a same-name replacement on this explicit path. No host or issuer is started.

## Declaration and inspection

`PlatformStorageMarkerExpectation::new(catalog, marker_oids, marker_owner_oids)` consumes a complete
`PlatformCatalogExpectation`, not a second configuration. Both arrays use the fixed order below:

| Position and JSON name | Table in the configured namespace |
|---|---|
| 0, `source` | Source: `collaboration_auth_source_schema` |
| 1, `identity` | Source: `collaboration_identity_schema` |
| 2, `access` | Source: `collaboration_access_schema` |
| 3, `task` | Task: `collaboration_task_schema` |
| 4, `artifacts` | Artifact: `collaboration_artifact_schema` |

All ten values must be nonzero `u32`; the five relation OIDs must be distinct within `pg_class`.
Owners may be shared but cannot equal the declared runtime-role OID. Numeric overlap across different
PostgreSQL catalogs and `u32::MAX` are valid. Construction performs no IO. The opaque declaration has
no Debug, Clone, serialization, identity getters or conversion into an observation. Trusted operators
must independently select the identities; automatically trusting first-connection values establishes
no history.

`inspect_platform_storage_markers_expected(config, connection, expected)` rejects complete-config
differences before SQL and preserves SQLx/raw outer-transaction refusal. One owned REPEATABLE READ,
READ ONLY transaction uses the existing ten-second inspection budget:

1. Lock all five fixed qualified table names before the first catalog SELECT.
2. Match the actual physical catalog, then collect all five table shapes, OIDs/owners and held-lock
   evidence without reading any marker row.
3. Compare all ten identities before the first marker-row query.
4. Read fixed versions and configured scopes, recheck shapes/catalog and confirm COMMIT before
   returning a genuine `PlatformStorageMarkerObservation`.

Retained-observation verifiers share this early identity comparison. Ordinary
`inspect_platform_storage_markers` still observes current markers under a supplied genuine catalog;
it does not infer a historical declaration.

## Private input format 3

Keep the complete [format 2 input](platform-catalog-expectation.md), change `format_version` to **3**,
and add the required `marker_identity` object below. Numbers are synthetic, not deployment defaults:

```json
{
  "source": {"oid": 301, "owner_oid": 102},
  "identity": {"oid": 302, "owner_oid": 102},
  "access": {"oid": 303, "owner_oid": 102},
  "task": {"oid": 304, "owner_oid": 102},
  "artifacts": {"oid": 305, "owner_oid": 102}
}
```

Both `catalog_identity` and `marker_identity` are required. Every record must be a complete named
object: missing/null values, duplicate/unknown keys, positional arrays, wrong types, zero, overflow,
repeated relation OIDs and runtime-owned markers fail. All fields count toward the unchanged 16 KiB
limit. Schema versions/scopes remain fixed by the original contract, not extra user-selected fields.

| Format | Identity fields | Diagnostic meaning |
|---|---|---|
| 1 | Both forbidden, including null | Current catalog and markers |
| 2 | Required `catalog_identity`; forbidden `marker_identity`, including null | Declared catalog, current markers |
| 3 | Both complete declarations required | Declared catalog and all five marker identities |

`check-config` only validates syntax/internal consistency, without a database connection or opening
the configured Artifact root. Format 3 [check-storage](platform-storage-check.md) directly calls
`check_platform_database_storage_markers_expected(config, endpoint, password, expected)`: one fresh
attempt, declared inspection and close on success/failure share one ten-second database budget.
There is no weaker-check fallback. Process/private-file/root gates, generic output and post-runtime
root/process rechecks remain unchanged. All observations are discarded, with no baseline export.

## Limits and verification

A trusted assertion does not authenticate its source. OID reuse or physical clones retaining every
configured identity cannot be distinguished. Format 3 does not pin historical Artifact-root inodes
or all business tables, establish full schema health/current Session authority, or authenticate the
peer before password delivery. Synchronous root checks retain no hard deadline. No pools, retries,
repairs, migration, listener, issuer or normal-app mounting is added;
[retained Session composition](platform-session-connection.md) remains separate.
Timeout/cancellation does not prove remote rollback or authorize automatic retry.

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_marker_expectation_values
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
bash scripts/test-platform-marker-expectation.sh
```

Only the last command needs an explicit disposable `CYANREX_TEST_DATABASE_URL`, database/role
administration and runtime-role password authentication. The runner verifies every exact selector
exists. Fixture identities come from independent administrator queries, not the tested observer;
faults affect only owned disposable storage, restored/cleaned before final assertions. These tests
are not LAN/TLS deployment or a usable running host.

On 2026-10-16, all five new and 36 adjacent SQL regressions passed on an isolated PostgreSQL instance.
The backend/common gate passed 1,196 Rust tests and 426 common checks, with 918 opt-in Rust tests
ignored by default, not counted as passes. The temporary instance was stopped and its disposable data
removed; frontend/SDK builds, deployed LAN/TLS and kernel acceptance were not rerun for this slice.
