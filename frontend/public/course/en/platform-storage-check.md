# One shot platform storage check

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This **0.6.0** follow-up, Unreleased on **2026-10-15**, adds an explicit Linux `check-storage`
command under [platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
It joins the existing private-input, process, Artifact-root, database-catalog and storage-marker
checks into one diagnostic. It does not start a host or return a connection, retained baseline,
authorization or reusable readiness receipt. The teaching Engine and its startup remain unchanged.

## Run against an explicitly selected target

Use the same strict version 1 or version 2 JSON and private-file policy as
[check-config](platform-startup-input.md). Only after independently reviewing the service identity,
groups and local database route, run:

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-storage --uid 1000 --gid 1000 --groups 1000 --config /private/platform/input.json
```

The numbers and path above are examples, not discovered defaults. Argument order is fixed;
`--groups none` explicitly selects no supplementary groups. Syntax errors exit 64. All process,
input, root, database and close failures exit 1 with the same fixed error, without printing paths,
passwords or raw driver/SQL messages. Success exits 0 and prints only
`platform storage observations completed; no host started`.

Unlike `check-config`, this command opens the configured existing root and one local database
connection. A syntactically valid configuration or nonexistent root can pass `check-config` but
cannot by itself pass `check-storage`.

## Sequence and limits

The command first prepares the explicit low-privilege process, reads the private input and rechecks
single-main-thread state. It then observes the existing effective-UID-owned `0700` Artifact root,
without creating it or enumerating contents, before constructing a current-thread Tokio runtime.

The runtime invokes `check_platform_database_storage(config, endpoint, password)`. This separate
library diagnostic shares only the connector's internal explicit options/dial builders; it does not
construct a `PlatformDatabaseConnector` from a newly discovered observation. One ten-second budget
covers a single connection attempt, the original read-only catalog transaction, the original read-only
five-marker transaction and explicit connection close. Inspection failure also goes through bounded
close; inspection or close failure cannot produce success.

With a [version-2 catalog declaration](platform-catalog-expectation.md), it instead calls
`check_platform_database_storage_expected(config, endpoint, password, expected)` using the same
single-attempt core. Complete-config mismatch is rejected before dialing; the actual catalog must
match all declared identities before marker inspection. Version 1 keeps the original diagnostic path.

After the runtime is dropped, the command rechecks the original Artifact root and process, even if
database work failed. Root metadata I/O is outside the database deadline: synchronous filesystems,
including NFS/FUSE, have no hard time bound here. The database transactions and root snapshots are
not a cross-resource atomic snapshot or a lock against same-UID/root changes. Timeout or termination
drops unpublished client state, not proof that a remote server has already completed rollback.

The [catalog contract](platform-host-preflight.md) checks the configured system identifier/database
OID, exact role, listed role/namespace restrictions and Source-only search path. Empty namespaces
do not suffice: the [five-marker contract](platform-storage-markers.md) must also pass, including
fixed versions and configured scope fields. Only the necessary marker metadata is read, not users,
credentials, Sessions, Task/Artifact content or audit history. The Artifact check reads metadata only.

Version 1 checks the objects observed during this invocation. A valid same-name substitution
before the initial observations can pass; version 1 supplies no prior namespace/marker/root identity.
Version 2 rejects catalog identity changes against its explicit declaration, including replacement
before the first connection, but still supplies no historical marker/root identity.
The temporary catalog observation only binds marker inspection within this invocation;
all observations are discarded. Future host startup still needs its separately reviewed original
baseline. Success is not full schema compatibility, business health, writeability, permission-history
validation, current Session authorization or deployment acceptance.

## Connection and secret boundaries

Transport remains the [explicit local endpoint policy](platform-database-connection.md): canonical
Unix sockets or literal loopback IPs, with SSL disabled. The operator must independently trust the
local peer before password delivery; physical identity is checked after authentication, not TLS or
peer authentication. No URL, pgpass, Engine defaults or `.env` is loaded. Captured PGOPTIONS,
including empty, is rejected before networking; the root's metadata I/O may occur earlier.

There is no reconnect, pool, fallback, installation, migration, permission repair or Session issuance.
The CLI installs no driver logging subscriber and emits only fixed messages. Library callers still
own their global logging policy, including server notices, and process preparation. Private input
is neither cloned nor reported; dropping its buffers is not secure zeroization.

## Verification

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_check_defaults_tdd
./scripts/test-platform-storage-check.sh
```

The second command requires an explicit disposable `CYANREX_TEST_DATABASE_URL` with role/database
administration and password authentication for runtime roles. Administrator-only local trust is
allowed; trust for every role cannot validate wrong-password refusal. CI reuses its existing
PostgreSQL service and runs all five exact SQL selectors after verifying that each exists.

Default tests use real loopback peers for startup error redaction, the actual handshake deadline,
socket closure and absence of a second attempt. PGOPTIONS and unsafe-root tests observe no dial;
inotify positive controls verify UID rejection before opening the configuration FIFO. These peer
tests do not cover cancellation during authenticated catalog work.

The SQL CLI tests install real templates only in owned fixtures and use a role with marker metadata
access but no business-column reads. They cover success and wrong password/coordinates, unsafe
privileges and marker failures without repairs. Independent lock observations identify actual marker
waits before root replacement or termination of the direct child. Data/input snapshots remain equal,
remaining diagnostic backends reach zero, and fixture resources are cleaned before final assertions.
This is local diagnostic evidence, not real multi-machine LAN/TLS or browser-save acceptance.

The separate [retained storage connector](platform-storage-connection.md) accepts independently
selected original observations and can return a fresh verified connection. This CLI remains a one-shot
diagnostic: it neither constructs that connector nor exports or adopts observations for it.
