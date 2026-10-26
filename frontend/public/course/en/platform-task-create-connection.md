# Retained platform empty Task creation

Included in source **0.6.3**, the independent platform host can create an owner-private empty manual
Task under [host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It completes the
small backend chain: create an empty Draft → [list private metadata](platform-task-index-connection.md)
→ [reopen the saved Task](platform-task-content-connection.md). Code is optional, not a prerequisite;
no editor, assessment or Run is required. Separate [Draft manifest replacement](platform-task-replace-connection.md)
now changes a title or existing exact owned text references. This empty-create command publishes no
Artifacts; separate [retained text publication](platform-text-artifact-connection.md) is now explicitly
hosted. This does not connect browser saving or change existing teaching authentication or local drafts.

## Strict empty-create contract

Only exact `POST /platform/v1/tasks` is accepted, with no query or trailing slash. The body must be
an object containing exactly these three fields:

```json
{"schema_version":1,"task_id":"11111111-1111-4111-8111-111111111111","title":"Group discussion"}
```

The Task identifier must be a valid nonzero typed UUID. The title uses the existing manifest rules:
nonblank, at most 256 UTF-8 bytes, without forbidden controls. Unknown/duplicate fields, arrays,
missing fields and wrong types are rejected before dispatch. In particular, the caller cannot choose
an owner, scope, namespace, status, revision, catalogue definition, manifest or payload. A successful
response is **201** with the existing `schema_version`, `task` and `manifest` projection: manual Draft,
revision 1, no definition or input references, and `payload: []`. The `contents` field is absent;
there is no Location or Set-Cookie header. Success means confirmed database COMMIT, not receipt by a client.

The current audited active Human membership and exact Session account generation determine ownership.
A teacher does not acquire another owner's private Task. An identifier occupied by a valid manual
Task of this user, or by another owner, gives the same conflict. Unsupported/corrupt owned records
instead fail closed as unavailable. Creation never adopts or overwrites an occupied Task. Concurrent
creation has at most one confirmed winner. This command is not an idempotency/recovery API.

## One retained storage and one transaction

`PlatformTaskCreateConnector::new(storage)` consumes one genuine `PlatformStorageConnector` without
I/O. Origin, scope, namespaces, catalog/five-marker identities and the root descriptor come from that
original binding, not request data, environment or a fallback pool. Each command opens one fresh
connection and a READ COMMITTED transaction; it never overrides explicit READ ONLY role policy.

Before writing, the command verifies original storage, current Source/Registry authority and Task
state. It pins the configured Artifact namespace even though no Artifact rows or files are created.
Its metadata-only validator proves all four Artifact relation locks and checks the original schema
and scope without requiring SELECT on Artifact business columns. Existing content readers retain
their previous business-read permission checks; a create-only role does not thereby gain reopening.

The shared pool-free Task policy writes the head and exactly one matching outbox record. After
`SET CONSTRAINTS ALL IMMEDIATE`, it revalidates the original Artifact target and table shape, then
rereads the exact expected Task head/event/manifest pair. This catches deferred side effects, including
Artifact RLS changes that leave its marker untouched. It restores the original Source, checks the
original root and retained storage again, and only then repeats current owner/account/membership and
fresh exact Session expiry immediately before COMMIT. No pending result is published beforehand.

Connection verification, business work and connection close retain separate ten-second budgets.
There is no HTTP timeout around dispatched mutation. Synchronous root checks can still stall the
current thread; these are not hard process deadlines or an atomic filesystem/database transaction.
A confirmed commit stays confirmed if close fails. Cancellation, timeout, disconnect or a service
error is an **unconfirmed** outcome, not rollback proof or permission to retry, adopt or clean up.
A later conflict also cannot establish what happened to an earlier unconfirmed request. The tests
exercise cancellation and dropped delivery; they do not simulate a lost COMMIT acknowledgement.

## HTTP authority and independent capacity

`TaskCreateHttpState::new(connector)` and `with_capacity(connector, capacity)` select only this
capability; `build_task_create_router` mounts only the collection route. It requires the exact selected
Origin, `x-cyanrex-platform-request: 1` and dedicated `cyanrex_platform_session` Cookie. Authorization
headers are rejected. Headers are only ingress checks: the transaction authorizes the actual write.
JSON content types and encoding rules match existing private APIs. The actual streamed body is
bounded to 64 KiB and two seconds, both ending before dispatch. Other collection methods return 405
with `Allow: POST`; the standalone read-only builder still has no collection route.

Responses retain `no-store`, `no-cache` and `nosniff`; there is no CORS, redirect, token disclosure or
Cookie issuance. Predispatch errors report `not_attempted`; all dispatched errors use `unconfirmed`,
including generic private conflicts. Diagnostics do not expose storage paths or driver errors.

Original `PlatformWorkspaceHttpState` constructors expose seven routes: three Session routes, private
index GET, Task-item GET, empty-create POST and manifest-replacement PUT. They split one retained storage
Arc into six capabilities and six independent fail-fast response-lifetime lanes, with publication disabled. Each is
1–16, default four. Clones share permits until the final body frame/drop. These are not global memory,
connection, worker, persistent-storage or distributed-abuse quotas.

`with_create_capacities(storage, login, lifecycle, index, content, create)` preserves its signature,
as do earlier `with_capacities` and `with_content_capacities`; all default the replacement lane to four.
New `with_replace_capacities(storage, login, lifecycle, index, content, create, replace)` validates all six
capacities before allocation/splitting. Both CLI hosts explicitly use `with_text_publication_capacities`
to add private text POST: eight routes/seven lanes, validating all seven before splitting the same
original storage Arc. Each CLI lane has four slots; the global 16-connection gate is not 28-request
concurrency. Session-only and standalone readonly/create-only routers
are unchanged. The full pooled write router is not merged; SDK/OpenAPI, frontend transport, LAN/DNS
listeners and teaching AppState remain separate.

## Provisioning, verification and remaining work

Trusted provisioning must supply the original Source 2, identity/access 2, Task 3 and Artifact 1
schemas, authorization/head reads and column UPDATE row-lock privileges, plus Task head/outbox INSERT.
No password/TOTP SELECT or Artifact business SELECT/write is required by this create-only path.
Reopening additionally needs its existing explicit read profile. There is no installer, migration,
automatic grant, role escalation, quota policy or automatic retry.

```bash
env CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_create
bash scripts/test-platform-task-create-connection.sh
bash scripts/test-platform-text-publication-host.sh
```

The runner requires an explicit disposable `CYANREX_TEST_DATABASE_URL` with administration privileges.
Ten HTTP units and eight exact real SQL/HTTP/host cases cover strict input, ownership, concurrent
duplicates, original binding/root drift, immediate/deferred side effects, observed wait expiry and
cancellation, narrow role permissions, response admission and an actual low-privilege loopback process.
Title/existing-reference replacement belongs to the separate retained capability, not this create-only
router. Browser saving/recovery, text or binary publication, catalogue definitions, status transitions,
Run dispatch and LAN deployment remain separate.
The second runner inventories six real SQL/workspace/plain/TLS selectors, including old-constructor
compatibility; its inventory is not a passing result. Text publication remains a separate command.
