# Platform connections with retained storage bindings

Source **0.6.3** includes explicit loopback/TLS hosting, limited Session issuance and private
Task create/list/reopen/manifest replacement. Browser/SDK saving, new-byte publication, LAN access
and migration remain absent; earlier implementation-stage labels retain their original scope.

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This Unreleased follow-up after **0.6.0**, under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3), composes the existing
database connector, five storage-marker observations and private Artifact-root observation.
Each explicit call returns one fresh connection only after comparing all three original bindings.
It is an unmounted library, not a running platform host or current Session authorization.

The separate Unreleased [expected-storage opener](platform-storage-open.md) now inspects complete
trusted declarations and confirms the initial close before returning this retained capability.

## Explicit construction

`PlatformStorageConnector::new(config, database, original_markers, root_path, original_root)`
consumes a trusted `PlatformDatabaseConnector` and genuine, caller-selected marker/root observations.
Construction performs no filesystem or database I/O. It rejects inconsistent complete configurations,
a database binding that does not match the marker observation's catalog, or a different raw root path,
using fixed `InvalidConfig`. Even a spelling such as a trailing `/.` is not accepted as the original path.

The database connector may hold either an [original observation](platform-database-connection.md) or
an [explicit catalog declaration](platform-catalog-expectation.md). Both must match the genuine catalog
inside the supplied [marker observation](platform-storage-markers.md), not just its names or scope.
A declaration is never converted into an observation.

“Original” means the observation time independently selected by the trusted caller. The constructor
does not discover, inspect or adopt a first runtime baseline, export observations from diagnostics,
or prove identities from before those observations. In particular,
[check-storage](platform-storage-check.md) still discards its observations and cannot seed this API.
The caller must separately obtain and trust the observations; same-name substitutions predating that
trusted observation are not historical identity evidence.

The retained root holds its original private descriptor. The new connector has no Debug, Clone,
serialization, password/options accessor, public root descriptor or baseline-update API.

## Connection sequence and failure handling

`connect().await` first rechecks the original [Artifact root](platform-artifact-root.md) synchronously.
An unsafe or replaced root fails before dialing. It then runs one database phase with a shared
ten-second budget: one fresh database connection, the original catalog verification, then verification
of the original five marker identities, versions, owners and scopes. Marker inspection failure closes
the unpublished connection within that same budget; cancellation or expiry drops unpublished state.

After that phase, it rechecks the original root even if database work failed. Only successful database
and root checks publish the caller-owned `PgConnection`. If database work succeeded but the root
postcheck failed, a separate ten-second close budget applies. Any failure returns fixed `Unavailable`,
without raw filesystem, SQL or driver details; no failure adopts replacement identities or reconnects.

Both filesystem checks are outside the database budget and have **no hard time bound**, including on
NFS/FUSE. A verified connection is privately held during the final root check. Therefore this is not
a ten-second total connection operation. The two database transactions and root checks are separate
snapshots, not a cross-resource atomic lock or continuous identity protection.

Restoring exact original objects may allow a later explicit call. There is no automatic retry,
fallback, migration, installation, permission repair, cleanup of application data or permission to
replay uncertain business commands. Timeout/cancellation does not prove that a remote server has
finished rollback.

## Caller responsibilities and remaining integration

The result is **not forced read-only**. Only verification transactions are read-only; the caller can
later change connection state. Business commands still need their original transaction-owned Source,
resource and fresh Session authorization guards. Marker acceptance is not complete schema or business
health, and root verification reads metadata rather than content.

The existing [local endpoint and secret policy](platform-database-connection.md) remains unchanged:
physical identity is checked after database authentication, not before password delivery. Independently
trust the local peer and handle logging, including server notices. No new SQLx options, environment
defaults, pool, listener, issuer, CLI mode or configuration format is added.

This Unix library does not prepare a process. A trusted host must separately apply the
[Linux process gate](platform-process-startup.md) before secrets, workers and connections.
A 2026-10-16 Unreleased [existing-Session adapter](platform-session-connection.md) now consumes this
connector without a pool and rechecks its original catalog/markers inside query/logout transactions.
Resource adapters, normal platform startup, secure Session issuance and browser saving remain
separate; no teaching Engine is constructed or switched.

## Verification

```bash
bash scripts/test-platform-storage-connection.sh
```

Use an explicit disposable `CYANREX_TEST_DATABASE_URL` with role/database administration and runtime-role
password authentication; administrator-only local trust is allowed. The runner verifies its exact test
names before executing them. Tests cover compatible observations/declarations, mixed bindings, first-call
marker/root replacement, drift without repair and exact restoration. Real loopback probes check
precheck refusal. An inotify positive control separates precheck opens from the root postcheck after
an accepted database peer fails. Independently observed PostgreSQL marker waits exercise root replacement
and cancellation of the composed connection future, followed by explicit healthy calls. Owned resources
are reclaimed before final assertions.
These are local regressions, not LAN/TLS deployment, public authorization or browser-save acceptance.
