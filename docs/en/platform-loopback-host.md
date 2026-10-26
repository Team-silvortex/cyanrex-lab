# Platform loopback Session host

The separate [native HTTPS mode](platform-loopback-tls-host.md) adds private TLS material and bounded
handshakes. The `serve-loopback` mode below remains HTTP-only and never becomes its fallback or proxy backend.

Optional trailing flags are ordered `[--notify-systemd] [--session-ui]`. The first adds
[service-state notifications](platform-host-notifications.md); the second enables the Session/private Task
browser shell described below. Both default off; unflagged startup remains unchanged.

Included in source **0.6.4**, Linux `cyanrex-platform serve-loopback` runs the three existing [Session](platform-session-router.md)
routes plus the [private Task index](platform-task-index-connection.md), [read-only reopening](platform-task-content-connection.md)
and [empty manual Draft creation](platform-task-create-connection.md), plus [Draft manifest replacement](platform-task-replace-connection.md), in a low-privilege process under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It accepts only canonical
`127.0.0.1:PORT` or `[::1]:PORT` with port 1024–65535 and a matching HTTP Origin. It does not expose
the platform to a LAN, add TLS, or serve as a supported TLS reverse-proxy backend.

Included in 0.6.4, [retained text publication](platform-text-artifact-connection.md) is explicit in both
CLI hosts. This separate POST is not browser text saving or the older pooled text/read router.

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

Startup still requires the configured role's original preflight and business privileges; it grants
none automatically. Publication adds only INSERT on `collaboration_artifacts`,
`collaboration_artifact_revisions` and `collaboration_artifact_outbox` in the configured Artifact
namespace. Keep existing login credential SELECT, Task writes and Artifact/Task row-lock column
grants: the combined role serves all lanes. No broad UPDATE/DELETE/DDL is added. Trusted provisioning
and account creation remain separate.

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

The resource handlers remain POST `/platform/v1/session/login`, GET `/platform/v1/session` and POST
`/platform/v1/session/logout`, GET `/platform/v1/task-index`, GET `/platform/v1/tasks/{task_id}` and
POST `/platform/v1/tasks`, PUT `/platform/v1/tasks/{task_id}/content` and POST
`/platform/v1/text-artifacts`. The Task create body contains only `schema_version: 1`, `task_id`
and a nonblank title; owner, scope, initial Draft status and empty payload derive on the server. Code and
a Run are not required. The separate replacement PUT accepts only `schema_version`, `expected_revision`
and `manifest`, changing a manual Draft title or its existing exact owned text references. Neither write
publishes Artifacts. The optional shell's metadata-only save reuses that PUT. The separate text POST accepts only schema version,
Artifact/revision IDs, title and text; confirmed 201 returns metadata without echoing text.
Its actual wire-body limit is 2 MiB/two seconds, independently of the decoded 256 KiB text limit.
The 16 KiB Hyper parser buffer remains; it is not a total-body limit. Strict DTO/body, Origin, marker, Cookie,
method and admission rules remain unchanged, including the 32 KiB/two-second login-body limit,
shared 64-per-60-second eligible login budget and seven separate login/lifecycle/index/content/create/replace/publish response lanes.
Both CLI hosts use `with_text_publication_capacities` with four slots per lane; trusted capacities are 1–16.
It validates all seven before splitting one original storage Arc into seven capabilities. Old constructors
retain seven routes/six lanes with publication disabled; `with_replace_capacities` still checks all six.
The global 16-connection gate remains, not a promise of 28 concurrent requests. Both Task write bodies
retain the existing 64 KiB/two-second bound; text publication retains four separate actual file-job slots.
There is no timeout around a complete connection or a dispatched business operation.

Confirmed login sets the dedicated host-only HttpOnly SameSite=Strict Cookie at Path `/platform/v1`;
loopback HTTP omits Secure. Confirmed logout alone clears it. TCP delivery does not prove browser
receipt. Cookies are not isolated by port: use a trusted local account and plan a separate Cookie-safe
HTTPS host before broader browser or LAN deployment. Do not forward this plaintext listener as a TLS backend.

## Optional browser Session shell

Explicit `--session-ui` enables only seven compiled assets: `/platform/`, `/platform/ui/app.js`,
`/platform/ui/labels.js`, `/platform/ui/tasks.js`, `/platform/ui/task-edit.js`, `/platform/ui/task-contract.js` and
`/platform/ui/style.css`. Without it those paths remain absent. Open
`/platform/` on the configured literal-loopback Origin; the shell offers English, Simplified Chinese,
Spanish, Japanese and Korean with an in-memory language selector. It is not Next.js, a directory
server, a proxy or a replacement for the teaching application.

External scripts/styles and a restrictive CSP keep the shell on the same Origin. It calls only the
existing dedicated platform Session login/check/confirmed logout, private Task index/content GETs and
the metadata-only use of the existing manifest PUT described below. Login success
is followed by one read-only check of the browser Cookie roundtrip; it does not establish a username
or role. Password/OTP fields clear on handoff, not by secure memory erasure. No secret is put in a URL,
browser storage or logs. Unknown/malformed replies and the ten-second browser waiting deadline remain
unknown, not proof of rollback; writes are never retried automatically. Logout requires confirmation
with Cancel focused by default. Cookie host/Path/Secure policy and non-port-isolation are unchanged.

After an active Session observation, users explicitly load 20-entry owner-private pages and select
a Task to reopen; neither listing nor selecting is automatic. Pages replace rather than accumulate.
Index metadata does not prove content readability. Empty payloads are valid; text is displayed inertly,
including duplicate filenames and unknown valid language hints, without an editor or Run.

Index wire data is bounded to 128 KiB and content wire data to 50 MiB, with at most 16,384 body frames
and a ten-second browser waiting bound. Parsing is not preempted by that timer. Strict JSON rejects
duplicate/escaped keys and unsafe numbers; exact scope/revision/ordered pins and local SHA-256 must
match before display. Decoded content remains at most 32 items of 256 KiB each. Task 401/403, a new
Session check, confirmed-logout dispatch and pagehide clear old private data; epoch checks discard
late replies. Text editing, Task creation and execution are not connected; the normal Next.js local
draft editor and generic save-attempt helper remain separate. No authentication API or database grant is added.

The **0.6.4 metadata-save slice** lets the owner explicitly edit only an existing manual Draft's
title and existing payload display filenames/language hints. Artifact pins, order, bytes and item count
cannot change. A target/revision-bound preview defaults to Cancel; confirmation sends one expected-revision
PUT with a complete canonical body at most 64 KiB. Only HTTP 200 plus a strict, matching three-field
Task/manifest receipt confirms the metadata save. Its reply is bounded to 128 KiB/4,096 frames with a
ten-second browser wait, not a parse/server deadline. Displayed text remains the previously checked copy,
not a fresh content read. Conflict, malformed/lost replies and cancellation retain uncertainty without replay.

Dirty or unknown work guards Task navigation, Session checks, login and logout with an explicit
discard dialog, again defaulting to Cancel. Metadata 401/403 headers immediately clear private DOM;
context changes/pagehide also isolate work in closed page memory. A later Session cannot restore or replay it.
Explicit discard is required before continuing. This is neither secure erasure nor rollback or recovery.
No raw-text edit, new Artifact, SDK integration, new HTTP API or database privilege is introduced.

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
bash scripts/test-platform-text-publication-host.sh
bash scripts/test-platform-task-reader-browser.sh
bash scripts/test-platform-task-metadata-browser.sh
bash scripts/test-platform-browser-session.sh
bash scripts/test-platform-browser-tasks.sh
bash scripts/test-platform-browser-task-metadata.sh
```

Default tests exercise actual TCP with private synthetic routers and actual CLI rejection paths;
synthetic transport tests do not bypass production startup. The eight exact PostgreSQL selections
require disposable `CYANREX_TEST_DATABASE_URL` administration and verify real child-process startup,
login/query/logout, original identity/root drift, permissions, observed waits and signal shutdown.
The publication runner separately inventories six real SQL/workspace/plain/TLS selectors; inventory
is not a passing result or browser acceptance.

The browser Session runner inventories two real SQL/browser selectors for plain and TLS hosts. It
requires an explicit disposable `CYANREX_TEST_DATABASE_URL`, absolute `CYANREX_PLAYWRIGHT_MODULE`
and `CYANREX_CHROMIUM_PATH`, OpenSSL, `certutil` and usable isolated browser/NSS trust. It does not
modify user/system trust or bypass certificate checks. The local legacy NSS environment currently
blocks TLS-browser verification; native TLS acceptance is not evidence that a browser TLS flow passed.

The Task-reader browser runner uses production assets with mock APIs; its 12 cases are UI evidence,
not SQL authorization. The separate Task browser runner selects one real plain-host SQL/browser case
for private paging/reopening and revocation without Task/Artifact writes; it does not claim TLS-browser coverage.
The metadata fixture separately inventories 12 mock-API browser cases; its SQL/browser runner selects
one real plain-host metadata-save/conflict/revocation case. These are separate from TLS-browser acceptance.

This explicit binary mode does not mount the normal teaching AppState or Next.js frontend authentication,
Task status writes, pooled Artifact read routes, OpenAPI or SDK operations. Task writes are limited to
empty Draft POST and expected-revision manifest PUT; the separate text POST creates only private Artifacts.
TLS/domain planning, authorized LAN access,
installed service supervision and browser Task-saving integration remain separate work.
