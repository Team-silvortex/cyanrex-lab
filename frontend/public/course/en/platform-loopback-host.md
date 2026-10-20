# Platform loopback Session host

The separate [native HTTPS mode](platform-loopback-tls-host.md) adds private TLS material and bounded
handshakes. The `serve-loopback` mode below remains HTTP-only and never becomes its fallback or proxy backend.

An optional final `--notify-systemd` flag adds [service-state notifications](platform-host-notifications.md)
without changing these storage, transport or shutdown contracts. The unflagged command remains unchanged.

Included in source **0.6.3**, Linux `cyanrex-platform serve-loopback` runs the three existing [Session](platform-session-router.md)
routes plus the [private Task index](platform-task-index-connection.md), [read-only reopening](platform-task-content-connection.md)
and [empty manual Draft creation](platform-task-create-connection.md), plus [Draft manifest replacement](platform-task-replace-connection.md), in a low-privilege process under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It accepts only canonical
`127.0.0.1:PORT` or `[::1]:PORT` with port 1024–65535 and a matching HTTP Origin. It does not expose
the platform to a LAN, add TLS, or serve as a supported TLS reverse-proxy backend.

## Explicit startup

The fixed command shape is:

```text
cyanrex-platform serve-loopback --uid UID --gid GID --groups GID,...|none --config /absolute/private/input.json --bind 127.0.0.1:PORT
```

Supply the actual nonroot process UID, GID and exact allowed supplementary groups, not requested
credentials to acquire. The [process gate](platform-process-startup.md) rejects unexpected identity,
effective/permitted/inheritable/ambient capabilities and threads before secrets or runtime creation;
it sets and verifies `no_new_privs=1`. A nonempty capability bounding set alone is allowed. These
checks do not make a sandbox and do not lower credentials automatically.

The [private input](platform-startup-input.md) must be format 3 with complete trusted
[catalog and five-marker identities](platform-marker-expectation.md). Formats 1 and 2 remain valid
for their old diagnostics but cannot run this host. The Origin must equal `http://` followed by the
canonical bind authority, including its explicit port. DNS names, `localhost`, other 127/8 addresses,
mapped/scoped IPv6, wildcards, LAN addresses, low ports and noncanonical spellings are rejected.

Use an owned 0700 final configuration directory and an owned single-link 0400 or 0600 regular file.
The existing descriptor-relative, no-follow, 16 KiB reader and strict JSON validation remain intact.
The host neither generates declarations nor converts a successful diagnostic into a runtime binding.
Keep the file and its database password private; dropping in-memory options is not secure zeroization.

The startup sequence prepares and rechecks the single main thread, loads and rechecks private input,
validates mode/Origin/bind, observes the selected owned Artifact root, and only then creates the
current-thread IO/time runtime. One `open_platform_storage_expected` call verifies the complete original
catalog and markers, closes the initial connection and rechecks the root before handing off storage.
The root observation is selected at startup: format 3 does not declare a historical root inode.
`PGOPTIONS`, even empty, is rejected; there is no environment database fallback, installer, pool,
migration, baseline adoption or automatic retry. Explicit local database peer trust remains necessary.

The host checks the current thread after storage work even on failure, builds one shared workspace state,
registers SIGTERM/SIGINT and binds the literal address last. It rechecks the thread before printing
`platform loopback host ready`. Binding may briefly succeed before a subsequent process check fails;
absence of Ready is not proof that no socket was ever bound.

Startup still requires the configured role's original preflight privileges; it does not add login,
query, logout or Task business privileges. Trusted provisioning and account creation remain separate.

## HTTP ingress

The private Hyper HTTP/1.1 driver admits at most 16 concurrent connections. Each connection has one
request, at most 32 headers, a 16 KiB parser buffer and a two-second complete-header deadline including
an idle first byte. A full host closes excess accepted sockets without queueing a waiting request.
The connection permit covers response writing; finished tasks are reaped instead of accumulating.

Requests must use HTTP/1.1, origin-form URI and exactly one Host equal to the selected canonical
authority. CONNECT and Upgrade are rejected. Forwarded headers do not select authority, scheme or
database. HTTP/2, WebSocket, connection reuse and a second pipelined command are unsupported.

Host-gate rejections have empty private responses with no-store/no-cache/nosniff headers. Raw parser
failures can instead close the connection or emit parser-generated errors without the router's
private-header middleware. Neither path is a business JSON contract, Cookie issuance or CORS service.

Only POST `/platform/v1/session/login`, GET `/platform/v1/session` and POST
`/platform/v1/session/logout`, GET `/platform/v1/task-index`, GET `/platform/v1/tasks/{task_id}` and
POST `/platform/v1/tasks` and PUT `/platform/v1/tasks/{task_id}/content` reach handlers. The create body contains only `schema_version: 1`, `task_id`
and a nonblank title; owner, scope, initial Draft status and empty payload derive on the server. Code and
a Run are not required. The separate replacement PUT accepts only `schema_version`, `expected_revision`
and `manifest`, changing a manual Draft title or its existing exact owned text references. Neither write
publishes Artifacts or connects browser saving. Strict DTO/body, Origin, marker, Cookie,
method and admission rules remain unchanged, including the 32 KiB/two-second login-body limit,
shared 64-per-60-second eligible login budget and six separate login/lifecycle/index/content/create/replace response lanes.
Each lane defaults to four slots, configurable to 1–16 through trusted composition. The old three/four/five-capacity
constructors keep their signatures and default replacement to four; `with_replace_capacities` validates all six
before splitting the original binding. Both write bodies retain the existing 64 KiB/two-second bound.
There is no timeout around a complete connection or a dispatched business operation.

Confirmed login sets the dedicated host-only HttpOnly SameSite=Strict Cookie at Path `/platform/v1`;
loopback HTTP omits Secure. Confirmed logout alone clears it. TCP delivery does not prove browser
receipt. Cookies are not isolated by port: use a trusted local account and plan a separate Cookie-safe
HTTPS host before browser or LAN integration. Do not forward this plaintext listener as a TLS backend.

## Shutdown and failure

SIGTERM or SIGINT stops acceptance and asks existing connections to shut down gracefully. A soft
30-second drain then aborts and joins remaining local futures and reports failure. A one-second
runtime shutdown wait bounds waiting, not termination of already dispatched password work.
Synchronous filesystem checks can stall the current thread and its timers; there is no hard process
exit deadline. Peer protocol failures close that connection; accept failures or task panics fail the
host and drain its other work without retrying.

A clean signal-driven drain prints `platform loopback host stopped`. Handled startup or running errors
use the same neutral stderr `platform loopback host failed` and exit 1; invalid command grammar exits 64.
Only this serve mode installs a panic hook that omits the panic payload, location and backtrace and
prints that fixed notice. A task panic can produce the notice again when the host returns failure;
main-thread panic exit codes, abort and OOM are not normal Result failures. Ordinary error output
contains no driver details, paths, password or tokens. A running failure does not claim that no host
started. Cancellation, disconnect or shutdown does not prove SQL rollback, undo an issued Session,
restore OTP state or authorize retries and compensation.

## Verification and remaining work

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_serve_loopback_defaults_tdd
bash scripts/test-platform-loopback-host.sh
```

Default tests exercise actual TCP with private synthetic routers and actual CLI rejection paths;
synthetic transport tests do not bypass production startup. The eight exact PostgreSQL selections
require disposable `CYANREX_TEST_DATABASE_URL` administration and verify real child-process startup,
login/query/logout, original identity/root drift, permissions, observed waits and signal shutdown.

This explicit binary mode does not mount the normal teaching AppState, frontend authentication,
Artifact publication, Task status writes, standalone Artifact routes, OpenAPI or SDK operations.
Task writes are limited to empty Draft POST and expected-revision manifest PUT. TLS/domain planning, authorized LAN access,
installed service supervision and browser integration remain separate work.
