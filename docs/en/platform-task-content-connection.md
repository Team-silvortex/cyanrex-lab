# Retained platform Task content reads

Included in source **0.6.3**, the independent platform host can reopen the current owner's saved Task
through exact `GET /platform/v1/tasks/{task_id}`, under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). This extends the
[private metadata index](platform-task-index-connection.md) with the original Task snapshot, manifest
and verified text payload. Empty payload is valid; code, an editor, an assessment and a Run are not
required. Code is optional. This does not connect browser saving; browser reopening,
Artifact publication and general binary payloads remain separate. Existing-reference [Draft manifest
replacement](platform-task-replace-connection.md) is now available through its own retained write capability.

The separate [empty Task create path](platform-task-create-connection.md) now creates an owner's manual
Draft from only a Task ID and title. It is not payload publication or a write method on this reader.

## Original storage and exact content

`PlatformTaskContentConnector::new(storage)` consumes one genuine `PlatformStorageConnector` without
I/O. Origin, scope, Task and Artifact namespaces, and the original root descriptor come only from that
binding. Requests supply only a Task identifier and existing Session credentials, never an owner,
namespace, root path or trusted Artifact reference. The connector has no write methods or pool fallback.
This initial host read is manual-only; catalogue Tasks require separately frozen definitions and are
not silently converted. Existing pooled manual/catalogue APIs retain their own explicit configurations.

Each read opens one fresh original-storage connection and a READ COMMITTED transaction. It verifies
the retained catalog and all five markers, resolves the exact Session account generation and audited
active Human membership, and locks the owner's Task before reading Artifacts. A teacher role does not
grant another owner's private content. Missing and foreign-owned Tasks produce the same verified absence.

The shared reader validates the Task head against its latest event and validates the complete
manifest. It selects and pins the Artifact namespace even for an empty saved Task. Payload references
are read in a fixed Artifact/revision lock order and returned in manifest order. Each exact revision
must agree with its owned head, revision record and outbox, and its private regular file must match
the expected length and SHA-256. A pinned older revision remains that revision; a newer head does not
replace it. UTF-8, text-profile and payload binding checks precede publication of any result.

After byte work, the reader rechecks the Task metadata and restores the original Source. The host
then checks the original root and retained storage again. Only after those waits does it repeat the
original namespace/relation pins, current membership and fresh exact Session expiry, then COMMIT.
No partial payload, pending metadata or reusable authorization leaves before confirmed commit.
Cancelled or failed work does not grant retry, adoption, cleanup or rollback certainty.

## Original descriptors and bounded file work

The Artifact reader duplicates the descriptor retained by the original storage observation and checks
it against the visible no-follow root. It does not open a new acceptable baseline. File reads retain
the existing owned 0700 root and owned, single-link 0400 regular-file policy, exact inode/size/hash
checks and refusal of substituted, missing, symlinked or nonprivate content. This is not isolation
from the same UID or root, nor an atomic filesystem/database transaction.

Each retained content connector has four fail-fast blocking-work slots. The actual blocking closure
holds its permit until completion, even after the HTTP waiter is cancelled or times out. There is no
unbounded fallback queue. Original explicitly pooled Artifact APIs retain their previous worker
policy. Connection verification, business work and close use separate ten-second budgets; synchronous
root metadata checks can still stall the current thread and have no hard deadline. A confirmed
COMMIT remains confirmed if closing the connection fails.

## Read-only HTTP and admission

`TaskContentReadHttpState::new(connector)` or `with_capacity(connector, capacity)` selects only the
retained read capability; construction derives Origin from it and opens no storage. The standalone
`build_task_content_read_router` owns only the exact Task-item route. The existing complete Task
router is not merged: collection creation, `/content` replacement and `/status` mutation stay absent
from this standalone read router. The composed host's separate create/replace capabilities do not expand it.
Only GET is accepted at a canonical item path; other methods return private 405 with `Allow: GET`.
Raw encoded/noncanonical identifiers and any query are rejected before credentials or body work.

The existing dedicated Cookie, request marker, optional exact GET Origin, empty-body, encoding and
two-second body rules apply. Foreign/missing Tasks return private dispatched 404. Responses retain
no-store/no-cache/nosniff, without CORS, Cookie issuance, redirects or token disclosure. Successful
JSON uses the existing shape: `schema_version`, the owner's `task`, `manifest`, and ordered
`contents: [{artifact, text}]`; empty payload returns `contents: []`. Text remains inert data, not HTML,
an executable request, filesystem path or an editor requirement.

The current text profile allows up to 32 items, each at most 256 KiB; JSON escaping can expand the
response beyond the raw-text total. Inbound metadata limits and the small index projection are not
content-response size limits. A valid list entry does not prove its bytes can be reopened later.

The explicit plain/TLS loopback hosts now use seven routes: three Session routes, the metadata index,
this Task-item GET, empty-create POST and manifest-replacement PUT. `PlatformWorkspaceHttpState::with_capacities(storage,
login, lifecycle, index)` keeps its original signature and defaults content, creation and replacement to four.
`with_content_capacities(storage, login, lifecycle, index, content)` also preserves its signature and defaults
creation and replacement to four. `with_create_capacities(storage, login, lifecycle, index, content, create)`
keeps its signature and defaults replacement to four. New `with_replace_capacities(storage, login, lifecycle,
index, content, create, replace)` validates all six capacities before allocating or splitting the original
binding. Each is 1–16, default four. Clones share each
response-lifetime lane through the final body frame/drop; lanes remain independent. These budgets
are not one global connection, memory, worker or distributed-abuse quota. Session-only composition
still has exactly three routes. Normal teaching AppState, frontend authentication, SDK/OpenAPI,
LAN/DNS listeners, general Run and file-distribution mounting remain unchanged.

## Provisioning and verification

No installer, migration, automatic grant or READ WRITE override is added. Trusted provisioning must
provide original Source 2, identity/access 2, Task 3 and Artifact 1 schemas, their read privileges and
the column UPDATE permissions required by authorization/head row locks. The retained read does not
need password/TOTP SELECT, Artifact writes or Task writes; it is not a PostgreSQL read-only transaction.
An explicitly READ ONLY role policy or missing privilege fails without using a privileged pool.

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::task_content_read
cargo test --manifest-path engine/Cargo.toml --locked --lib services::artifact_store::reader
bash scripts/test-platform-task-content-connection.sh
```

The exact runner needs disposable `CYANREX_TEST_DATABASE_URL` administration. Eight real SQL/HTTP/
host cases cover original bindings and descriptors, exact ordered revisions and zero payload, account
and owner isolation, corruption, narrow privileges, observed Task/Artifact waits followed by expiry,
cancellation, response admission and an actual capless loopback process. Worker units cover permit
lifetime after waiter cancellation. This is not browser integration or classroom/LAN deployment acceptance.
