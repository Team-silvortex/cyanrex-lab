# Retained platform Task metadata index

Included in source **0.6.3**, the independent platform host adds owner-private Task listing under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). A Task can contain no payload
items; code, an editor, a domain definition and a Run are not prerequisites. This connects metadata
discovery only. Separate [content reopening](platform-task-content-connection.md) now adds a Task-item GET;
neither path connects browser saving, cross-user teaching review or Task acceptance.
The separate [empty Task create path](platform-task-create-connection.md) now admits only an owner's
manual Draft with no payload; this does not turn the index into a writer.
Separate [Draft manifest replacement](platform-task-replace-connection.md) changes titles or existing
owned exact text references with an expected revision, without expanding this read capability.

## Original storage and current authority

`PlatformTaskIndexConnector::new(storage)` consumes a genuine `PlatformStorageConnector`. Origin,
Task namespace and workspace come only from that original configuration. There is no caller-supplied
owner, alternate Source, pool, baseline replacement or storage accessor. Construction performs no I/O.
The initial retained-host policy admits manual Tasks only. A selected catalogue Task without a frozen
configured definition rejects the entire page; it is neither hidden nor promoted to a manual Task.

Each command rejects malformed credentials and pagination before dialing, then opens one fresh
original-storage connection. A READ COMMITTED transaction verifies the retained catalog and all five
markers, resolves the exact Session account incarnation and audited active Human membership, and reads
only that owner's Task heads and latest events. The shared strict reader validates the selected rows,
manifests and one lookahead before constructing a bounded page. Source and Task namespaces retain
their original identities; no later observation replaces those pins.

The prepared read restores the original Source before the host repeats retained-storage checks.
After those waits, final checks repeat original pins, current membership and the exact Session with
fresh expiry. Metadata is returned only after confirmed COMMIT. A login result or earlier Session
query never authorizes this read. A teacher role does not grant another owner's private metadata.

Connection verification, business work and final close retain separate ten-second budgets.
Synchronous Artifact-root metadata checks have no hard deadline and do not lock the root throughout
the business transaction. Confirmed COMMIT remains confirmed if close fails; cancellation or timeout
does not establish remote rollback or permit automatic retry.

No Artifact record or byte is read. Nevertheless, this retained host requires its complete original
storage binding, including the Artifact marker and root. The older explicitly pooled index retains
its narrower metadata-only storage dependency; it is not a fallback for a failed retained binding.

## Explicit HTTP composition

`TaskContentIndexHttpState::with_platform_storage(storage, capacity)` selects only this retained
backend and derives Origin from storage. Existing Source-based constructors and standalone routes
remain available with their original contracts.

`PlatformWorkspaceHttpState::new(storage)` and `build_platform_workspace_router` share one original
storage handle across login, Session lifecycle, index, content-read, empty-create and manifest-replace capabilities.
`with_capacities(storage, login, lifecycle, index)` keeps its signature and defaults content and create
to four; `with_content_capacities` preserves its signature and defaults create to four.
`with_create_capacities` keeps its five-capacity signature; all three also default replacement to four. New
`with_replace_capacities(storage, login, lifecycle, index, content, create, replace)` validates all six
capacities before splitting. Each is 1–16, default 4. Clones share response-body budgets; the six lanes remain
independent; these are not a whole-host connection or worker quota.

The explicit Linux `serve-loopback` and `serve-loopback-tls` modes use these seven routes:

| Exact path | Method | Operation |
|---|---|---|
| `/platform/v1/session/login` | POST | Existing-account login and dedicated Cookie |
| `/platform/v1/session` | GET | Fresh Session status |
| `/platform/v1/session/logout` | POST | Confirmed exact-token logout |
| `/platform/v1/task-index` | GET | Current owner's private Task metadata |
| `/platform/v1/tasks/{task_id}` | GET | Current owner's exact empty/text Task content |
| `/platform/v1/tasks` | POST | Current owner's empty manual Draft, only version/Task ID/title |
| `/platform/v1/tasks/{task_id}/content` | PUT | Current owner's Draft title/existing exact text refs, expected revision required |

The older `build_platform_session_router` still owns exactly the first three paths. Only the separate
empty-create POST and manifest-replacement PUT write Tasks; Artifact publication, status writes, standalone Artifacts,
file distribution, teaching and execution routes stay absent. The host's existing
plain/TLS ingress, notification and shutdown rules do not change. Normal teaching AppState, frontend
authentication, SDK/OpenAPI and LAN/DNS listeners remain unconnected.

Index requests retain the [existing strict HTTP contract](task-content-index-http.md): dedicated
Cookie and request marker; optional exact Origin on GET; empty body; only canonical `after` and
`limit` queries. Pages default to 20 entries, bounded to 1–32 plus one validated lookahead. Each entry
contains only reference, revision, title, status, definition reference, update time and payload count.
Cursors are positions, not authority. Private no-store/no-cache/nosniff responses contain no owner,
manifest, Artifact pins, credentials or reusable permission. Index responses never set or clear Cookies.

## Provisioning and verification

The host does not install, migrate, grant privileges or override an explicit READ ONLY role policy.
Trusted provisioning must supply the existing Source 2, identity/access 2 and Task 3 schemas and
business privileges. Relation pinning does not require password/TOTP SELECT; account/Session column
reads and row-lock privileges, audited Registry reads/locks and Task head/outbox reads/locks are still
required. A read-only HTTP operation is not a PostgreSQL read-only transaction: authorization and
head consistency use row locks. Missing privileges fail privately without a privileged-pool fallback.

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::workspace_router
bash scripts/test-platform-task-index-connection.sh
```

The exact SQL runner requires disposable `CYANREX_TEST_DATABASE_URL` administration. It covers
owner isolation and empty-payload Tasks, revoked membership, original storage drift, actual lock waits
and final expiry, strict lookahead, missing privileges, HTTP cancellation and response-body admission.
The actual loopback child case verifies host mounting, not a deployed classroom or browser workflow.
