# Existing Sessions over retained platform storage

Included in **0.6.2** on **2026-10-16**, this follow-up after **0.6.1** connects the
[retained storage connector](platform-storage-connection.md) to existing Session query/logout and
the explicit [Session HTTP adapter](platform-session-lifecycle.md). It follows
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3); no listener or issuer is started.

## Explicit composition

`PlatformSessionConnector::new(storage)` consumes a genuine `PlatformStorageConnector` without IO.
Authority and Origin come from that connector's complete original configuration, not a second caller
argument. The wrapper has no Clone, Debug, serialization, pool, secret accessor or baseline updater.
Its only commands are `validate_session(token)` and `logout(token)`; `origin()` exposes public metadata.

`SessionLifecycleHttpState::with_platform_storage(storage, max_in_flight)` selects this backend and
inherits its Origin. The original `new(source, origin)` and `with_capacity` remain compatible.
Each state selects exactly one backend, never a pooled fallback after platform failure. Clones share
the existing response-lifetime admission budget; separately constructed states remain independent.

Construction does not obtain or adopt observations. Trusted callers must select the original
catalog, five markers and private root independently, and apply the separate
[Linux process gate](platform-process-startup.md) before secrets, workers or connections.
The check-only CLI neither constructs this wrapper nor returns observations for it.

## Transaction sequence

Each canonical token command obtains **one fresh physical connection**, through the original root
precheck, bounded catalog/marker verification and root postcheck. It then owns a READ COMMITTED
business transaction on that same connection, preserving any explicit READ ONLY policy:

1. Verify the retained original catalog and all five marker identities, shapes, versions and scopes
   inside the business transaction; retain their relation locks through completion.
2. Prepare the original Source-only Session operation. Capture namespace, raw search path and
   relation pins, lock the Source row, then lock/read the exact Session/account or delete its digest.
3. Verify the same retained catalog/markers again, without changing the baseline or search path.
4. Finish the original Source checks, then read fresh Session expiry or verify exact-token absence.
   No additional storage guard runs after this final read and before the COMMIT attempt.
5. Return success only after confirmed COMMIT, then close the connection without retry.

The shared internal preparation/finish helpers also serve the old pooled Session APIs, but are not
public partial commands or reusable authorization. Existing login, credentials, cleanup, resource
commands and their transaction boundaries remain separate. The platform logout path additionally
flushes deferred constraints/triggers before its final storage/Source/absence checks.

Malformed query tokens return no Session without connecting; malformed logout tokens are invalid
input. Canonical unknown or expired tokens may confirm logout without a separate validate precheck.
Only confirmed HTTP logout clears `cyanrex_platform_session`; storage/Source/commit failure returns
private `503 service_unavailable`, `unconfirmed`, and leaves both platform and teaching Cookies alone.

## Permissions and failure boundaries

The runtime role combines the five-marker SELECT grants with the existing
[Session column grants](platform-session-permissions.md), plus Source-row locking rights. It needs
an explicitly non-read-only policy for row locks/logout; runtime code never uses SET READ WRITE to
bypass a read-only role. Password salt/hash, TOTP secret and OTP watermark need not be readable.
No tables, users, grants, schemas, files or Sessions are installed by these commands.

Connection/database preparation, business work and graceful close have separate ten-second budgets.
Synchronous root checks remain outside a hard deadline. Root identity is checked for connection
admission, not continuously locked during SQL. Marker observations do not pin the historical identity
of every business table: Session relation pins are captured and retained within each command.
These are not cross-resource atomicity, a global connection limit or a complete host health check.
Timeout/cancellation or an unconfirmed response does not prove rollback or permit automatic replay;
a confirmed commit remains confirmed even if subsequent graceful close fails.

## Verification and remaining work

Default transport/backend regressions run with:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_backend_tdd
```

For actual database/role/trigger/lock tests, provide only a disposable `CYANREX_TEST_DATABASE_URL`
with database/role administration and runtime-role password authentication, then run:

```bash
bash scripts/test-platform-session-connection.sh
```

The runner checks every exact selector exists before execution. Tests use committed synthetic
existing Sessions, not public login/OTP issuance, and remove their own roots, databases and roles.
The seven cases cover credential-restricted success, read-only/missing grants, original marker/root
replacement, expiry after an observed Source-row wait, namespace-owner drift and cancellation during
that wait, redirected DELETE and deferred Session reinsertion. On 2026-10-16, all seven and 64 adjacent
Session/OTP/storage SQL regressions passed on an isolated PostgreSQL instance; that instance was then
stopped and its disposable data removed.

Normal application/CLI startup, LAN TLS, public Session delivery, private Task resource composition,
browser saving, SDK/OpenAPI exposure, migration and recovery are still separate work.
