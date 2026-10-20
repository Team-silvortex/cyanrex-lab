# Open retained platform storage from trusted declarations

Included in source **0.6.3**, `open_platform_storage_expected` connects complete trusted
[catalog and five-marker declarations](platform-marker-expectation.md) to a genuine
[retained storage connector](platform-storage-connection.md), under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). Unlike a diagnostic,
confirmed success returns a capability that retains the verified bindings. It starts no host,
mounts no route, installs no schema and grants no current Session authority.

## Explicit inputs

The Unix library function consumes one `PlatformHostPreflightConfig`, local
`PlatformDatabaseEndpoint`, complete `PlatformStorageMarkerExpectation` and genuine
`PlatformArtifactRootObservation`, borrowing the exact root path and password. The password is copied
into the private connection options retained by the result; dropping these values is not secure
zeroization. It returns `Result<PlatformStorageConnector, PlatformStorageConnectionError>`.

The trusted caller must independently select the catalog/marker identities and the original root
observation. A declaration is not a fabricated observation, and an owned root opened now cannot prove
its historical identity. Matching names, versions and scopes alone cannot fill missing declarations.
There are no current-only or catalog-only overloads, baseline exports or update methods.

Prepare the [Linux process](platform-process-startup.md) before secrets, runtime threads and dialing.
The library does not lower credentials, prepare the process or authenticate a substituted local
database peer before delivering a password. Local transport trust and the SQLx/server notice-log
policy remain the caller's responsibility.

## Opening sequence

1. Compare the complete declared configuration and original root's raw path/configuration before IO.
   Inconsistent values return `InvalidConfig`; a spelling such as trailing `/.` is not the original path.
2. Recheck the caller's original private root, not a newly adopted directory. A replaced or unsafe
   root returns `Unavailable` before dialing.
3. Build the existing explicit SQLx options once. Reject invalid passwords and captured `PGOPTIONS`,
   including an empty value, as `InvalidConfig`; no process environment is changed.
4. Within one ten-second database budget, dial once, inspect the complete expected catalog and all
   five markers using the existing read-only transaction, then close the initial connection after
   either inspection outcome. A successful inspection is insufficient if the client close call fails.
5. Recheck the original root after the database phase, including a normally returned failure or
   timeout. Only if inspection, close and root checks all succeed, compose the retained connector
   from those genuine marker observations, the consumed catalog declaration, original root and
   already prepared options. No second environment read, connection or observation is used to
   assemble the result.

Success confirms completion of the client close call, not a server Terminate acknowledgement or
observed backend disappearance. The returned connector has no live initial connection, pool, secret accessor, Debug, Clone or
serialization. Its later explicit `connect()` calls continue to compare the original catalog,
five markers and root; they do not refresh a baseline. Business commands must retain their own
transaction-owned fresh Session, namespace and resource checks.

## Failure and cancellation

Observation, dialing, database timeout and close failures return generic `Unavailable`, without
driver errors or secrets. The function never installs, migrates, repairs, adopts replacements,
retries or cleans application data. Every later attempt requires another complete trusted input;
a failed attempt supplies neither a binding nor authority to retry uncertain business work.

Synchronous filesystem metadata checks have no hard deadline and are outside the database budget.
Root and database observations are not a cross-resource atomic lock. Cancelling the future drops
unpublished state; it does not guarantee a final root postcheck, acknowledged server rollback or
immediate disappearance of a remote backend.

The existing `check-storage` and `check_platform_database_storage_*` diagnostics still return only
`Result<()>` and discard all observations. Formats 1–3 and their diagnostic meanings are unchanged.
This factory is a separate trusted-library API, not a way to turn diagnostic success into a runtime
baseline. The explicit [loopback host](platform-loopback-host.md) now composes it from format 3 input.

## Verification and remaining composition

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_defaults_tdd
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_tdd
bash scripts/test-platform-storage-open.sh
```

Default tests cover explicit configuration/root rejection and real local connection probes.
The exact SQL runner requires a disposable `CYANREX_TEST_DATABASE_URL` with database/role
administration and runtime-role password authentication. It verifies real declarations, readonly
permissions, retained Session lifecycle, original identity replacement, actual marker waits and
cancellation. These tests do not establish a deployed host, browser delivery or LAN/TLS acceptance.

The independent [shared Session router](platform-session-router.md) now pairs login and lifecycle
over one binding. The explicit loopback mode adds bounded local listening; TLS, platform browser
authentication and Task/Artifact/index wiring remain separate. Teaching Engine,
live routes, OpenAPI and SDK contracts are unchanged.
