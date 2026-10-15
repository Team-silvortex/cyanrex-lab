# Explicit platform database connections

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This 2026-10-15 Unreleased follow-up after **0.6.0** adds a single-attempt connector under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). Each new connection
is compared with an original [catalog observation](platform-host-preflight.md) or explicit trusted
[catalog declaration](platform-catalog-expectation.md) before it reaches the caller.
It is not a pool, host listener, Session issuer or storage installer. Teaching startup is unchanged.

## Trusted construction

`PlatformDatabaseConnector::new(config, original, endpoint, password)` consumes the original
observation and requires its complete host configuration to match. It neither creates a new baseline
nor adopts a matching name. Configuration still includes the exact physical database and role,
three namespaces, origin, workspace and Session limits; these are not business/schema health proofs.

The alternative `::with_catalog_expectation(config, expected, endpoint, password)` consumes an explicit
[catalog declaration](platform-catalog-expectation.md) supplied independently before the first connection.
It requires the same complete-config match and never fills identities from the first read.

`PlatformDatabaseEndpoint::unix(directory, port)` accepts a canonical absolute UTF-8 directory,
without empty/dot/parent components or control characters. The complete socket filename has a
conservative 100-byte limit. `::loopback(address, port)` accepts only literal loopback IP addresses,
not DNS, wildcard or LAN addresses. Ports are explicit and nonzero.

The explicit password may be empty, is bounded to 4096 UTF-8 bytes and cannot contain NUL. Empty is
an explicit value, never a request to consult a password file. The connector has no Debug, Clone,
serialization or option/password accessor. The caller remains responsible for handling its own secrets.

## No environment authority

SQLx 0.8.6 `new_without_pgpass` still reads environment internally. This connector does not claim
zero environment reads: it rejects any captured PGOPTIONS, including an empty string, before
networking, because the SQLx options setter appends rather than replaces. All active host, socket,
port, database, role, password and application settings are explicitly selected. It never parses a
connection URL, loads pgpass, clears global PG variables or loads Engine environment defaults.

Startup options set only the Source search path, 5000 ms statement timeout, 2000 ms lock timeout
and 10000 ms idle-in-transaction timeout; application name is `cyanrex-platform-connection`.
Ordinary/slow SQL statement logging is disabled. Other driver logs remain the caller's responsibility:
server notices can still reach `sqlx::postgres::notice` through a global subscriber.

The supported local transports explicitly use SSL Disable; inherited PGSSL certificate values remain
inert, not activated as fallback. **Physical identity is checked after authentication, not before
password delivery.** Independently trust the socket path or loopback route before supplying secrets.
This is not TLS/peer authentication, remote database support or deployed-LAN acceptance.

## One attempt and the original target

`connect().await` opens one fresh `PgConnection`, then calls the existing catalog verifier against
the original observation, or inspects and compares the explicit declaration, within one overall
ten-second deadline. The dial stage also has a ten-second
ceiling. There is no pool callback, automatic reconnect, retry, fallback address or replacement observation.
Only a verified connection is returned; cancellation or failure drops unpublished connection state.
Raw driver, SQL and network errors are mapped to the fixed `Unavailable` error.

Every explicit call repeats role safety, physical target and original namespace/owner comparisons.
An otherwise valid same-name replacement is rejected. Restoring the exact original identity may allow
a later explicit call; it never causes automatic reconnection or authorizes replaying business commands.

The returned connection belongs to the trusted caller. It is **not forced read-only**, continuously
pinned or a current Session authorization. Its verification transaction alone is read-only; callers can
change connection state afterward. Business commands must retain their original transaction-owned
source/resource guards. Empty namespaces can pass; storage markers, content and audit history are not read.

## Verification and remaining integration

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_host::database_connection
./scripts/test-platform-database-connection.sh
```

The second command requires an explicit disposable `CYANREX_TEST_DATABASE_URL` with role/database
administration; it never falls back to the application URL. Four exact SQL selections cover distinct
fresh backends, settings, valid same-name namespace replacement, owner drift and privilege/control-function
loss. The fixture's read-only state comes from its own role policy; this is not SCRAM authentication evidence.
Owned databases, roles and connections are removed before final behavior assertions. CI reuses its existing
disposable PostgreSQL service.

Default tests isolate PG-variable pollution in child processes and use a private FIFO to detect forbidden
password-file reads. Separate real loopback peers exercise internal dial failure, cancellation and the
actual handshake deadline; they do not cover public connect cancellation during catalog verification.
This library is not yet composed with the [Linux startup gate](platform-process-startup.md), a pool,
platform routes or browser saving. No installation, migration, privilege repair or Session issuance occurs.

A separate [retained storage connector](platform-storage-connection.md) composes this connector with
caller-selected original marker and root observations. It verifies them before publishing a fresh
connection, without changing this lower-level API or turning diagnostics into a retained baseline.
